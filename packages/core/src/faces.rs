//! Telling people apart. The system vision signal (Apple Vision on macOS, see `platform.rs`) finds each face and its five key points; a small face-recognition
//! network (SFace, 128 numbers per face) turns the aligned face into a vector where the same person lands close together;
//! then all faces in a project are grouped greedily by how alike those vectors are. Nothing here names anybody: a
//! "person" is just a cluster, numbered from the one with the most photos.
//!
//! The network is a plain 112x112 ArcFace-style embedder, so any model with that interface can replace the file.

use std::path::Path;
use std::sync::Mutex;

use image::{DynamicImage, RgbImage};
use ort::session::Session;
use ort::value::Tensor;

use crate::analyze::Photo;
use crate::modelfiles;

pub const MODEL_FILE: &str = "face_recognition_sface_2021dec.onnx";
const SIZE: u32 = 112;
/// Where the five points sit in the 112x112 crop the network was trained on: left eye, right eye, nose, left and right
/// mouth corner (left/right as seen in the picture).
const TEMPLATE: [[f32; 2]; 5] = [[38.2946, 51.6963], [73.5318, 51.5014], [56.0252, 71.7366], [41.5493, 92.3655], [70.7299, 92.2041]];
/// A face shorter than this (as a share of the picture's longer side) is too few pixels to recognise; it is skipped.
pub const MIN_FACE: f32 = 0.06;
/// A face joins the person whose average face it is at least this close to (cosine similarity).
const SAME_PERSON: f32 = 0.42;
/// Two people whose average faces are this close are one person seen at different angles.
const MERGE: f32 = 0.38;
/// A "person" has to show up in at least this many photos, so a passer-by does not get an avatar of their own.
const MIN_PHOTOS: usize = 2;

/// One face as Apple Vision reports it: box `[x, y, w, h]` and the five points, all top-left origin, normalised to the image.
#[derive(Clone, Copy, Debug)]
pub struct FaceMark {
    pub bbox: [f32; 4],
    pub pts: [[f32; 2]; 5],
}

impl FaceMark {
    pub fn from_row(r: &[f32; 14]) -> Self {
        let mut pts = [[0f32; 2]; 5];
        for (i, p) in pts.iter_mut().enumerate() {
            *p = [r[4 + i * 2], r[5 + i * 2]];
        }
        FaceMark { bbox: [r[0], r[1], r[2], r[3]], pts }
    }
}

pub struct Face {
    pub bbox: [f32; 4],
    /// Unit length.
    pub emb: Vec<f32>,
}

pub fn available(dir: &Path) -> bool {
    modelfiles::has_model(dir, MODEL_FILE)
}

pub struct FaceEmbedder {
    session: Mutex<Session>,
}

impl FaceEmbedder {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let mut b = Session::builder().map_err(|e| e.to_string())?;
        let session = b.commit_from_file(dir.join(MODEL_FILE)).map_err(|e| e.to_string())?;
        Ok(Self { session: Mutex::new(session) })
    }

    /// An embedding for every mark that is big enough to recognise. Faces the network cannot read are left out.
    pub fn embed(&self, img: &DynamicImage, marks: &[FaceMark]) -> Vec<Face> {
        let rgb = img.to_rgb8();
        let (w, h) = (rgb.width() as f32, rgb.height() as f32);
        marks
            .iter()
            .filter(|m| m.bbox[3] * h >= MIN_FACE * w.max(h))
            .filter_map(|m| {
                let pts: Vec<[f32; 2]> = m.pts.iter().map(|p| [p[0] * w, p[1] * h]).collect();
                let crop = align(&rgb, &pts)?;
                let emb = self.run(&crop)?;
                Some(Face { bbox: m.bbox, emb })
            })
            .collect()
    }

    fn run(&self, crop: &RgbImage) -> Option<Vec<f32>> {
        // RGB, planar, 0..255 as they are: what OpenCV's own recogniser feeds this network.
        let n = (SIZE * SIZE) as usize;
        let mut data = vec![0f32; 3 * n];
        for (i, p) in crop.pixels().enumerate() {
            for c in 0..3 {
                data[c * n + i] = p[c] as f32;
            }
        }
        let input = Tensor::from_array(([1usize, 3usize, SIZE as usize, SIZE as usize], data)).ok()?;
        let mut s = self.session.lock().ok()?;
        let out = s.run(ort::inputs![input]).ok()?;
        let (_, v) = out[0].try_extract_tensor::<f32>().ok()?;
        normalise(v)
    }
}

fn normalise(v: &[f32]) -> Option<Vec<f32>> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    (n > 1e-6 && v.iter().all(|x| x.is_finite())).then(|| v.iter().map(|x| x / n).collect())
}

