//! Search photos by describing them. A multilingual text encoder (Chinese, English and 50 more languages) maps the query into the same
//! 512-number space as the CLIP ViT-B/32 image embeddings, so "烟花", "two people on stairs" or "红色的囍字" can be compared with every
//! photo by a dot product. Nothing leaves the machine: the model is local and the photos are only ever compared through their embeddings.

use std::path::Path;
use std::sync::Mutex;

use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

use crate::modelfiles;

pub const TEXT_MODEL: &str = "text_multilingual_qint8.onnx";
pub const TEXT_TOKENIZER: &str = "text_multilingual_tokenizer.json";
pub const TEXT_DENSE: &str = "text_multilingual_dense.bin";
const MAX_LEN: usize = 64;
const HIDDEN: usize = 768;
const OUT: usize = 512;

pub struct TextEncoder {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
    /// Projection 768 -> 512 (row-major, no bias), the last layer of the sentence-transformers model.
    dense: Vec<f32>,
}

impl TextEncoder {
    pub fn available(dir: &Path) -> bool {
        [TEXT_MODEL, TEXT_TOKENIZER, TEXT_DENSE].iter().all(|n| modelfiles::has_model(dir, n))
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let mut b = Session::builder().map_err(|e| e.to_string())?;
        // The int8 model is built for the CPU; it is tiny work (one short sentence), so no accelerator is involved.
        let session = b.commit_from_file(dir.join(TEXT_MODEL)).map_err(|e| e.to_string())?;
        let tokenizer = Tokenizer::from_bytes(modelfiles::read_model(dir, TEXT_TOKENIZER)?).map_err(|e| e.to_string())?;
        let raw = modelfiles::read_model(dir, TEXT_DENSE)?;
        if raw.len() != HIDDEN * OUT * 4 {
            return Err("text projection has an unexpected size".into());
        }
        let dense = raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        Ok(Self { session: Mutex::new(session), tokenizer, dense })
    }

    /// The embedding of a query: its prompts are encoded and averaged.
    pub fn encode_query(&self, text: &str) -> Result<Vec<f32>, String> {
        let mut acc = vec![0f32; OUT];
        for p in &prompts(text) {
            for (a, v) in acc.iter_mut().zip(self.encode(p)?) {
                *a += v;
            }
        }
        let norm = acc.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-9);
        acc.iter_mut().for_each(|v| *v /= norm);
        Ok(acc)
    }

    /// The unit-length embedding of a sentence.
    pub fn encode(&self, text: &str) -> Result<Vec<f32>, String> {
        let enc = self.tokenizer.encode(text, true).map_err(|e| e.to_string())?;
        let n = enc.get_ids().len().min(MAX_LEN);
        if n == 0 {
            return Err("empty query".into());
        }
        let ids: Vec<i64> = enc.get_ids()[..n].iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = enc.get_attention_mask()[..n].iter().map(|&x| x as i64).collect();
        let ids_t = Tensor::from_array(([1usize, n], ids)).map_err(|e| e.to_string())?;
        let mask_t = Tensor::from_array(([1usize, n], mask.clone())).map_err(|e| e.to_string())?;
        let tokens: Vec<f32> = {
            let mut s = self.session.lock().map_err(|e| e.to_string())?;
            let out = s.run(ort::inputs!["input_ids" => ids_t, "attention_mask" => mask_t]).map_err(|e| e.to_string())?;
            let (_, data) = out[0].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
            data.to_vec()
        };
        if tokens.len() != n * HIDDEN {
            return Err(format!("unexpected text model output ({} values for {n} tokens)", tokens.len()));
        }
        // mean pooling over the real tokens
        let mut pooled = vec![0f32; HIDDEN];
        let mut count = 0f32;
        for (t, &m) in mask.iter().enumerate() {
            if m == 0 { continue; }
            count += 1.0;
            for (i, p) in pooled.iter_mut().enumerate() {
                *p += tokens[t * HIDDEN + i];
            }
        }
        pooled.iter_mut().for_each(|p| *p /= count.max(1.0));
        let mut out = vec![0f32; OUT];
        for (j, o) in out.iter_mut().enumerate() {
            let row = &self.dense[j * HIDDEN..(j + 1) * HIDDEN];
            *o = row.iter().zip(&pooled).map(|(w, x)| w * x).sum();
        }
        if out.iter().any(|v| !v.is_finite()) {
            return Err("text model produced a non-finite embedding".into());
        }
        let norm = out.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-9);
        out.iter_mut().for_each(|v| *v /= norm);
        Ok(out)
    }
}


