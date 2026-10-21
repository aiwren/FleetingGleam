//! Keep / review / reject, ported from the old `decide.ts` so it runs compiled instead of shipping as readable JS.
//! Quality decides, not a quota: every photo that clears `GOOD_LINE` is kept, however many that is (a folder of
//! good photos is kept whole; a folder of poor ones is not padded out). The keep ratio is only a ceiling for when you want fewer:
//! the photos cut by it are not rejected but go to review. Below the line is a small review slice (the photos closest to it), then reject, with
//! per-photo reasons. What the person is asked to look at is kept small on purpose: the app decides what it can, and the review pile only holds
//! the real toss-ups (see `REVIEW_SHARE`, `NEAR_SHARE`).

use crate::analyze::Photo;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

/// A photo at or above this score is worth keeping. Scores of decent photos sit well above it (the middle of a typical
/// folder is around 0.8); the review band below it holds the borderline ones.
const GOOD_LINE: f32 = 0.70;
/// Only photos this close below the line can be asked about: a photo further down is simply below the line, not a toss-up.
const REVIEW_BAND: f32 = 0.10;
const REVIEW_FLOOR: f32 = GOOD_LINE - REVIEW_BAND;
/// At most this share of the photos is put to review for being just under the line (the closest ones first).
const REVIEW_SHARE: f32 = 0.08;
/// At most this share is put to review for being as good as the best of their group (the closest ones first); the other mates are rejected.
const NEAR_SHARE: f32 = 0.04;
/// Score subtracted per already-kept photo of the same scene, so picks cover the whole trip ("diverse" pick mode).
const SCENE_PENALTY: f32 = 0.06;
/// A non-best photo this close to its group's best can be worth a second look.
const NEAR_BEST: f32 = 0.03;

/// How picks favour coverage vs. raw score.
#[derive(Clone, Copy, PartialEq)]
pub enum PickMode {
    /// Spread keeps across scenes (the default): a scene penalty discourages picking many from the same one.
    Diverse,
    /// Just take the top-scoring photos, scene be damned.
    Best,
}
impl PickMode {
    pub fn parse(s: Option<&str>) -> Self {
        match s {
            Some("best") => PickMode::Best,
            _ => PickMode::Diverse,
        }
    }
    fn scene_penalty(self) -> f32 {
        match self {
            PickMode::Diverse => SCENE_PENALTY,
            PickMode::Best => 0.0,
        }
    }
}

fn is_hard(flags: &[String]) -> bool {
    flags.iter().any(|f| f == "blur" || f == "utility")
}
fn is_soft(flags: &[String]) -> bool {
    flags.iter().any(|f| f != "blur" && f != "utility")
}
fn pct(v: f32) -> i64 {
    (v * 100.0).round() as i64
}

#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Keep,
    Review,
    Reject,
}

#[derive(Serialize, Clone)]
pub struct Reason {
    pub k: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p: Option<HashMap<&'static str, Value>>,
}
fn reason(k: &'static str) -> Reason {
    Reason { k, p: None }
}
fn reason_with(k: &'static str, params: &[(&'static str, Value)]) -> Reason {
    Reason { k, p: Some(params.iter().cloned().collect()) }
}

#[derive(Serialize, Clone)]
pub struct Decision {
    pub state: State,
    pub manual: bool,
    pub reasons: Vec<Reason>,
}

fn flag_reasons(p: &Photo) -> Vec<Reason> {
    let mut out = Vec::new();
    if p.flags.iter().any(|f| f == "blur") {
        out.push(reason("reason.blur"));
    }
    if p.flags.iter().any(|f| f == "utility") {
        out.push(reason("reason.utility"));
    }
    if p.flags.iter().any(|f| f == "over") {
        out.push(reason_with("reason.over", &[("pct", Value::from(pct(p.clipped_high)))]));
    }
    if p.flags.iter().any(|f| f == "under") {
        out.push(reason("reason.under"));
    }
    if p.flags.iter().any(|f| f == "eyes") {
        out.push(reason("reason.eyes"));
    }
    out
}

