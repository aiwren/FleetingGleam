//! English to Chinese for the one-line scene descriptions, with the opus-mt-en-zh model (Helsinki-NLP, Apache-2.0) running on this machine.
//! The tokenizer file is the model's own with one change: its text-normalisation step (a table the `tokenizers` crate cannot read)
//! is left out, which does not matter for plain English sentences.

use std::path::Path;
use std::sync::Mutex;

use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

use crate::modelfiles;

pub const ENCODER: &str = "opusmt_encoder_model_int8.onnx";
pub const DECODER: &str = "opusmt_decoder_model_merged_int8.onnx";
pub const TOKENIZER: &str = "opusmt_tokenizer.json";
pub const FILES: [&str; 3] = [ENCODER, DECODER, TOKENIZER];

const HIDDEN: usize = 512;
const LAYERS: usize = 6;
const HEADS: usize = 8;
const HEAD_DIM: usize = 64;
/// The decoder starts from the padding token, which it must never write.
const START: i64 = 65000;
const EOS: i64 = 0;
const MAX_TOKENS: usize = 64;

fn session(dir: &Path, name: &str) -> Result<Session, String> {
    // Two threads each: describing scenes runs while the app is in use and must not take every core.
    let mut b = Session::builder().map_err(|e| e.to_string())?.with_intra_threads(2).map_err(|e| e.to_string())?;
    b.commit_from_file(dir.join(name)).map_err(|e| e.to_string())
}

pub struct Translator {
    encoder: Mutex<Session>,
    decoder: Mutex<Session>,
    tokenizer: Tokenizer,
}

impl Translator {
    pub fn available(dir: &Path) -> bool {
        FILES.iter().all(|n| modelfiles::has_model(dir, n))
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let tokenizer = Tokenizer::from_bytes(modelfiles::read_model(dir, TOKENIZER)?).map_err(|e| e.to_string())?;
        Ok(Self { encoder: Mutex::new(session(dir, ENCODER)?), decoder: Mutex::new(session(dir, DECODER)?), tokenizer })
    }

    /// The Chinese for a short English sentence (greedy decoding).
    /// The Chinese for a short English phrase. This small model sometimes stutters on very short input ("花园园", "街街道"), so the
    /// phrase is tried in a few spellings and the first answer that reads cleanly wins.
    pub fn zh(&self, text: &str) -> Result<String, String> {
        let text = text.trim();
        let lower = text.to_lowercase();
        let bare = ["a ", "an ", "the "].iter().find_map(|a| lower.strip_prefix(a)).unwrap_or(&lower).to_string();
        let tries = [text.to_string(), format!("{text}."), bare.clone(), format!("{bare}.")];
        let mut first = None;
        for t in tries {
            let zh = self.translate(&t)?;
            if clean(&zh) { return Ok(zh); }
            first.get_or_insert(zh);
        }
        Ok(first.unwrap_or_default())
    }

    fn translate(&self, text: &str) -> Result<String, String> {
        let enc = self.tokenizer.encode(text, true).map_err(|e| e.to_string())?;
        let src: Vec<i64> = enc.get_ids().iter().map(|&x| x as i64).collect();
        let n = src.len();
        let hidden: Vec<f32> = {
            let ids = Tensor::from_array(([1usize, n], src)).map_err(|e| e.to_string())?;
            let mask = Tensor::from_array(([1usize, n], vec![1i64; n])).map_err(|e| e.to_string())?;
            let mut s = self.encoder.lock().map_err(|e| e.to_string())?;
            let out = s.run(ort::inputs!["input_ids" => ids, "attention_mask" => mask]).map_err(|e| e.to_string())?;
            let (_, d) = out["last_hidden_state"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
            d.to_vec()
        };
        let mut ids: Vec<i64> = vec![START];
        for _ in 0..MAX_TOKENS {
            let mut inputs: Vec<(std::borrow::Cow<'static, str>, ort::session::SessionInputValue<'static>)> = vec![
                ("encoder_attention_mask".into(), Tensor::from_array(([1usize, n], vec![1i64; n])).map_err(|e| e.to_string())?.into()),
                ("encoder_hidden_states".into(), Tensor::from_array(([1usize, n, HIDDEN], hidden.clone())).map_err(|e| e.to_string())?.into()),
                ("input_ids".into(), Tensor::from_array(([1usize, ids.len()], ids.clone())).map_err(|e| e.to_string())?.into()),
                ("use_cache_branch".into(), Tensor::from_array(([1usize], vec![false])).map_err(|e| e.to_string())?.into()),
            ];
            // no cache between steps: empty past tensors, and the whole sentence so far is read again (cheap for a sentence)
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
                // never write a pair of tokens twice: the small model otherwise stutters ("花花花花")
                let prev = *ids.last().unwrap();
                let banned: Vec<i64> = ids.windows(2).filter(|w| w[0] == prev).map(|w| w[1]).collect();
                last.iter().enumerate().filter(|(i, _)| *i as i64 != START && !banned.contains(&(*i as i64)) && *i as i64 != prev).max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).map(|(i, _)| i as i64).ok_or("no logits")?
            };
            if next == EOS { break; }
            ids.push(next);
        }
        let words: Vec<u32> = ids.iter().skip(1).map(|&x| x as u32).collect();
        Ok(self.tokenizer.decode(&words, true).map_err(|e| e.to_string())?.replace(' ', "").trim_end_matches(|c| matches!(c, '。' | '.' | '，' | ',' | '！' | '!')).to_string())
    }
}

/// A translation that reads like a phrase: Chinese only, no character doubled next to itself and no two-character run repeated.
fn clean(zh: &str) -> bool {
    let c: Vec<char> = zh.chars().collect();
    !c.is_empty()
        && c.iter().all(|ch| !ch.is_ascii_alphabetic())
        && c.windows(2).all(|w| w[0] != w[1])
        && !c.windows(4).any(|w| w[0] == w[2] && w[1] == w[3])
        && !c.windows(3).any(|w| w[0] == w[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stutter_is_not_clean() {
        assert!(clean("湖前的树") && clean("白色大拱门"));
        assert!(!clean("花园园") && !clean("街街道") && !clean("A会议室A会议室") && !clean("花园园花园") && !clean(""));
    }

    #[test]
    #[ignore = "needs models/ with the scene-text pack"]
    fn translates_a_scene() {
        let t = Translator::load(&crate::dev_models_dir()).unwrap();
        let zh = t.zh("A large white building with a steeple on top of it.").unwrap();
        println!("{zh}");
        assert!(zh.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)));
    }

    #[test]
    #[ignore = "needs models/ with the scene-text pack"]
    fn short_scene_phrases() {
        let t = Translator::load(&crate::dev_models_dir()).unwrap();
        for e in ["A large sculpture of a tree", "A large white building", "Pink flowers in a field", "A field of flowers", "A table and chairs", "A large white archway", "A tree in front of a lake", "A person sitting in front of a tree", "A large beer in front of a city", "A rectangular sky in front of a tree", "A street", "A large balloon sculpture in front of a building", "A garden", "A room"] {
            println!("{e} => {}", t.zh(e).unwrap());
        }
    }
}