/// Common words people use to describe photos, with the English the image-text model understands best. The model is clearly sharper
/// in English (the same query scores far higher on real matches than on absent things), so a Chinese query is compared in both languages.
const GLOSSARY: &[(&str, &str)] = &[
    ("烟花", "fireworks"), ("雨伞", "umbrella"), ("伞", "umbrella"), ("教堂", "church"), ("婚纱", "wedding dress"), ("婚礼", "wedding"), ("新娘", "bride"), ("新郎", "groom"),
    ("海边", "seaside"), ("大海", "sea"), ("沙滩", "beach"), ("海", "sea"), ("湖", "lake"), ("河", "river"), ("山", "mountain"), ("雪山", "snow mountain"), ("雪", "snow"),
    ("森林", "forest"), ("树", "trees"), ("草地", "grass lawn"), ("花", "flowers"), ("玫瑰", "roses"), ("向日葵", "sunflowers"), ("天空", "sky"), ("云", "clouds"),
    ("日落", "sunset"), ("日出", "sunrise"), ("夜景", "night scene"), ("夜晚", "night"), ("雨", "rain"), ("阳光", "sunlight"), ("逆光", "backlight"),
    ("猫", "cat"), ("狗", "dog"), ("鸟", "bird"), ("鱼", "fish"), ("水族馆", "aquarium"), ("动物", "animal"), ("汽车", "car"), ("自行车", "bicycle"), ("火车", "train"), ("飞机", "airplane"),
    ("街道", "street"), ("建筑", "building"), ("楼梯", "stairs"), ("桥", "bridge"), ("窗户", "window"), ("门", "door"), ("房间", "room"), ("餐厅", "restaurant"), ("咖啡", "coffee"),
    ("蛋糕", "cake"), ("美食", "food"), ("食物", "food"), ("饭", "meal"), ("桌子", "table"), ("椅子", "chair"), ("长椅", "bench"), ("气球", "balloons"), ("泡泡", "bubbles"),
    ("女孩", "girl"), ("男孩", "boy"), ("女生", "girl"), ("男生", "boy"), ("女人", "woman"), ("男人", "man"), ("小孩", "child"), ("宝宝", "baby"), ("情侣", "couple"), ("新人", "newlyweds"),
    ("拥抱", "hugging"), ("亲吻", "kissing"), ("牵手", "holding hands"), ("微笑", "smiling"), ("大笑", "laughing"), ("背影", "back view of a person"), ("侧脸", "profile of a face"),
    ("自拍", "selfie"), ("戒指", "ring"), ("捧花", "bouquet"), ("手捧花", "bouquet"), ("西装", "suit"), ("裙子", "dress"), ("帽子", "hat"), ("眼镜", "glasses"), ("墨镜", "sunglasses"),
    ("红色", "red"), ("蓝色", "blue"), ("绿色", "green"), ("黄色", "yellow"), ("白色", "white"), ("黑色", "black"), ("粉色", "pink"), ("紫色", "purple"),
    ("囍字", "double happiness character"), ("喜字", "double happiness character"), ("灯笼", "lantern"), ("蜡烛", "candle"), ("礼物", "gift"), ("手机", "phone"), ("书", "book"),
    ("公园", "park"), ("花园", "garden"), ("城市", "city"), ("乡村", "countryside"), ("旅行", "travel"), ("风景", "landscape"), ("室内", "indoors"), ("室外", "outdoors"), ("在", ""), ("上", ""), ("里", ""), ("的", ""), ("和", "and"), ("与", "and"),
];

/// The prompts a query is compared with: the Chinese sentence and, when every word is known, its English counterpart.
pub fn prompts(text: &str) -> Vec<String> {
    let t = text.trim();
    if t.is_ascii() {
        return vec![t.to_string()]; // already English: use it as typed
    }
    let mut out = vec![format!("一张{t}的照片")];
    let mut order: Vec<&(&str, &str)> = GLOSSARY.iter().collect();
    order.sort_by_key(|(z, _)| std::cmp::Reverse(z.chars().count()));
    let mut rest = t.replace(' ', "");
    let mut found: Vec<(usize, &str)> = Vec::new();
    for (zh, en) in order {
        while let Some(pos) = rest.find(zh) {
            found.push((pos, en));
            rest.replace_range(pos..pos + zh.len(), &"\u{0}".repeat(zh.len()));
        }
    }
    let leftover = rest.chars().any(|c| c != '\u{0}');
    if !leftover && !found.is_empty() {
        found.sort_by_key(|f| f.0);
        let words: Vec<&str> = found.iter().map(|f| f.1).filter(|w| !w.is_empty()).collect();
        out.push(format!("a photo of {}", words.join(" ")));
    }
    out
}

