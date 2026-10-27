//! What kind of picture is this? Decided from the CLIP embedding the photo already has, by comparing it with a few descriptions
//! ("a close-up portrait photo of a person", "a landscape photo of nature", ...) whose embeddings were computed once, at build time,
//! with CLIP's own text model. Nothing is trained and no text model has to run on the user's machine.
//! The kind picks which checks matter most (see scoring.rs): a portrait is judged by the face, a landscape by exposure and overall
//! sharpness, food by colour and look.

pub const NAMES: [&str; 14] = ["portrait", "people", "landscape", "architecture", "food", "animal", "object", "document", "street", "night", "sport", "macro", "stage", "vehicle"];

static B32: &[u8] = include_bytes!("scene_b32.bin");
static L14: &[u8] = include_bytes!("scene_l14.bin");

fn rows(bytes: &[u8], dim: usize) -> Vec<Vec<f32>> {
    bytes.chunks_exact(dim * 4).map(|r| r.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()).collect()
}

/// Zero-shot kind of a photo from its (unit-length) analysis embedding, or `None` for an embedding of an unknown size
/// or one with NaN / 无穷大（以前缓存里一条这样的向量，`partial_cmp().unwrap()` 就让每次打开项目都 panic）。
pub fn classify(emb: &[f32]) -> Option<(&'static str, f32)> {
    if emb.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let table = match emb.len() {
        512 => rows(B32, 512),
        768 => rows(L14, 768),
        _ => return None,
    };
    let logits: Vec<f32> = table.iter().map(|t| 100.0 * t.iter().zip(emb).map(|(a, b)| a * b).sum::<f32>()).collect();
    let max = logits.iter().cloned().fold(f32::MIN, f32::max);
    let exp: Vec<f32> = logits.iter().map(|l| (l - max).exp()).collect();
    let sum: f32 = exp.iter().sum();
    let (i, e) = exp.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
    Some((NAMES[i], e / sum))
}

/// Faces settle "portrait or people in a scene?" better than the words do: a landscape-like frame with a person in it is a people
/// picture, and an object or building never has a face to judge.
pub fn with_faces(kind: &'static str, faces: u32) -> &'static str {
    if faces >= 1 && matches!(kind, "landscape" | "architecture" | "object" | "food") { "people" } else { kind }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_have_the_right_shape() {
        assert_eq!(rows(B32, 512).len(), NAMES.len());
        assert_eq!(rows(L14, 768).len(), NAMES.len());
        // every row is a unit vector
        for r in rows(B32, 512).iter().chain(rows(L14, 768).iter()) {
            let n = r.iter().map(|v| v * v).sum::<f32>().sqrt();
            assert!((n - 1.0).abs() < 1e-3);
        }
    }

    #[test]
    fn a_description_classifies_as_itself() {
        // the portrait row of the table is, by construction, a portrait
        let row = rows(B32, 512).remove(0);
        assert_eq!(classify(&row).unwrap().0, "portrait");
        assert!(classify(&[0.0; 10]).is_none());
        let mut bad = rows(B32, 512).remove(0);
        bad[3] = f32::NAN;
        assert!(classify(&bad).is_none(), "NaN 不 panic");
        bad[3] = f32::INFINITY;
        assert!(classify(&bad).is_none());
        assert_eq!(with_faces("landscape", 1), "people");
        assert_eq!(with_faces("animal", 1), "animal");
    }
}
