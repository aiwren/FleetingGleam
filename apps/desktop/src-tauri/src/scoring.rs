//! All scoring constants live here. None of them are calibrated on a labelled set yet:
//! they are starting points, tuned by hand against a handful of images.

/// Weights of the base score.
pub const W_AESTHETIC: f32 = 0.45;
pub const W_SHARP: f32 = 0.35;
pub const W_EXPOSURE: f32 = 0.20;
/// How much the within-group face comparison can move the score.
pub const W_FACE: f32 = 0.20;
/// Apple's aesthetics and our CLIP head are blended when both exist.
pub const W_SYS_IN_AESTHETIC: f32 = 0.6;

/// Maps the raw "sharpest tiles" Laplacian energy (1024px working size) to 0..1 on a log scale.
const SHARP_LO: f32 = 6.0;
const SHARP_HI: f32 = 220.0;

pub fn sharp_score(raw: f32) -> f32 {
    let (lo, hi) = ((1.0 + SHARP_LO).ln(), (1.0 + SHARP_HI).ln());
    (((1.0 + raw.max(0.0)).ln() - lo) / (hi - lo)).clamp(0.0, 1.0)
}

/// Only clipping and near-black frames are penalised, so night scenes are not punished for being dark.
pub fn expo_score(brightness: f32, clipped_high: f32, clipped_low: f32) -> f32 {
    let mut pen = clipped_high * 3.0 + clipped_low * 1.2;
    if brightness > 0.92 || brightness < 0.05 {
        pen += 0.3;
    }
    1.0 - pen.clamp(0.0, 1.0)
}

/// Apple's overallScore is roughly -1..1; real photos sit around 0.3..0.8.
pub fn sys_norm(s: f32) -> f32 {
    (s + 0.2).clamp(0.0, 1.0)
}

pub fn combine_aesthetic(clip: Option<f32>, sys: Option<f32>) -> Option<f32> {
    match (clip, sys) {
        (Some(c), Some(s)) => Some(W_SYS_IN_AESTHETIC * sys_norm(s) + (1.0 - W_SYS_IN_AESTHETIC) * c),
        (Some(c), None) => Some(c),
        (None, Some(s)) => Some(sys_norm(s)),
        (None, None) => None,
    }
}

pub fn flags(sharp: f32, brightness: f32, clipped_high: f32, utility: bool) -> Vec<String> {
    let mut f = Vec::new();
    if sharp < 0.18 {
        f.push("blur".to_string());
    }
    if clipped_high > 0.18 {
        f.push("over".to_string());
    }
    if brightness < 0.05 {
        f.push("under".to_string());
    }
    if utility {
        f.push("utility".to_string());
    }
    f
}

/// Eyes measured below this are treated as closed (about 0.25-0.4 when open; a wide smile can squint down to ~0.18).
pub const EYES_CLOSED: f32 = 0.13;

/// How much each check counts for a kind of picture: (aesthetic, sharpness, exposure, face comparison within a group).
/// Common sense, not measurement: a portrait lives or dies by the face, a landscape by clean exposure and overall sharpness,
/// an animal by whether the subject is in focus, food by how it looks.
pub fn weights(kind: &str) -> (f32, f32, f32, f32) {
    match kind {
        "portrait" => (0.30, 0.30, 0.15, 0.35),
        "people" => (0.40, 0.30, 0.15, 0.20),
        "landscape" => (0.45, 0.25, 0.30, 0.0),
        "architecture" => (0.40, 0.35, 0.25, 0.0),
        "food" => (0.55, 0.30, 0.15, 0.0),
        "animal" => (0.35, 0.45, 0.20, 0.0),
        "object" => (0.45, 0.35, 0.20, 0.0),
        "street" => (0.40, 0.35, 0.25, 0.0),
        "night" => (0.55, 0.30, 0.15, 0.0),
        "sport" => (0.30, 0.50, 0.20, 0.0),
        "macro" => (0.40, 0.45, 0.15, 0.0),
        "stage" => (0.55, 0.30, 0.15, 0.0),
        "vehicle" => (0.40, 0.35, 0.25, 0.0),
        _ => (W_AESTHETIC, W_SHARP, W_EXPOSURE, W_FACE),
    }
}

pub fn final_score(kind: &str, aesthetic: Option<f32>, sharp: f32, expo: f32, face_rel: Option<f32>, flags: &[String]) -> f32 {
    let (wa, ws, we, wf) = weights(kind);
    let mut s = wa * aesthetic.unwrap_or(0.5) + ws * sharp + we * expo;
    if let (Some(f), true) = (face_rel, wf > 0.0) {
        s = (1.0 - wf) * s + wf * f;
    }
    for flag in flags {
        // A night or stage picture is dark on purpose: that is not an exposure fault.
        if matches!(kind, "night" | "stage") && matches!(flag.as_str(), "under") { continue; }
        s *= match flag.as_str() {
            "blur" => 0.6,
            "utility" => 0.3,
            "over" | "under" => 0.85,
            "eyes" => 0.85,
            _ => 1.0,
        };
    }
    s.clamp(0.0, 1.0)
}