/// How many people the query asks for (the face detector counts them far better than CLIP does), and the query without those words.
pub fn people_in_query(q: &str) -> (Option<u32>, String) {
    // 1..=4 exactly that many faces, 5 = four or more, 6 = at least two ("group photo" of any size)
    let rules: [(&[&str], u32); 6] = [
        (&["一个人", "1个人", "单人", "一人", "独照", "single person", "one person", "solo"], 1),
        (&["两个人", "2个人", "两人", "二人", "双人", "情侣", "two people", "2 people", "couple"], 2),
        (&["三个人", "3个人", "三人", "three people", "3 people"], 3),
        (&["四个人", "4个人", "四人", "four people", "4 people"], 4),
        (&["多人", "很多人", "集体", "group of people", "crowd"], 5),
        (&["合照", "合影", "group photo", "together"], 6),
    ];
    let lower = q.to_lowercase();
    for (words, n) in rules {
        for w in words {
            if lower.contains(w) {
                let rest = lower.replacen(w, " ", 1);
                return (Some(n), rest.split_whitespace().collect::<Vec<_>>().join(" "));
            }
        }
    }
    (None, q.trim().to_string())
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// A query taken apart: words the measuring rules answer exactly (colours, number of people) and the rest, left to the text model.
pub struct Query {
    pub text: Option<String>,
    pub colours: Vec<usize>,
    pub background: bool,
    pub people: Option<u32>,
}

pub fn parse(q: &str) -> Query {
    let (people, rest) = people_in_query(q);
    let (colours, background, rest) = crate::colorsig::parse_query(&rest);
    Query { text: (!rest.trim().is_empty()).then_some(rest), colours, background, people }
}

/// What the ranking needs to know about one photo.
pub struct Item<'a> {
    pub sem: Option<&'a [f32]>,
    pub color: Option<&'a [u8]>,
    pub faces: u32,
}

/// Scores one photo for a query, or `None` if it cannot be what was asked for. A background colour must really fill a good part of
/// the border, so "red background" never returns a photo that merely has a red flower in it.
pub fn score(q: &Query, qv: Option<&[f32]>, it: &Item) -> Option<f32> {
    let mut score = match (qv, it.sem) {
        (Some(v), Some(e)) => cosine(v, e),
        (Some(_), None) => return None,
        (None, _) => 0.0,
    };
    if !q.colours.is_empty() {
        let sig = it.color?;
        let frac = q.colours.iter().map(|&c| crate::colorsig::fraction(sig, c, q.background)).sum::<f32>() / q.colours.len() as f32;
        if q.background {
            // "red background": the border of the picture must really be that colour
            if frac < 0.25 {
                return None;
            }
            score += 0.25 * frac;
        } else {
            // "blue dress": the colour belongs to something in the picture, which a colour count cannot pin down, so it only helps
            score += 0.10 * frac;
        }
    }
    if let Some(n) = q.people {
        let ok = match n { 5 => it.faces >= 4, 6 => it.faces >= 2, _ => it.faces == n };
        score += if ok { 0.06 } else { -0.06 };
    }
    Some(score)
}

/// The lowest similarity that counts as "really matches". Measured on real photos: things that are in the photos score 0.27-0.30, things
/// that are not (fireworks, in a project without any) top out around 0.24-0.26. A query about something absent then finds nothing,
/// instead of the least-unlike photos.
pub const MATCH_MIN: f32 = 0.265;

/// Best first, keeping only what stands out: close to the best match and clearly above the typical photo.
pub fn rank(q: &Query, qv: Option<&[f32]>, items: &[Item]) -> Vec<(usize, f32)> {
    let mut hits: Vec<(usize, f32)> = items.iter().enumerate().filter_map(|(i, it)| score(q, qv, it).map(|s| (i, s))).collect();
    hits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    if let Some(v) = qv {
        // Judged by the similarity to the words themselves; the colour and people bonuses do not count towards "is it there".
        let cos_of = |i: usize| items[i].sem.map_or(f32::MIN, |e| cosine(v, e));
        if q.colours.is_empty() && q.people.is_none() {
            if hits.iter().map(|h| cos_of(h.0)).fold(f32::MIN, f32::max) < MATCH_MIN {
                return Vec::new();
            }
            hits.retain(|h| cos_of(h.0) >= MATCH_MIN - 0.02);
        }
    }
    if let (Some(&(_, best)), true) = (hits.first(), hits.len() > 3) {
        let cutoff = if q.text.is_some() { best - 0.05 } else { best * 0.7 };
        hits.retain(|h| h.1 >= cutoff);
        hits.truncate(500);
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn people_words_are_understood() {
        assert_eq!(people_in_query("两个人 在楼梯上"), (Some(2), "在楼梯上".to_string()));
        assert_eq!(people_in_query("烟花"), (None, "烟花".to_string()));
        assert_eq!(people_in_query("Two people on stairs"), (Some(2), "on stairs".to_string()));
        assert_eq!(people_in_query("合影").0, Some(6));
        assert_eq!(people_in_query("红底合照"), (Some(6), "红底".to_string()));
    }
}

#[cfg(test)]
mod prompt_tests {
    use super::*;

    #[test]
    fn prompts_in_two_languages() {
        assert_eq!(prompts("烟花"), vec!["一张烟花的照片".to_string(), "a photo of fireworks".to_string()]);
        assert_eq!(prompts("草地上的女孩")[1], "a photo of grass lawn girl");
        assert_eq!(prompts("神秘的东西").len(), 1); // unknown words: Chinese only
        assert_eq!(prompts("two people on stairs"), vec!["two people on stairs".to_string()]);
    }
}