/// The face rotated, scaled and cropped to the 112x112 frame the network expects: the least-squares similarity
/// transform taking the five key points onto `TEMPLATE`, then a bilinear resample.
pub fn align(img: &RgbImage, pts: &[[f32; 2]]) -> Option<RgbImage> {
    let n = pts.len() as f32;
    let ms = pts.iter().fold([0f32; 2], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
    let md = TEMPLATE.iter().fold([0f32; 2], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
    // complex-number form of the 2D similarity fit: d = c * s + t
    let (mut num_re, mut num_im, mut den) = (0f32, 0f32, 0f32);
    for (s, d) in pts.iter().zip(TEMPLATE.iter()) {
        let (sx, sy, dx, dy) = (s[0] - ms[0], s[1] - ms[1], d[0] - md[0], d[1] - md[1]);
        num_re += sx * dx + sy * dy;
        num_im += sx * dy - sy * dx;
        den += sx * sx + sy * sy;
    }
    if den < 1e-3 {
        return None;
    }
    let (a, b) = (num_re / den, num_im / den);
    let scale2 = a * a + b * b;
    if scale2 < 1e-9 {
        return None;
    }
    let mut out = RgbImage::new(SIZE, SIZE);
    for v in 0..SIZE {
        for u in 0..SIZE {
            // invert: s = (d - md) / c + ms
            let (dx, dy) = (u as f32 - md[0], v as f32 - md[1]);
            let sx = (dx * a + dy * b) / scale2 + ms[0];
            let sy = (dy * a - dx * b) / scale2 + ms[1];
            out.put_pixel(u, v, sample(img, sx, sy));
        }
    }
    Some(out)
}

fn sample(img: &RgbImage, x: f32, y: f32) -> image::Rgb<u8> {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let px = |xx: i32, yy: i32| img.get_pixel(xx.clamp(0, w - 1) as u32, yy.clamp(0, h - 1) as u32).0;
    let (a, b, c, d) = (px(x0 as i32, y0 as i32), px(x0 as i32 + 1, y0 as i32), px(x0 as i32, y0 as i32 + 1), px(x0 as i32 + 1, y0 as i32 + 1));
    let mut o = [0u8; 3];
    for i in 0..3 {
        let top = a[i] as f32 * (1.0 - fx) + b[i] as f32 * fx;
        let bot = c[i] as f32 * (1.0 - fx) + d[i] as f32 * fx;
        o[i] = (top * (1.0 - fy) + bot * fy).round().clamp(0.0, 255.0) as u8;
    }
    image::Rgb(o)
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

struct Cluster {
    sum: Vec<f32>,
    centre: Vec<f32>,
    members: Vec<(usize, usize)>,
}

impl Cluster {
    fn new(emb: &[f32], at: (usize, usize)) -> Self {
        Cluster { sum: emb.to_vec(), centre: emb.to_vec(), members: vec![at] }
    }
    fn add(&mut self, emb: &[f32], at: (usize, usize)) {
        self.sum.iter_mut().zip(emb).for_each(|(s, e)| *s += e);
        self.centre = normalise(&self.sum).unwrap_or_else(|| self.centre.clone());
        self.members.push(at);
    }
    fn photos(&self) -> usize {
        let mut p: Vec<usize> = self.members.iter().map(|m| m.0).collect();
        p.sort_unstable();
        p.dedup();
        p.len()
    }
}

/// Groups every embedded face in the project into people and writes each face's person number into `Photo::people`
/// (`-1` = not a recurring person). Number 0 is the person in the most photos. Bigger, clearer faces are placed first,
/// so a cluster's average is built from the faces that are easiest to trust.
pub fn assign_people(photos: &mut [Photo]) {
    for p in photos.iter_mut() {
        p.people = vec![-1; p.face_boxes.len()];
    }
    let mut faces: Vec<(usize, usize, f32)> = Vec::new();
    for (pi, p) in photos.iter().enumerate() {
        for (fi, e) in p.face_embs.iter().enumerate() {
            if !e.is_empty() && fi < p.face_boxes.len() {
                let b = p.face_boxes[fi];
                faces.push((pi, fi, b[2] * b[3]));
            }
        }
    }
    faces.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut clusters: Vec<Cluster> = Vec::new();
    for &(pi, fi, _) in &faces {
        let emb = &photos[pi].face_embs[fi];
        let best = clusters.iter().enumerate().map(|(i, c)| (i, dot(&c.centre, emb))).max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        match best {
            Some((i, sim)) if sim >= SAME_PERSON => clusters[i].add(emb, (pi, fi)),
            _ => clusters.push(Cluster::new(emb, (pi, fi))),
        }
    }
    // The same person at another angle can seed a second cluster; fold those back together. Clusters with more than one face are
    // joined in one pass (a union-find over their centres); the single faces, by far the most numerous in a crowd, are then attached
    // to whichever joined cluster is close enough, so the work stays linear in them instead of quadratic in everything.
    let (mut multi, singles): (Vec<Cluster>, Vec<Cluster>) = clusters.into_iter().partition(|c| c.members.len() > 1);
    let mut root: Vec<usize> = (0..multi.len()).collect();
    fn find(root: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while root[i] != i {
            root[i] = root[root[i]];
            i = root[i];
        }
        i
    }
    for i in 0..multi.len() {
        for j in i + 1..multi.len() {
            if dot(&multi[i].centre, &multi[j].centre) >= MERGE {
                let (a, b) = (find(&mut root, i), find(&mut root, j));
                if a != b {
                    root[b] = a;
                }
            }
        }
    }
    let mut joined: Vec<Cluster> = Vec::new();
    let mut slot: Vec<Option<usize>> = vec![None; multi.len()];
    for i in 0..multi.len() {
        let r = find(&mut root, i);
        let members = std::mem::take(&mut multi[i].members);
        match slot[r] {
            Some(k) => {
                for m in members {
                    joined[k].add(&photos[m.0].face_embs[m.1], m);
                }
            }
            None => {
                let mut c = Cluster::new(&photos[members[0].0].face_embs[members[0].1], members[0]);
                for m in &members[1..] {
                    c.add(&photos[m.0].face_embs[m.1], *m);
                }
                slot[r] = Some(joined.len());
                joined.push(c);
            }
        }
    }
    let mut clusters = joined;
    for s in singles {
        let m = s.members[0];
        let emb = &photos[m.0].face_embs[m.1];
        let best = clusters.iter().enumerate().map(|(i, c)| (i, dot(&c.centre, emb))).max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        match best {
            Some((i, sim)) if sim >= MERGE => clusters[i].add(emb, m),
            _ => clusters.push(s),
        }
    }
    clusters.sort_by_cached_key(|c| std::cmp::Reverse(c.photos()));
    let mut next = 0i32;
    for c in clusters.iter().filter(|c| c.photos() >= MIN_PHOTOS) {
        for &(pi, fi) in &c.members {
            photos[pi].people[fi] = next;
        }
        next += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photo_with(faces: &[(f32, &[f32])]) -> Photo {
        Photo {
            face_boxes: faces.iter().map(|(s, _)| [0.0, 0.0, *s, *s]).collect(),
            face_embs: faces.iter().map(|(_, e)| normalise(e).unwrap()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn similar_faces_form_one_person_and_a_lone_face_is_nobody() {
        let a1 = [1.0, 0.1, 0.0, 0.0];
        let a2 = [0.9, 0.2, 0.05, 0.0];
        let b1 = [0.0, 0.1, 1.0, 0.0];
        let b2 = [0.05, 0.0, 0.9, 0.2];
        let stranger = [0.0, 1.0, 0.0, 0.0];
        let mut photos = vec![photo_with(&[(0.3, &a1), (0.2, &b1)]), photo_with(&[(0.3, &a2)]), photo_with(&[(0.3, &b2), (0.1, &stranger)]), photo_with(&[(0.3, &a1)])];
        assign_people(&mut photos);
        assert_eq!(photos[0].people[0], 0, "a is in three photos, so is person 0");
        assert_eq!(photos[1].people[0], 0);
        assert_eq!(photos[3].people[0], 0);
        assert_eq!(photos[0].people[1], 1, "b is in two photos, so is person 1");
        assert_eq!(photos[2].people[0], 1);
        assert_eq!(photos[2].people[1], -1, "a face that appears once is not a recurring person");
    }

    #[test]
    fn alignment_maps_the_template_onto_itself() {
        let mut img = RgbImage::new(112, 112);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgb([x as u8, y as u8, 7]);
        }
        let pts: Vec<[f32; 2]> = TEMPLATE.to_vec();
        let out = align(&img, &pts).unwrap();
        assert_eq!(out.get_pixel(50, 60).0, [50, 60, 7]);
    }

    #[test]
    #[ignore = "needs models/ and a real photo (set PROBE_IMG); prints similarities between the faces found"]
    fn embeds_faces_on_a_real_photo() {
        let dir = crate::dev_models_dir();
        let m = FaceEmbedder::load(&dir).expect("load");
        let path = std::env::var("PROBE_IMG").expect("set PROBE_IMG=<image>");
        let marks: Vec<f32> = std::env::var("PROBE_MARKS").expect("PROBE_MARKS=comma separated 14 numbers per face").split(',').map(|s| s.trim().parse().unwrap()).collect();
        let marks: Vec<FaceMark> = marks.chunks(14).map(|c| FaceMark::from_row(&c.try_into().unwrap())).collect();
        let faces = m.embed(&image::open(path).unwrap(), &marks);
        for (i, a) in faces.iter().enumerate() {
            for (j, b) in faces.iter().enumerate().skip(i + 1) {
                println!("{i} vs {j}: {:.3}", dot(&a.emb, &b.emb));
            }
        }
    }
}
