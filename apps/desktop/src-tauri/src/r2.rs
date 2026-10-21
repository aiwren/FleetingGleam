//! 云备份：把"我的设置与喜好"（见 taste.rs，只有设置和各风格学到的向量与决定，没有照片、项目和路径）自动存到用户自己的
//! Cloudflare R2 桶里。没有账号系统，所以用的是用户自己在 Cloudflare 后台建的桶和 API 令牌：账户 ID、桶名、Access Key ID 记在
//! `backup.json`，Secret Access Key 在应用自己的加密仓库里（secrets.rs）。R2 走 S3 兼容接口，这里自己做 SigV4 签名（只需要 PUT / GET 两个对象操作），
//! 不引入整套 AWS SDK。

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 备份对象在桶里的键：固定一份"最新"，另外按天在 `auto/` 下轮转一份（只留最近 `keep` 份）；"立即备份"走的也是这一套。
pub const KEY_LATEST: &str = "fleetinggleam/taste.json";
pub const KEY_AUTO: &str = "fleetinggleam/auto";
/// 自动备份默认留这么多份。
pub const DEFAULT_KEEP: usize = 30;

/// 一个备份目标（目前只有 Cloudflare R2 一种）：用户起的名字 + 桶的位置；Secret 只在钥匙串里，按目标 id 存。
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Target {
    pub id: String,
    pub name: String,
    /// 方式：现在只有 `r2`。
    #[serde(default = "kind_r2")]
    pub kind: String,
    /// Cloudflare 账户 ID；也可以直接填完整的 S3 端点（带 https://）。
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub access_key: String,
    /// 上一次成功备份到这里的时间（Unix 秒）；0 = 还没有过。
    #[serde(default)]
    pub last_ok: u64,
    #[serde(default)]
    pub last_error: Option<String>,
}

fn kind_r2() -> String {
    "r2".to_string()
}

