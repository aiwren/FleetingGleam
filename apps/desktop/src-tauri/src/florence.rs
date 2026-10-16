//! Says in a sentence what a photo shows, with Florence-2 (a small image-to-text model that runs on this machine).
//! The model is four ONNX parts: a vision encoder that turns the picture into 577 feature vectors, a token-embedding table,
//! and a text encoder + decoder that write the caption one word at a time. There is no vocabulary of scenes anywhere: the
//! decoder writes whatever it sees ("a woman holding a bouquet on a road by a river").

use std::path::Path;
use std::sync::Mutex;

use image::imageops::FilterType;
use image::DynamicImage;
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

use crate::modelfiles;

pub const VISION: &str = "florence_vision_encoder_int8.onnx";
pub const EMBED: &str = "florence_embed_tokens_int8.onnx";
pub const ENCODER: &str = "florence_encoder_model_int8.onnx";
pub const DECODER: &str = "florence_decoder_model_merged_int8.onnx";
pub const TOKENIZER: &str = "florence_tokenizer.json";
pub const FILES: [&str; 5] = [VISION, EMBED, ENCODER, DECODER, TOKENIZER];

const SIZE: u32 = 768;
const HIDDEN: usize = 768;
const LAYERS: usize = 6;
const HEADS: usize = 12;
const HEAD_DIM: usize = 64;
const START: i64 = 2;
const FORCED_BOS: i64 = 0;
const EOS: i64 = 2;
const MAX_TOKENS: usize = 40;
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];
/// Florence-2's task prompt for a short caption.
const PROMPT: &str = "What does the image describe?";

fn session(dir: &Path, name: &str) -> Result<Session, String> {
    // Two threads each: describing scenes runs while the app is in use and must not take every core.
    let mut b = Session::builder().map_err(|e| e.to_string())?.with_intra_threads(2).map_err(|e| e.to_string())?;
    b.commit_from_file(dir.join(name)).map_err(|e| e.to_string())
}

pub struct Captioner {
    vision: Mutex<Session>,
    embed: Mutex<Session>,
    encoder: Mutex<Session>,
    decoder: Mutex<Session>,
    tokenizer: Tokenizer,
}

impl Captioner {
    pub fn available(dir: &Path) -> bool {
        FILES.iter().all(|n| modelfiles::has_model(dir, n))
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let tokenizer = Tokenizer::from_bytes(modelfiles::read_model(dir, TOKENIZER)?).map_err(|e| e.to_string())?;
        Ok(Self {
            vision: Mutex::new(session(dir, VISION)?),
            embed: Mutex::new(session(dir, EMBED)?),
            encoder: Mutex::new(session(dir, ENCODER)?),
            decoder: Mutex::new(session(dir, DECODER)?),
            tokenizer,
        })
    }