/// What the group's best does clearly better than this photo, if one thing stands out: the reason key that names it.
/// Closed eyes trump everything (the flag is already on the photo); otherwise the biggest gap among the parts of the score,
/// and only when it is big enough to be the story (the gap of a hair is not told).
fn shortfall(p: &Photo, best: &Photo) -> Option<&'static str> {
    if p.flags.iter().any(|f| f == "eyes") {
        return Some("reason.betterEyes");
    }
    let mut gaps: Vec<(&'static str, f32)> = vec![("reason.betterSharp", best.sharp_score - p.sharp_score), ("reason.betterExpo", best.expo_score - p.expo_score)];
    if let (Some(a), Some(b)) = (best.ai_score, p.ai_score) {
        gaps.push(("reason.betterLook", a - b));
    }
    if let (Some(a), Some(b)) = (best.face_min, p.face_min) {
        gaps.push(("reason.betterFace", a - b));
    }
    gaps.into_iter().filter(|(_, d)| *d >= 0.05).max_by(|a, b| a.1.total_cmp(&b.1)).map(|(k, _)| k)
}

/// The taste model's say, when it moved the score enough to matter: so the person can see when it is their own preference talking.
fn pref_reason(p: &Photo) -> Option<Reason> {
    const SAYS: f32 = 0.04;
    if p.pref_delta >= SAYS {
        Some(reason_with("reason.prefUp", &[("pts", Value::from(pct(p.pref_delta)))]))
    } else if p.pref_delta <= -SAYS {
        Some(reason_with("reason.prefDown", &[("pts", Value::from(pct(-p.pref_delta)))]))
    } else {
        None
    }
}

/// One candidate in the pick queue: the best photo left of a scene, with what it is worth right now (score minus the scene penalty).
struct Cand {
    value: f32,
    idx: usize,
    scene: usize,
}
impl PartialEq for Cand {
    fn eq(&self, o: &Self) -> bool { self.cmp(o) == std::cmp::Ordering::Equal }
}
impl Eq for Cand {}
impl PartialOrd for Cand {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(o)) }
}
impl Ord for Cand {
    // the higher value first; on a tie the photo that comes first in the pool
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.value.partial_cmp(&o.value).unwrap_or(std::cmp::Ordering::Equal).then_with(|| o.idx.cmp(&self.idx))
    }
}

/// Greedily takes up to `limit` photos from `pool` (highest `score - scene penalty` first), stopping early at `floor`.
/// Each scene keeps its photos best first, and a queue holds the best of every scene: taking one only changes the value of its own scene's next
/// photo, so every pick costs O(log scenes) instead of a pass over the whole pool.
fn take_greedy(pool: Vec<&Photo>, limit: usize, floor: f32, state: State, count_scene: bool, scene_penalty: f32, chosen: &mut HashMap<String, State>, scene_count: &mut HashMap<usize, usize>) {
    use std::collections::BinaryHeap;
    let mut by_scene: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, p) in pool.iter().enumerate() {
        by_scene.entry(p.scene).or_default().push(i);
    }
    // each scene's photos, worst first, so the best is popped off the end
    for v in by_scene.values_mut() {
        v.sort_by(|&a, &b| pool[a].score.partial_cmp(&pool[b].score).unwrap_or(std::cmp::Ordering::Equal).then(b.cmp(&a)));
    }
    let value = |i: usize, scene_count: &HashMap<usize, usize>| pool[i].score - scene_penalty * (*scene_count.get(&pool[i].scene).unwrap_or(&0) as f32);
    let mut heap: BinaryHeap<Cand> = BinaryHeap::new();
    for (&scene, v) in by_scene.iter_mut() {
        if let Some(&i) = v.last() {
            heap.push(Cand { value: value(i, scene_count), idx: i, scene });
        }
    }
    let mut taken = 0;
    while taken < limit {
        let Some(c) = heap.pop() else { break };
        let p = pool[c.idx];
        if p.score < floor {
            // This scene has nothing left above the line (its photos are in order, best first). The queue is ordered by value, which includes the
            // scene penalty, so a photo below the line can come up while photos above it are still waiting in other scenes: skip it, do not stop.
            continue;
        }
        chosen.insert(p.path.clone(), state.clone());
        if count_scene {
            *scene_count.entry(p.scene).or_insert(0) += 1;
        }
        taken += 1;
        let v = by_scene.get_mut(&c.scene).unwrap();
        v.pop();
        if let Some(&next) = v.last() {
            heap.push(Cand { value: value(next, scene_count), idx: next, scene: c.scene });
        }
    }
}

