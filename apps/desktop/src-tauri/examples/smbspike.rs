//! 试探：用纯 Rust 的 smb2 客户端直接连 NAS，列目录、读一个文件的开头。凭据只从环境变量来，不落盘。
//! 用法：FG_NAS_HOST=192.168.1.9 FG_NAS_USER=me FG_NAS_PASSWORD='…' [FG_NAS_SHARE=photo] cargo run --example smbspike
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("FG_NAS_HOST")?;
    let user = std::env::var("FG_NAS_USER").unwrap_or_default();
    let pass = std::env::var("FG_NAS_PASSWORD").unwrap_or_default();
    let t = Instant::now();
    let mut client = smb2::connect(&format!("{host}:445"), &user, &pass).await?;
    println!("已连接并通过认证 {host}（{} ms）", t.elapsed().as_millis());
    // 共享列表直接问服务器要，不用猜名字
    let shares = client.list_shares().await?;
    println!("服务器上的共享（{} 个）：", shares.len());
    for sh in &shares { println!("  {sh:?}"); }
    let wanted = std::env::var("FG_NAS_SHARE").ok();
    for sh in &shares {
        let name = format!("{sh:?}");
        let name = name.split('"').nth(1).unwrap_or("").to_string();
        if name.is_empty() || name.ends_with('$') { continue; }
        if let Some(w) = &wanted { if &name != w { continue; } }
        let t = Instant::now();
        match client.connect_share(&name).await {
            Ok(mut tree) => {
                match client.list_directory(&mut tree, "").await {
                    Ok(entries) => {
                        println!("共享「{name}」根目录 {} 项（{} ms）：", entries.len(), t.elapsed().as_millis());
                        for e in entries.iter().take(10) { println!("    {e:?}"); }
                    }
                    Err(e) => println!("共享「{name}」列目录失败：{e}"),
                }
            }
            Err(e) => println!("共享「{name}」连不上：{e}"),
        }
    }
    Ok(())
}