    fn embed_ids(&self, ids: &[i64]) -> Result<Vec<f32>, String> {
        let t = Tensor::from_array(([1usize, ids.len()], ids.to_vec())).map_err(|e| e.to_string())?;
        let mut s = self.embed.lock().map_err(|e| e.to_string())?;
        let out = s.run(ort::inputs!["input_ids" => t]).map_err(|e| e.to_string())?;
        let (_, data) = out["inputs_embeds"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        Ok(data.to_vec())
    }

    /// Any Florence-2 task prompt.
    pub fn ask(&self, img: &DynamicImage, prompt: &str, max_tokens: usize) -> Result<String, String> {
        self.ask_raw(img, prompt, max_tokens, true)
    }

    fn ask_raw(&self, img: &DynamicImage, prompt: &str, max_tokens: usize, skip_special: bool) -> Result<String, String> {
        // 1. the picture as feature vectors
        let rgb = img.resize_exact(SIZE, SIZE, FilterType::CatmullRom).to_rgb8();
        let plane = (SIZE * SIZE) as usize;
        let mut px = vec![0f32; 3 * plane];
        for (i, p) in rgb.pixels().enumerate() {
            for c in 0..3 {
                px[c * plane + i] = (p[c] as f32 / 255.0 - MEAN[c]) / STD[c];
            }
        }
        let image_features: Vec<f32> = {
            let t = Tensor::from_array(([1usize, 3, SIZE as usize, SIZE as usize], px)).map_err(|e| e.to_string())?;
            let mut s = self.vision.lock().map_err(|e| e.to_string())?;
            let out = s.run(ort::inputs!["pixel_values" => t]).map_err(|e| e.to_string())?;
            let (_, d) = out["image_features"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
            d.to_vec()
        };
        // 2. the task prompt, as embeddings, after the picture
        let enc = self.tokenizer.encode(prompt, true).map_err(|e| e.to_string())?;
        let prompt_ids: Vec<i64> = enc.get_ids().iter().map(|&x| x as i64).collect();
        let mut merged = image_features;
        merged.extend(self.embed_ids(&prompt_ids)?);
        let len = merged.len() / HIDDEN;
        // 3. the text encoder reads both
        let hidden: Vec<f32> = {
            let e = Tensor::from_array(([1usize, len, HIDDEN], merged)).map_err(|e| e.to_string())?;
            let m = Tensor::from_array(([1usize, len], vec![1i64; len])).map_err(|e| e.to_string())?;
            let mut s = self.encoder.lock().map_err(|e| e.to_string())?;
            let out = s.run(ort::inputs!["inputs_embeds" => e, "attention_mask" => m]).map_err(|e| e.to_string())?;
            let (_, d) = out["last_hidden_state"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
            d.to_vec()
        };
        // 4. the decoder writes the caption, one token at a time (greedy; it re-reads what it has written each step, which is
        //    cheap for a sentence and saves carrying a cache between steps)
        let mut ids: Vec<i64> = vec![START, FORCED_BOS];
        for _ in 0..max_tokens {
            let emb = self.embed_ids(&ids)?;
            let mut inputs: Vec<(std::borrow::Cow<'static, str>, ort::session::SessionInputValue<'static>)> = vec![
                ("encoder_attention_mask".into(), Tensor::from_array(([1usize, len], vec![1i64; len])).map_err(|e| e.to_string())?.into()),
                ("encoder_hidden_states".into(), Tensor::from_array(([1usize, len, HIDDEN], hidden.clone())).map_err(|e| e.to_string())?.into()),
                ("inputs_embeds".into(), Tensor::from_array(([1usize, ids.len(), HIDDEN], emb)).map_err(|e| e.to_string())?.into()),
                ("use_cache_branch".into(), Tensor::from_array(([1usize], vec![false])).map_err(|e| e.to_string())?.into()),
            ];
            for l in 0..LAYERS {
                for part in ["decoder.key", "decoder.value", "encoder.key", "encoder.value"] {
                    inputs.push((format!("past_key_values.{l}.{part}").into(), Tensor::from_array(([1usize, HEADS, 0, HEAD_DIM], Vec::<f32>::new())).map_err(|e| e.to_string())?.into()));
                }
            }
            let next = {
                let mut s = self.decoder.lock().map_err(|e| e.to_string())?;
                let out = s.run(inputs).map_err(|e| e.to_string())?;
                let (shape, logits) = out["logits"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
                let vocab = *shape.last().ok_or("no logits")? as usize;
                let last = &logits[logits.len() - vocab..];
                last.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).map(|(i, _)| i as i64).ok_or("no logits")?
            };
            if next == EOS { break; }
            ids.push(next);
        }
        let words: Vec<u32> = ids.iter().skip(2).map(|&x| x as u32).collect();
        Ok(self.tokenizer.decode(&words, skip_special).map_err(|e| e.to_string())?.trim().to_string())
    }
}

/// Boxes (fractions of the picture) of everything the model calls a person: from its "locate the objects" answer, which reads
/// like `man<loc_125><loc_300><loc_509><loc_998>woman<loc_462>...` (four numbers per box, 0..999, top-left then bottom-right).
fn person_boxes(text: &str) -> Vec<[f32; 4]> {
    let mut out = Vec::new();
    let mut label = String::new();
    let mut nums: Vec<f32> = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("<loc_") {
            let end = r.find('>').unwrap_or(r.len());
            nums.push(r[..end].parse::<f32>().unwrap_or(0.0) / 1000.0);
            rest = &r[(end + 1).min(r.len())..];
            if nums.len() == 4 {
                let l = label.to_lowercase();
                if ["person", "man", "woman", "boy", "girl", "child", "people", "human", "lady", "guy", "bride", "groom", "baby", "kid"].iter().any(|w| l.contains(w)) {
                    out.push([nums[0], nums[1], nums[2], nums[3]]);
                }
                nums.clear();
                // several boxes can follow one label; a new label starts at the next non-`<loc_` character
            }
        } else {
            if !nums.is_empty() { nums.clear(); }
            let next = rest.find("<loc_").unwrap_or(rest.len());
            // a label directly after a finished box replaces the previous one
            if rest.as_bytes().first().map_or(false, |_| true) && (label.is_empty() || !rest[..next].trim().is_empty()) {
                label = rest[..next].trim().to_string();
            }
            rest = &rest[next..];
        }
    }
    out
}

/// Drops the "A picture of ..." and "blurry" the model likes to open with (they describe the photo, not the place; the blur is our own
/// doing where a person was painted over) and the full stop.
fn tidy(sentence: &str) -> String {
    let mut s = sentence.trim().trim_end_matches('.').replace("blurry ", "").replace("Blurry ", "");
    for _ in 0..3 {
        let lower = s.to_lowercase();
        let cut = ["a series of pictures of ", "a series of photos of ", "a series of photos showing ", "a picture of ", "a photo of ", "a painting of ", "a close up of ", "a close-up of ", "an image of ", "a collage of "]
            .iter().find(|p| lower.starts_with(*p)).map(|p| p.len());
        match cut { Some(n) => s = s[n..].to_string(), None => break }
    }
    let mut c = s.chars();
    match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() }
}

/// The short form of a description, for a label: the main thing and where it is, without the extra clauses
/// ("A large white building with a steeple on top of it" becomes "A large white building"). People were painted out before the
/// sentence was written, so when a clause still talks about one ("A person's face and water") the next clause is used.
pub fn short(sentence: &str) -> String {
    let mut s = sentence.trim().trim_end_matches('.').to_string();
    for tail in [" on top of it", " in the background", " of it"] {
        s = s.replace(tail, "");
    }
    let mut chunks = vec![s.as_str()];
    for sep in [" with ", " and ", " that ", " which ", ", ", " next to ", " while "] {
        chunks = chunks.into_iter().flat_map(|c| c.split(sep)).collect();
    }
    let about_people = |c: &str| {
        let l = c.to_lowercase();
        l.split(|ch: char| !ch.is_alphabetic()).any(|w| matches!(w, "person" | "people" | "man" | "men" | "woman" | "women" | "face" | "faces" | "girl" | "boy" | "child" | "couple" | "lady" | "human"))
    };
    let pick = chunks.iter().find(|c| !c.trim().is_empty() && !about_people(c)).or(chunks.first()).copied().unwrap_or("");
    let words: Vec<&str> = pick.split_whitespace().collect();
    let out = words[..words.len().min(8)].join(" ");
    let mut c = out.chars();
    match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() }
}

