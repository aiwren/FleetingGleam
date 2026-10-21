//! A colour signature per photo, made from its thumbnail when the search index is built. Image-text models are weak at "red background"
//! or "blue dress" (colours are attributes, not objects), while measuring colour is exact. So words about colour are answered by
//! measuring: how much of the whole picture, or of its border (the background), is that colour.

use image::DynamicImage;

/// Colour categories, in the order they are stored.
pub const NAMES: [&str; 10] = ["red", "orange", "yellow", "green", "cyan", "blue", "purple", "pink", "white", "black"];
const N: usize = NAMES.len();

fn category(r: f32, g: f32, b: f32) -> Option<usize> {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let v = max;
    let s = if max <= 0.0 { 0.0 } else { (max - min) / max };
    if v < 0.16 { return Some(9); } // black
    if s < 0.16 { return if v > 0.78 { Some(8) } else { None }; } // white; grey is left out
    if s < 0.30 || v < 0.20 { return None; } // washed out or muddy: no clear colour
    let d = max - min;
    let mut h = if max == r { ((g - b) / d) % 6.0 } else if max == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 } * 60.0;
    if h < 0.0 { h += 360.0; }
    Some(match h {
        h if h < 12.0 || h >= 345.0 => 0,
        h if h < 40.0 => 1,
        h if h < 68.0 => 2,
        h if h < 165.0 => 3,
        h if h < 200.0 => 4,
        h if h < 255.0 => 5,
        h if h < 300.0 => 6,
        _ => 7,
    })
}

/// 2 * N bytes: the fraction (0..255) of the picture in each colour, then the same for its border band.
pub fn signature(img: &DynamicImage) -> Vec<u8> {
    const S: u32 = 48;
    let small = img.resize_exact(S, S, image::imageops::FilterType::Triangle).to_rgb8();
    let mut all = [0u32; N];
    let mut edge = [0u32; N];
    let (mut n_all, mut n_edge) = (0u32, 0u32);
    let band = (S as f32 * 0.2) as u32;
    for (x, y, p) in small.enumerate_pixels() {
        let on_edge = x < band || x >= S - band || y < band || y >= S - band;
        n_all += 1;
        if on_edge { n_edge += 1; }
        if let Some(c) = category(p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0) {
            all[c] += 1;
            if on_edge { edge[c] += 1; }
        }
    }
    let f = |c: u32, n: u32| ((c as f32 / n.max(1) as f32) * 255.0).round() as u8;
    all.iter().map(|&c| f(c, n_all)).chain(edge.iter().map(|&c| f(c, n_edge))).collect()
}

/// Fraction (0..1) of a colour in the whole picture or in its border.
pub fn fraction(sig: &[u8], colour: usize, background: bool) -> f32 {
    sig.get(colour + if background { N } else { 0 }).map_or(0.0, |&b| b as f32 / 255.0)
}

/// Colour words in a query. Returns the colours asked for, whether the query says "background" (底 / 背景), and the query without those words.
pub fn parse_query(q: &str) -> (Vec<usize>, bool, String) {
    let words: [(&[&str], usize); 10] = [
        (&["红色", "红", "red"], 0), (&["橙色", "橙", "orange"], 1), (&["黄色", "黄", "yellow"], 2), (&["绿色", "绿", "green"], 3),
        (&["青色", "青", "cyan", "teal"], 4), (&["蓝色", "蓝", "blue"], 5), (&["紫色", "紫", "purple"], 6), (&["粉色", "粉", "pink"], 7),
        (&["白色", "白", "white"], 8), (&["黑色", "黑", "black"], 9),
    ];
    let mut rest = q.to_lowercase();
    let mut found = Vec::new();
    for (ws, idx) in words {
        for w in ws {
            // English words must stand alone; Chinese ones may sit inside a phrase ("红底", "蓝色的裙子")
            let hit = if w.is_ascii() { rest.split(|c: char| !c.is_ascii_alphabetic()).any(|t| t == *w) } else { rest.contains(w) };
            if hit {
                if !found.contains(&idx) { found.push(idx); }
                rest = if w.is_ascii() { rest.replace(w, " ") } else { rest.replacen(w, " ", 1) };
                break;
            }
        }
    }
    let mut background = false;
    for w in ["背景色", "背景", "底色", "底", "background", "backdrop"] {
        if !found.is_empty() && rest.contains(w) {
            background = true;
            rest = rest.replace(w, " ");
        }
    }
    let rest = rest.replace(['的', '，', ','], " ");
    (found, background, rest.split_whitespace().collect::<Vec<_>>().join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_words() {
        assert_eq!(parse_query("红底合照"), (vec![0], true, "合照".to_string()));
        assert_eq!(parse_query("蓝色的裙子"), (vec![5], false, "裙子".to_string()));
        assert_eq!(parse_query("a red umbrella").0, vec![0]);
        assert_eq!(parse_query("烟花"), (vec![], false, "烟花".to_string()));
        assert_eq!(parse_query("绿色背景").1, true);
    }

    #[test]
    fn signature_of_a_red_frame() {
        let img = image::RgbImage::from_fn(96, 96, |x, y| if x < 20 || x > 76 || y < 20 || y > 76 { image::Rgb([190, 30, 40]) } else { image::Rgb([240, 240, 240]) });
        let sig = signature(&DynamicImage::ImageRgb8(img));
        assert!(fraction(&sig, 0, true) > 0.9, "border is red");
        assert!(fraction(&sig, 8, false) > 0.3, "the middle is white");
        assert!(fraction(&sig, 0, false) < 0.7);
    }
}