impl Target {
    pub fn configured(&self) -> bool {
        !self.account.is_empty() && !self.bucket.is_empty() && !self.access_key.is_empty()
    }
    fn endpoint(&self) -> String {
        let a = self.account.trim().trim_end_matches('/');
        if a.contains("://") { a.to_string() } else { format!("https://{a}.r2.cloudflarestorage.com") }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// 开着就自动备份（设置或喜好变了之后过一会儿上传到每个目标）。
    #[serde(default)]
    pub enabled: bool,
    /// 自动备份最多留几份（按天一份）；0 当作默认。
    #[serde(default = "default_keep")]
    pub keep: usize,
    #[serde(default)]
    pub targets: Vec<Target>,
    // 下面三个是早期单目标格式留下的字段，只用来迁移
    #[serde(default, skip_serializing)]
    account: String,
    #[serde(default, skip_serializing)]
    bucket: String,
    #[serde(default, skip_serializing)]
    access_key: String,
}

fn default_keep() -> usize {
    DEFAULT_KEEP
}

impl Config {
    pub fn configured(&self) -> bool {
        self.targets.iter().any(Target::configured)
    }
    pub fn keep(&self) -> usize {
        if self.keep == 0 { DEFAULT_KEEP } else { self.keep }
    }
}

pub fn load(dir: &Path) -> Config {
    let mut c: Config = std::fs::read(dir.join("backup.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    // 早期的单目标格式：桶信息在顶层，Secret 按 Access Key 存；搬成一个目标，Secret 也搬到按 id 的条目
    if c.targets.is_empty() && !c.account.is_empty() {
        let t = Target { id: "t1".to_string(), name: "Cloudflare R2".to_string(), kind: kind_r2(), account: std::mem::take(&mut c.account), bucket: std::mem::take(&mut c.bucket), access_key: std::mem::take(&mut c.access_key), last_ok: 0, last_error: None };
        c.targets.push(t);
        let _ = save(dir, &c);
    }
    c
}

pub fn save(dir: &Path, c: &Config) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(c).map_err(|e| e.to_string())?;
    let path = dir.join("backup.json");
    let tmp = path.with_extension("json.tmp");
    crate::write_synced(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
}

// ---------------------------------------------------------------- Secret Access Key：本地加密仓库（见 secrets.rs）

fn secret_id(target_id: &str) -> String {
    format!("backup:{target_id}")
}

pub fn remember_secret(dir: &Path, target_id: &str, secret: &str) -> Result<(), String> {
    crate::secrets::set(dir, &secret_id(target_id), secret)
}
pub fn saved_secret(dir: &Path, target_id: &str) -> Option<String> {
    crate::secrets::get(dir, &secret_id(target_id))
}
pub fn forget_secret(dir: &Path, target_id: &str) {
    crate::secrets::delete(dir, &secret_id(target_id));
}

// ---------------------------------------------------------------- SigV4

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut m = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes any key length");
    m.update(data);
    m.finalize().into_bytes().to_vec()
}

/// 现在的 UTC 时间，签名要的两种写法：`YYYYMMDD` 和 `YYYYMMDDTHHMMSSZ`。
fn utc_now() -> (String, String) {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (days, rem) = (secs / 86400, secs % 86400);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // 从 1970 年起的天数算成年月日（Howard Hinnant 的算法）
    let z = days as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let date = format!("{y:04}{m:02}{d:02}");
    (date.clone(), format!("{date}T{hh:02}{mm:02}{ss:02}Z"))
}

pub fn today() -> String {
    let (d, _) = utc_now();
    format!("{}-{}-{}", &d[..4], &d[4..6], &d[6..])
}



/// 路径里每一段按 RFC 3986 编码（`/` 保留），和 S3 对规范化 URI 的要求一致。
fn encode_path(path: &str) -> String {
    let mut out = String::new();
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// 一次带签名的请求：方法、对象键（空串 = 桶本身）、查询串（已按 S3 的规范排好序并编码）、请求体；返回状态码和响应体。
fn request(cfg: &Target, secret: &str, method: &str, key: &str, query: &str, body: &[u8]) -> Result<(u16, Vec<u8>), String> {
    let endpoint = cfg.endpoint();
    let host = endpoint.split("://").nth(1).unwrap_or(&endpoint).trim_end_matches('/').to_string();
    let path = encode_path(&if key.is_empty() { format!("/{}", cfg.bucket.trim()) } else { format!("/{}/{}", cfg.bucket.trim(), key) });
    let (date, amz_date) = utc_now();
    let payload_hash = sha256_hex(body);
    let canonical = format!("{method}\n{path}\n{query}\nhost:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n\nhost;x-amz-content-sha256;x-amz-date\n{payload_hash}");
    let scope = format!("{date}/auto/s3/aws4_request");
    let to_sign = format!("AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}", sha256_hex(canonical.as_bytes()));
    let k_date = hmac(format!("AWS4{secret}").as_bytes(), date.as_bytes());
    let k_region = hmac(&k_date, b"auto");
    let k_service = hmac(&k_region, b"s3");
    let k_signing = hmac(&k_service, b"aws4_request");
    let signature = hex(&hmac(&k_signing, to_sign.as_bytes()));
    let auth = format!("AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders=host;x-amz-content-sha256;x-amz-date, Signature={signature}", cfg.access_key.trim());
    let agent = ureq::AgentBuilder::new().timeout_connect(std::time::Duration::from_secs(15)).timeout_read(std::time::Duration::from_secs(60)).build();
    let url = if query.is_empty() { format!("{endpoint}{path}") } else { format!("{endpoint}{path}?{query}") };
    let req = agent.request(method, &url).set("Authorization", &auth).set("x-amz-content-sha256", &payload_hash).set("x-amz-date", &amz_date);
    let resp = match if method == "PUT" { req.set("Content-Type", "application/json").send_bytes(body) } else { req.call() } {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => r.into_string().map(|t| (code, t)).map_or_else(|e| Err(format!("R2 answered {code}: {e}")), |(code, text)| Err(explain(code, &text)))?,
        Err(e) => return Err(format!("cannot reach R2: {e}")),
    };
    let code = resp.status();
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut resp.into_reader(), &mut bytes).map_err(|e| e.to_string())?;
    Ok((code, bytes))
}

/// 把 R2 的错误说成人话；原始的错误码留在括号里。
fn explain(code: u16, text: &str) -> String {
    let code_tag = text.split("<Code>").nth(1).and_then(|s| s.split("</Code>").next()).unwrap_or("");
    match (code, code_tag) {
        (403, _) | (401, _) => format!("R2 refused the credentials ({code} {code_tag})"),
        (404, "NoSuchBucket") => "no such bucket in that account".to_string(),
        (400, "InvalidAccessKeyId") => "the Access Key ID is not known to R2".to_string(),
        _ => format!("R2 answered {code} {code_tag}"),
    }
}

/// 上传一个对象。
pub fn put(cfg: &Target, secret: &str, key: &str, body: &[u8]) -> Result<(), String> {
    request(cfg, secret, "PUT", key, "", body).map(|_| ())
}

/// 删一个对象（不存在也算成功）。
pub fn delete(cfg: &Target, secret: &str, key: &str) -> Result<(), String> {
    request(cfg, secret, "DELETE", key, "", &[]).map(|_| ())
}

/// 某个前缀下的对象键（ListObjectsV2，最多一页 1000 个，够历史用），按字典序。
pub fn list(cfg: &Target, secret: &str, prefix: &str) -> Result<Vec<String>, String> {
    let query = format!("list-type=2&prefix={}", encode_query(prefix));
    let (_, body) = request(cfg, secret, "GET", "", &query, &[])?;
    let text = String::from_utf8_lossy(&body);
    let mut keys: Vec<String> = text.split("<Key>").skip(1).filter_map(|s| s.split("</Key>").next()).map(unescape_xml).collect();
    keys.sort();
    Ok(keys)
}

/// 查询串里的值按 S3 的要求编码（和路径一样，但 `/` 也要编）。
fn encode_query(v: &str) -> String {
    encode_path(v).replace('/', "%2F")
}

fn unescape_xml(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'")
}

/// 按天的那份只留最近 `keep` 份：键里带日期，字典序就是时间序，删掉靠前的。返回删掉的个数。
pub fn prune_auto(cfg: &Target, secret: &str, keep: usize) -> Result<usize, String> {
    let keep = if keep == 0 { DEFAULT_KEEP } else { keep };
    let keys = list(cfg, secret, &format!("{KEY_AUTO}/"))?;
    let extra = keys.len().saturating_sub(keep);
    for k in &keys[..extra] {
        delete(cfg, secret, k)?;
    }
    Ok(extra)
}

/// 读一个对象；不存在时是 `None`。
pub fn get(cfg: &Target, secret: &str, key: &str) -> Result<Option<Vec<u8>>, String> {
    match request(cfg, secret, "GET", key, "", &[]) {
        Ok((_, bytes)) => Ok(Some(bytes)),
        Err(e) if e.contains("NoSuchKey") || e.contains("404 ") => Ok(None),
        Err(e) => Err(e),
    }
}

/// 检查桶和凭据：读一下"最新"这个键，存在或不存在都算通；凭据错或桶不存在会报出来。
pub fn check(cfg: &Target, secret: &str) -> Result<bool, String> {
    get(cfg, secret, KEY_LATEST).map(|b| b.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_endpoint_comes_from_the_account_id_or_is_given_whole() {
        let c = Target { account: "abc123".into(), bucket: "b".into(), ..Default::default() };
        assert_eq!(serde_json::from_str::<Config>("{}").unwrap().keep(), DEFAULT_KEEP, "an old backup.json without keep gets the default");
        assert_eq!(c.endpoint(), "https://abc123.r2.cloudflarestorage.com");
        let c = Target { account: "https://s3.example.com/".into(), ..Default::default() };
        assert_eq!(c.endpoint(), "https://s3.example.com");
        assert!(!c.configured());
        // 早期单目标格式会被搬成一个目标
        let dir = std::env::temp_dir().join(format!("fg_backup_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("backup.json"), br#"{"account":"acc","bucket":"b","access_key":"k","enabled":true}"#).unwrap();
        let c = load(&dir);
        assert_eq!(c.targets.len(), 1);
        assert_eq!((c.targets[0].id.as_str(), c.targets[0].bucket.as_str(), c.targets[0].kind.as_str()), ("t1", "b", "r2"));
        assert!(c.enabled && c.configured());
        assert!(std::fs::read_to_string(dir.join("backup.json")).unwrap().contains("\"targets\""), "written back in the new shape");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn paths_are_encoded_like_s3_wants() {
        assert_eq!(encode_path("/my-bucket/fleetinggleam/taste.json"), "/my-bucket/fleetinggleam/taste.json");
        assert_eq!(encode_path("/b/a b+c"), "/b/a%20b%2Bc");
        assert_eq!(encode_query("fleetinggleam/history/"), "fleetinggleam%2Fhistory%2F");
        assert_eq!(unescape_xml("a&amp;b&lt;c"), "a&b<c");
    }

    /// AWS 文档里的 SigV4 签名密钥示例（Example Signing Key），确认 HMAC 链没接错。
    #[test]
    fn the_signing_key_matches_the_aws_example() {
        let k_date = hmac(b"AWS4wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY", b"20150830");
        let k_region = hmac(&k_date, b"us-east-1");
        let k_service = hmac(&k_region, b"iam");
        let k_signing = hmac(&k_service, b"aws4_request");
        assert_eq!(hex(&k_signing), "c4afb1cc5771d871763a393e44b703571b55cc28424d1a5e86da6ed3c154a4b9");
    }

    #[test]
    fn dates_are_utc_in_the_two_shapes_sigv4_wants() {
        let (d, t) = utc_now();
        assert_eq!(d.len(), 8);
        assert_eq!(t.len(), 16);
        assert!(t.starts_with(&d) && t.ends_with('Z'));
        assert_eq!(today().len(), 10);
    }

    /// 对着真的桶：`FG_R2_ACCOUNT=… FG_R2_BUCKET=… FG_R2_KEY=… FG_R2_SECRET=… cargo test --lib r2::tests::a_real_bucket -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn a_real_bucket_takes_and_gives_back_an_object() {
        let Ok(account) = std::env::var("FG_R2_ACCOUNT") else { return };
        let cfg = Target { account, bucket: std::env::var("FG_R2_BUCKET").unwrap(), access_key: std::env::var("FG_R2_KEY").unwrap(), ..Default::default() };
        let secret = std::env::var("FG_R2_SECRET").unwrap();
        let key = "fleetinggleam/test/probe.json";
        put(&cfg, &secret, key, br#"{"hello":"r2"}"#).expect("put");
        let back = get(&cfg, &secret, key).expect("get").expect("the object is there");
        assert_eq!(back, br#"{"hello":"r2"}"#);
        assert!(get(&cfg, &secret, "fleetinggleam/test/missing.json").expect("a missing key is not an error").is_none());
        let keys = list(&cfg, &secret, "fleetinggleam/test/").expect("list");
        assert!(keys.contains(&key.to_string()), "{keys:?}");
        delete(&cfg, &secret, key).expect("delete");
        assert!(!list(&cfg, &secret, "fleetinggleam/test/").unwrap().contains(&key.to_string()));
    }
}