/// Whether a short description names a place rather than a single object ("A table" is not a scene; "A large white building" and
/// "A garden" are). Bare objects make poor labels, so they are left out.
pub fn place_like(short: &str) -> bool {
    const PLACES: &[&str] = &[
        "garden", "street", "room", "forest", "woods", "beach", "field", "fields", "park", "city", "town", "village", "road", "lake", "river", "sea", "ocean",
        "mountain", "mountains", "hill", "hills", "valley", "kitchen", "bedroom", "restaurant", "cafe", "church", "building", "buildings", "house", "bridge",
        "desert", "island", "pool", "stage", "market", "shop", "store", "station", "airport", "hall", "yard", "farm", "meadow", "lawn", "path", "trail",
        "temple", "castle", "palace", "courtyard", "square", "plaza", "harbor", "harbour", "coast", "shore", "waterfall", "cave", "bar", "gym", "office",
        "classroom", "library", "museum", "stadium", "playground", "campsite", "tent", "stairs", "staircase", "sky", "sunset", "snow",
    ];
    let words: Vec<String> = short.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect();
    words.len() >= 3 || words.iter().any(|w| PLACES.contains(&w.as_str()))
}

impl Captioner {
    /// A sentence about the place, not the people: the people are found first and painted over with the picture's own average
    /// colour, so the caption has nothing left to say about them.
    pub fn scene(&self, img: &DynamicImage) -> Result<String, String> {
        let raw = self.ask_raw(img, "Locate the objects with category name in the image.", 120, false)?;
        let boxes = person_boxes(&raw);
        let mut rgb = img.to_rgb8();
        if !boxes.is_empty() {
            let (w, h) = (rgb.width() as f32, rgb.height() as f32);
            // paint each person over with a very soft blur of the whole picture: its colours and light stay, so the place around
            // them still reads as a place, but there is no person left to describe
            let soft = image::imageops::blur(&image::imageops::resize(&rgb, rgb.width() / 4, rgb.height() / 4, FilterType::Triangle), 6.0);
            let soft = image::imageops::resize(&soft, rgb.width(), rgb.height(), FilterType::Triangle);
            for b in &boxes {
                let (x0, x1, y0, y1) = ((b[0] * w) as u32, ((b[2] * w) as u32).min(rgb.width()), (b[1] * h) as u32, ((b[3] * h) as u32).min(rgb.height()));
                for y in y0..y1 { for x in x0..x1 { rgb.put_pixel(x, y, *soft.get_pixel(x, y)); } }
            }
        }
        self.ask(&DynamicImage::ImageRgb8(rgb), PROMPT, MAX_TOKENS).map(|c| tidy(&c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objects_are_not_places() {
        assert!(!place_like("A table") && !place_like("Water") && !place_like(""));
        assert!(place_like("A garden") && place_like("A large white building") && place_like("Pink flowers in a field"));
    }

    #[test]
    fn short_keeps_the_main_thing() {
        assert_eq!(short("A large white building with a steeple on top of it"), "A large white building");
        assert_eq!(short("Pink flowers in a field"), "Pink flowers in a field");
        assert_eq!(short("A street with a few benches and a street sign"), "A street");
        assert_eq!(short("A very long description of a place that goes on and on"), "A very long description of a place");
        assert_eq!(short("A person's face and water in the background"), "Water");
        assert_eq!(short("A woman sitting in a chair"), "A woman sitting in a chair");
    }

    #[test]
    fn tidy_keeps_the_place() {
        assert_eq!(tidy("A blurry picture of pink flowers in a field."), "Pink flowers in a field");
        assert_eq!(tidy("A series of pictures of a garden with a bunch of plants."), "A garden with a bunch of plants");
        assert_eq!(tidy("A street with a sign on the side of it."), "A street with a sign on the side of it");
    }

    #[test]
    fn person_boxes_are_read_from_the_answer() {
        let boxes = person_boxes("flower<loc_1><loc_2><loc_3><loc_4>human face<loc_10><loc_20><loc_30><loc_40>man<loc_100><loc_200><loc_500><loc_998>woman<loc_0><loc_0><loc_500><loc_500><loc_600><loc_600><loc_700><loc_700>table<loc_0><loc_678><loc_209><loc_998>");
        assert_eq!(boxes.len(), 4);
        assert!((boxes[1][0] - 0.1).abs() < 1e-6 && (boxes[1][3] - 0.998).abs() < 1e-6);
        assert!(person_boxes("building<loc_1><loc_2><loc_3><loc_4>").is_empty());
    }

    #[test]
    #[ignore = "needs models/ with the scene-text pack and FLO_IMGS (a folder of jpgs)"]
    fn describes_scenes() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models");
        let c = Captioner::load(&dir).unwrap();
        let mut files: Vec<_> = std::fs::read_dir(std::env::var("FLO_IMGS").unwrap()).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().map_or(false, |x| x == "jpg")).collect();
        files.sort();
        for f in files.iter().take(24) {
            let t = std::time::Instant::now();
            println!("{} [{:?}] {:?}", f.file_name().unwrap().to_string_lossy(), t.elapsed(), c.scene(&image::open(f).unwrap()));
        }
    }
}