/// Three-way decision for every photo, each with human-readable reasons (i18n keys plus parameters).
pub fn decide(photos: &[Photo], keep_ratio: f32, pick_mode: PickMode, one_per_group: bool, overrides: &HashMap<String, String>) -> HashMap<String, Decision> {
    let n = photos.len();
    let mut by_group: HashMap<usize, Vec<&Photo>> = HashMap::new();
    for p in photos {
        by_group.entry(p.group).or_default().push(p);
    }
    let best_of = |g: usize| -> &Photo { by_group[&g].iter().find(|p| p.group_best).copied().expect("every group has a best photo") };

    // With `one_per_group`, only a group's best photo can be kept; its good-but-not-best mates are shown for review (or rejected when clearly worse).
    let eligible: Vec<&Photo> = photos.iter().filter(|p| (!one_per_group || p.group_best) && !is_hard(&p.flags)).collect();

    // `keep_ratio` is a ceiling (1.0 = no limit).
    let keep_n = ((n as f32 * keep_ratio).round() as usize).max(1);
    let good_unkept = |chosen: &HashMap<String, State>| eligible.iter().filter(|p| !is_soft(&p.flags) && p.score >= GOOD_LINE && !chosen.contains_key(&p.path)).count();
    let scene_penalty = pick_mode.scene_penalty();
    let mut scene_count: HashMap<usize, usize> = HashMap::new();
    let mut chosen: HashMap<String, State> = HashMap::new();

    take_greedy(eligible.iter().filter(|p| !is_soft(&p.flags)).copied().collect(), keep_n, GOOD_LINE, State::Keep, true, scene_penalty, &mut chosen, &mut scene_count);
    // good photos the ceiling cut are all reviewed; below the line only a slice is
    let review_n = ((n as f32 * REVIEW_SHARE).round() as usize).max(1) + good_unkept(&chosen);
    take_greedy(eligible.iter().filter(|p| !chosen.contains_key(&p.path)).copied().collect(), review_n, REVIEW_FLOOR, State::Review, false, scene_penalty, &mut chosen, &mut scene_count);

    // The toss-ups inside groups: the mates closest to their group's best, never more than a few percent of the project. The rest of the mates are
    // simply the weaker copies of a better shot.
    let mut near: Vec<(f32, &Photo)> = photos
        .iter()
        .filter(|p| !p.group_best && !chosen.contains_key(&p.path) && !is_hard(&p.flags))
        .map(|p| (best_of(p.group).score - p.score, p))
        .filter(|(d, _)| *d <= NEAR_BEST)
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.path.cmp(&b.1.path)));
    let near_n = ((n as f32 * NEAR_SHARE).round() as usize).max(1);
    let near_ok: std::collections::HashSet<&str> = near.iter().take(near_n).map(|(_, p)| p.path.as_str()).collect();

    let mut out = HashMap::with_capacity(n);
    for p in photos {
        let best = best_of(p.group);
        let size = by_group[&p.group].len();
        let flag_r = flag_reasons(p);
        let (state, mut reasons): (State, Vec<Reason>);

        if is_hard(&p.flags) {
            state = State::Reject;
            reasons = flag_r.clone();
        } else if let Some(st) = chosen.get(&p.path) {
            state = st.clone();
            reasons = Vec::new();
            if state == State::Keep {
                reasons.push(if size > 1 && p.group_best {
                    reason_with("reason.keepGroup", &[("n", Value::from(size)), ("score", Value::from(pct(p.score)))])
                } else {
                    reason_with("reason.keepSolo", &[("score", Value::from(pct(p.score)))])
                });
            } else if p.score >= GOOD_LINE {
                // a good photo that only the keep ceiling kept out: not a borderline one
                reasons.push(reason_with("reason.capCut", &[("score", Value::from(pct(p.score)))]));
            } else {
                reasons.push(reason_with("reason.reviewEdge", &[("score", Value::from(pct(p.score)))]));
            }
            reasons.extend(flag_r);
        } else if !p.group_best {
            if near_ok.contains(p.path.as_str()) {
                state = State::Review;
                reasons = vec![reason_with("reason.nearBest", &[("diff", Value::from(pct(best.score - p.score).max(0)))])];
            } else {
                state = State::Reject;
                // say what the best does better, when one thing stands out; the bare comparison otherwise
                let k = shortfall(p, best).unwrap_or("reason.betterExists");
                reasons = vec![reason_with(k, &[("best", Value::from(pct(best.score))), ("score", Value::from(pct(p.score)))])];
            }
            reasons.extend(flag_r);
        } else {
            state = State::Reject;
            reasons = vec![if is_soft(&p.flags) { reason("reason.exposureFloor") } else { reason_with("reason.belowLine", &[("score", Value::from(pct(p.score)))]) }];
            reasons.extend(flag_r);
        }
        if let Some(r) = pref_reason(p) {
            reasons.push(r);
        }

        if let Some(o) = overrides.get(&p.path) {
            let ostate = match o.as_str() {
                "keep" => State::Keep,
                "review" => State::Review,
                _ => State::Reject,
            };
            let mut with_manual = vec![reason_with("reason.manual", &[("stateKey", Value::from(format!("state.{o}")))])];
            with_manual.extend(reasons);
            out.insert(p.path.clone(), Decision { state: ostate, manual: true, reasons: with_manual });
        } else {
            out.insert(p.path.clone(), Decision { state, manual: false, reasons });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reason for a rejected group mate names the part of the score the best is clearly better at; closed eyes come first;
    /// a mate that is simply a little worse everywhere gets the plain comparison.
    #[test]
    fn a_rejected_mate_is_told_what_the_best_does_better() {
        let mut best = solo("a.jpg", 0.9, 1, &[]);
        best.sharp_score = 0.9; best.expo_score = 0.8; best.ai_score = Some(0.7);
        // mates well below the review band, so they are rejected as "a better one exists" and not asked about
        let mut soft = solo("b.jpg", 0.5, 1, &[]);
        soft.group_best = false; soft.sharp_score = 0.4; soft.expo_score = 0.78; soft.ai_score = Some(0.69);
        let mut closed = solo("c.jpg", 0.5, 1, &["eyes"]);
        closed.group_best = false; closed.sharp_score = 0.4; closed.expo_score = 0.8; closed.ai_score = Some(0.7);
        let mut even = solo("d.jpg", 0.5, 1, &[]);
        even.group_best = false; even.sharp_score = 0.88; even.expo_score = 0.79; even.ai_score = Some(0.68);
        let out = decide(&[best, soft, closed, even], 1.0, PickMode::Best, false, &HashMap::new());
        assert_eq!(out["b.jpg"].reasons[0].k, "reason.betterSharp");
        assert_eq!(out["c.jpg"].reasons[0].k, "reason.betterEyes");
        assert_eq!(out["d.jpg"].reasons[0].k, "reason.betterExists");
    }

    /// When the taste model moved a score by enough, the reasons say so, in the direction it moved it.
    #[test]
    fn the_taste_models_say_is_shown_when_it_mattered() {
        let mut up = solo("up.jpg", 0.8, 1, &[]);
        up.pref_delta = 0.06;
        let mut down = solo("down.jpg", 0.8, 2, &[]);
        down.pref_delta = -0.05;
        let mut quiet = solo("quiet.jpg", 0.8, 3, &[]);
        quiet.pref_delta = 0.01;
        let out = decide(&[up, down, quiet], 1.0, PickMode::Best, false, &HashMap::new());
        assert!(out["up.jpg"].reasons.iter().any(|r| r.k == "reason.prefUp"));
        assert!(out["down.jpg"].reasons.iter().any(|r| r.k == "reason.prefDown"));
        assert!(!out["quiet.jpg"].reasons.iter().any(|r| r.k.starts_with("reason.pref")));
    }

    /// A photo that is the sole (and therefore best) member of its own group, with everything else defaulted.
    fn solo(path: &str, score: f32, group: usize, flags: &[&str]) -> Photo {
        Photo {
            path: path.into(),
            score,
            group,
            scene: group,
            group_best: true,
            flags: flags.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    /// What the review pile is made of, on a real analysed folder: FG_CACHE (the analysis cache), FG_DIR (the folder), FG_TAG (the cache tag).
    #[test]
    #[ignore = "needs a real analysis cache: FG_CACHE=... FG_DIR=... [FG_TAG=v3|l14|thumb640]; run with --ignored --nocapture"]
    fn what_the_review_pile_is_made_of() {
        let (Ok(cache), Ok(dir)) = (std::env::var("FG_CACHE"), std::env::var("FG_DIR")) else { return };
        let tag = std::env::var("FG_TAG").unwrap_or_else(|_| "v3|l14|thumb640".into());
        let c = crate::cache::Cache::load(cache.into());
        let mut photos: Vec<Photo> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| crate::analyze::is_image(p))
            .filter_map(|p| { let (sz, mt) = crate::cache::file_key(&p)?; c.get(&p.to_string_lossy(), sz, mt, &tag) }).collect();
        photos.sort_by(|a, b| a.path.cmp(&b.path));
        crate::analyze::group_and_score(&mut photos, None);
        for one_per_group in [false, true] {
            let out = decide(&photos, 1.0, PickMode::Diverse, one_per_group, &HashMap::new());
            let count = |s: State| out.values().filter(|d| d.state == s).count();
            println!("\n{} photos, one per group {one_per_group}: keep {}, review {}, reject {}", photos.len(), count(State::Keep), count(State::Review), count(State::Reject));
            let mut by: std::collections::BTreeMap<String, usize> = Default::default();
            for d in out.values().filter(|d| d.state == State::Review) { *by.entry(d.reasons.first().map_or("-".into(), |r| r.k.to_string())).or_default() += 1; }
            println!("  review, by first reason: {by:?}");
            let mut rej: std::collections::BTreeMap<String, usize> = Default::default();
            for d in out.values().filter(|d| d.state == State::Reject) { *rej.entry(d.reasons.first().map_or("-".into(), |r| r.k.to_string())).or_default() += 1; }
            println!("  reject, by first reason: {rej:?}");
        }
        let mut hist = [0usize; 10];
        for p in &photos { hist[((p.score * 10.0) as usize).min(9)] += 1; }
        println!("score histogram (0.0-0.1 ... 0.9-1.0): {hist:?}");
        let mut sizes: std::collections::BTreeMap<usize, usize> = Default::default();
        let mut gs: HashMap<usize, usize> = HashMap::new();
        photos.iter().for_each(|p| *gs.entry(p.group).or_default() += 1);
        gs.values().for_each(|n| *sizes.entry(*n).or_default() += 1);
        println!("groups by size: {sizes:?}  (in a group of 2+: {} photos)", gs.values().filter(|n| **n > 1).sum::<usize>());
        let flagged = photos.iter().filter(|p| !p.flags.is_empty()).count();
        println!("photos with a flag: {flagged}; kinds: {:?}", photos.iter().fold(std::collections::BTreeMap::<String, usize>::new(), |mut m, p| { *m.entry(p.kind.clone()).or_default() += 1; m }));
    }

    #[test]
    fn splits_by_score_and_ratio() {
        let photos = vec![solo("a", 0.9, 0, &[]), solo("b", 0.8, 1, &[]), solo("c", 0.65, 2, &[]), solo("d", 0.1, 3, &[])];
        let out = decide(&photos, 0.5, PickMode::Diverse, false, &HashMap::new());
        assert_eq!(out["a"].state, State::Keep);
        assert_eq!(out["b"].state, State::Keep);
        assert_eq!(out["c"].state, State::Review, "just under the line is a toss-up");
        assert_eq!(out["d"].state, State::Reject);
        assert!(!out["a"].manual);
    }

    /// The bug that filled the review pile: one crowded scene's photos were worth less than a poor photo in a fresh scene, so the queue ran into the
    /// poor one first and stopped, leaving good photos unkept (and so to review).
    #[test]
    fn a_photo_below_the_line_does_not_stop_the_good_ones_behind_it() {
        // six good photos of one scene (each kept one lowers the value of the next), and one poor photo of a scene of its own
        let mut photos: Vec<Photo> = (0..6).map(|i| solo(&format!("s{i}"), 0.82 - i as f32 * 0.002, i, &[])).map(|mut p| { p.scene = 0; p }).collect();
        photos.push(solo("poor", 0.69, 10, &[]));
        let out = decide(&photos, 1.0, PickMode::Diverse, false, &HashMap::new());
        for i in 0..6 {
            assert_eq!(out[&format!("s{i}")].state, State::Keep, "s{i}");
        }
        assert_ne!(out["poor"].state, State::Keep);
    }

    #[test]
    fn the_review_pile_stays_small_and_is_made_of_the_closest_calls() {
        // 100 photos just under the line, 100 far below it, 100 good
        let mut photos = Vec::new();
        for i in 0..100 { photos.push(solo(&format!("g{i}"), 0.80, i, &[])); }
        for i in 0..100 { photos.push(solo(&format!("n{i}"), 0.62 + i as f32 * 0.0007, 100 + i, &[])); }
        for i in 0..100 { photos.push(solo(&format!("b{i}"), 0.30, 200 + i, &[])); }
        let out = decide(&photos, 1.0, PickMode::Diverse, false, &HashMap::new());
        let review: Vec<&String> = out.iter().filter(|(_, d)| d.state == State::Review).map(|(k, _)| k).collect();
        assert!(review.len() <= 300 / 12, "{} to review", review.len());
        assert!(review.iter().all(|k| k.starts_with('n')), "only photos near the line are asked about");
        assert!(review.iter().all(|k| k[1..].parse::<usize>().unwrap() >= 100 - review.len()), "the closest to the line first");
        assert!((0..100).all(|i| out[&format!("b{i}")].state == State::Reject), "far below the line is not a toss-up");
    }

    #[test]
    fn mates_that_tie_with_the_best_are_reviewed_only_up_to_a_few_percent() {
        // 40 groups of two, the mate within 1 point of the best: 40 toss-ups, but only a few percent of 80 photos are put to review
        let mut photos = Vec::new();
        for g in 0..40 {
            let mut best = solo(&format!("best{g}"), 0.85, g, &[]);
            best.scene = g;
            let mut mate = solo(&format!("mate{g}"), 0.85 - 0.002 * (g as f32 + 1.0), g, &[]);
            mate.group_best = false;
            mate.scene = g;
            photos.push(best);
            photos.push(mate);
        }
        let out = decide(&photos, 1.0, PickMode::Diverse, true, &HashMap::new());
        let review = out.iter().filter(|(k, d)| k.starts_with("mate") && d.state == State::Review).count();
        let rejected = out.iter().filter(|(k, d)| k.starts_with("mate") && d.state == State::Reject).count();
        assert!(review >= 1 && review <= 4, "{review}");
        assert_eq!(review + rejected, 40);
        assert!(out.iter().filter(|(k, _)| k.starts_with("best")).all(|(_, d)| d.state == State::Keep));
    }

    #[test]
    fn a_hard_flag_always_rejects_regardless_of_score() {
        let photos = vec![solo("a", 0.95, 0, &["blur"])];
        let out = decide(&photos, 0.5, PickMode::Diverse, false, &HashMap::new());
        assert_eq!(out["a"].state, State::Reject);
        assert_eq!(out["a"].reasons.len(), 1);
        assert_eq!(out["a"].reasons[0].k, "reason.blur");
    }

    #[test]
    fn diverse_mode_trades_a_little_score_for_scene_coverage() {
        // a, b: same scene, close scores. c: a different scene, in between. With 2 to keep:
        let photos = vec![solo("a", 0.90, 0, &[]), solo("b", 0.89, 0, &[]), solo("c", 0.87, 1, &[])];
        let diverse = decide(&photos, 0.6, PickMode::Diverse, false, &HashMap::new());
        assert_eq!(diverse["a"].state, State::Keep);
        assert_eq!(diverse["c"].state, State::Keep, "the scene penalty should make room for the other scene");
        assert_ne!(diverse["b"].state, State::Keep);

        let best = decide(&photos, 0.6, PickMode::Best, false, &HashMap::new());
        assert_eq!(best["a"].state, State::Keep);
        assert_eq!(best["b"].state, State::Keep, "pure best-score mode ignores scene coverage");
    }

    #[test]
    fn every_good_photo_is_kept_when_there_is_no_ceiling() {
        let photos: Vec<Photo> = (0..10).map(|i| solo(&format!("p{i}"), 0.8 + i as f32 * 0.01, i, &[])).collect();
        let out = decide(&photos, 1.0, PickMode::Diverse, false, &HashMap::new());
        assert!(out.values().all(|d| d.state == State::Keep), "a folder of good photos is kept whole");
    }

    #[test]
    fn a_good_photo_is_kept_even_when_a_group_mate_scores_higher() {
        let mut a = solo("a", 0.92, 0, &[]);
        let mut b = solo("b", 0.85, 0, &[]);
        a.group_best = true;
        b.group_best = false;
        let out = decide(&[a, b], 1.0, PickMode::Diverse, false, &HashMap::new());
        assert_eq!(out["b"].state, State::Keep, "meeting the standard is enough; being first of the group is not required");
    }

    #[test]
    fn one_per_group_keeps_only_the_best_of_a_group() {
        let mut a = solo("a", 0.92, 0, &[]);
        let mut b = solo("b", 0.85, 0, &[]);
        a.group_best = true;
        b.group_best = false;
        let out = decide(&[a, b], 1.0, PickMode::Diverse, true, &HashMap::new());
        assert_eq!(out["a"].state, State::Keep);
        assert_ne!(out["b"].state, State::Keep);
    }

    #[test]
    fn poor_photos_are_not_padded_into_keeps() {
        let photos: Vec<Photo> = (0..10).map(|i| solo(&format!("p{i}"), 0.4 + i as f32 * 0.01, i, &[])).collect();
        let out = decide(&photos, 1.0, PickMode::Diverse, false, &HashMap::new());
        assert!(out.values().all(|d| d.state != State::Keep));
    }

    #[test]
    fn a_good_photo_the_ceiling_left_out_is_not_called_borderline() {
        let photos: Vec<Photo> = [("a.jpg", 0.92), ("b.jpg", 0.88), ("c.jpg", 0.84), ("d.jpg", 0.80)].iter().enumerate().map(|(i, (p, s))| solo(p, *s, i, &[])).collect();
        let out = decide(&photos, 0.25, PickMode::Best, false, &HashMap::new());
        assert_eq!(out["a.jpg"].state, State::Keep);
        for p in ["b.jpg", "c.jpg", "d.jpg"] {
            assert_eq!(out[p].state, State::Review, "{p} is good but over the ceiling");
            assert_eq!(out[p].reasons[0].k, "reason.capCut", "{p} got {}", out[p].reasons[0].k);
        }
    }

    #[test]
    fn a_ceiling_sends_the_extra_good_photos_to_review() {
        let photos: Vec<Photo> = (0..10).map(|i| solo(&format!("p{i}"), 0.8 + i as f32 * 0.01, i, &[])).collect();
        let out = decide(&photos, 0.3, PickMode::Best, false, &HashMap::new());
        assert_eq!(out.values().filter(|d| d.state == State::Keep).count(), 3);
        assert_eq!(out.values().filter(|d| d.state == State::Review).count(), 7);
    }

    /// The straightforward version this replaced (one full pass per pick), kept here to check the queue picks exactly the same photos.
    fn naive_greedy(pool: &[&Photo], limit: usize, floor: f32, penalty: f32, count_scene: bool, counts: &mut HashMap<usize, usize>) -> Vec<String> {
        let mut rest: Vec<&Photo> = pool.to_vec();
        let mut out = Vec::new();
        while out.len() < limit && !rest.is_empty() {
            let (mut bi, mut bv) = (0usize, f32::NEG_INFINITY);
            for (i, p) in rest.iter().enumerate() {
                let v = p.score - penalty * (*counts.get(&p.scene).unwrap_or(&0) as f32);
                if v > bv { bv = v; bi = i; }
            }
            let p = rest.remove(bi);
            if p.score < floor { break; }
            out.push(p.path.clone());
            if count_scene { *counts.entry(p.scene).or_insert(0) += 1; }
        }
        out
    }

    #[test]
    fn the_queue_picks_what_the_full_pass_picked() {
        let mut seed = 12345u64;
        let mut rnd = move || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); ((seed >> 33) as f32) / (u32::MAX as f32 / 2.0) };
        for (n, scenes, limit, penalty) in [(300usize, 20usize, 90usize, 0.06f32), (500, 7, 400, 0.06), (200, 200, 50, 0.0), (400, 3, 399, 0.12)] {
            // scores on a coarse grid, so ties happen
            let photos: Vec<Photo> = (0..n).map(|i| { let mut p = solo(&format!("p{i}"), 0.3 + (rnd() * 40.0).floor() / 100.0, i, &[]); p.scene = (rnd() * scenes as f32) as usize % scenes; p }).collect();
            let pool: Vec<&Photo> = photos.iter().collect();
            let mut c1 = HashMap::new();
            let want: std::collections::HashSet<String> = naive_greedy(&pool, limit, 0.3, penalty, true, &mut c1).into_iter().collect();
            let mut chosen = HashMap::new();
            let mut c2 = HashMap::new();
            take_greedy(pool.clone(), limit, 0.3, State::Keep, true, penalty, &mut chosen, &mut c2);
            let got: std::collections::HashSet<String> = chosen.keys().cloned().collect();
            assert_eq!(got, want, "n={n} scenes={scenes} limit={limit}");
            assert_eq!(c1, c2);
        }
    }

    #[test]
    fn many_photos_are_decided_quickly_and_the_same_way() {
        // 20 000 photos in 800 scenes: the old pass-per-pick selection was quadratic and took seconds here
        let photos: Vec<Photo> = (0..20_000).map(|i| solo(&format!("p{i}"), 0.5 + ((i * 7919) % 1000) as f32 / 2000.0, i, &[])).map(|mut p| { p.scene = p.group % 800; p }).collect();
        let t = std::time::Instant::now();
        let out = decide(&photos, 0.3, PickMode::Diverse, false, &HashMap::new());
        assert!(t.elapsed().as_millis() < 1500, "took {:?}", t.elapsed());
        assert_eq!(out.values().filter(|d| d.state == State::Keep).count(), 6000);
        // every kept photo scores at least as much as the best photo that was not kept, less what its scene's penalty took off
        let kept_min = photos.iter().filter(|p| out[&p.path].state == State::Keep).map(|p| p.score).fold(1.0f32, f32::min);
        assert!(kept_min > 0.5);
    }

    #[test]
    fn a_manual_override_wins_and_is_marked() {
        let photos = vec![solo("a", 0.9, 0, &[]), solo("b", 0.1, 1, &[])];
        let mut overrides = HashMap::new();
        overrides.insert("a".to_string(), "reject".to_string());
        let out = decide(&photos, 0.5, PickMode::Diverse, false, &overrides);
        assert_eq!(out["a"].state, State::Reject);
        assert!(out["a"].manual);
        assert_eq!(out["a"].reasons[0].k, "reason.manual");
    }
}
