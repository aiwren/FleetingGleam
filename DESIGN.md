# Design System: FleetingGleam

## 1. Visual Theme & Atmosphere
A darkroom for travel photographs. The interface recedes so the pictures carry all the colour: a near-black, slightly warm canvas, hairline structure, and monochrome chrome with kept photos marked by a solid check. Density is balanced (5) — enough numbers to trust the ranking, enough air to judge a photo. Variance is moderate (5): a fixed left rail against a fluid, justified photo wall. Motion is fluid but quiet (5): spring-eased reveals, no perpetual decoration.

## 2. Color Palette & Roles
Two palettes, pure black and white, following the system appearance (prefers-color-scheme). No hue at all: hierarchy comes from contrast and weight.
- **Light** — Canvas #FFFFFF, Panel #F7F7F8, Surface #EFEFF1, Ink #111113, Ash #6C6C74, Hairline rgba(0,0,0,.09)
- **Dark** — Canvas #0E0E10, Panel #151517, Surface #1D1D20, Ink #EDEDEF, Ash #8F8F98, Hairline rgba(255,255,255,.09)
- **Accent = Ink.** Primary buttons, the "kept" check and the selection ring are solid Ink; "rejected" photos are dimmed and desaturated. The photo viewer is always dark.

## 3. Typography Rules
- **UI & Display:** Geist Variable — tight tracking on titles (-0.02em), hierarchy through weight and tone
- **Numbers & Metadata:** Geist Mono — every score, count and measurement, tabular figures
- **Banned:** Inter, serif faces (this is software UI), emoji

## 4. Component Stylings
* **Buttons:** Primary is solid Ink with inverse text, 8px radius, 1px downward press. Secondary is a Hairline-outlined Lifted Surface. No glows.
* **Photo tiles:** Natural aspect ratio, 10px radius, no card chrome. Score chip and best marker appear on the image; details on hover.
* **Slider:** Thin track, Ink fill.
* **Loaders:** Shimmering skeleton tiles plus a thin progress line. No spinners.
* **Empty state:** Composed panel that says what to do and what happens next.
* **Viewer:** Full-window overlay, photo left, measurements right, keyboard driven.

## 5. Layout Principles
Modular: a 220px native-vibrancy sidebar lists feature modules; each module owns a workspace plus an optional 300px inspector. Native macOS menu bar carries Open, Toggle Sidebar, Toggle Inspector. Photos flow in justified rows (each row fills the width, natural aspect kept). Below 900px the rail collapses above the wall. Max content width is unconstrained because the photo wall is the product.

## 6. Motion & Interaction
Tiles reveal with a short staggered rise (transform + opacity only, capped delay). Hover lifts brightness, not size. Viewer fades in. Respect prefers-reduced-motion.

## 7. Anti-Patterns (Banned)
No emoji, no Inter, no pure black, no glows, no gradient text, no 3-equal-card rows, no spinners, no overlapping text, no cloud or upload language — everything is on-device and the UI says so plainly.

## 8. Modules
Each feature is a named module. **定格** (照片筛选) is the first: freezing the few moments worth keeping from a trip. Next: **测光** (摄影参数); future modules follow the same camera-operation naming (显影, 取景…).

## 9. Signalling keep / reject (cultural note)
Black, white and grey must never carry the meaning "bad" or "discarded" (they read as mourning or ill omen in several cultures). Photos always keep their original colour: no grayscale, no black or white wash.
- **Keep** = Gleam Amber (#DEA13F light / #E8B45C dark) check badge; the inspector verdict fills amber.
- **Reject** = shape only: a small glass "x 淘汰" tag, the tile eases slightly smaller and lighter. No colour change to the photo.
Amber is the single accent, used only for "kept".

## 10. Motion language
Motion is the product's second face. The brand idea is a *gleam*: a brief glint of light that is gone almost as soon as it appears. Every animation either (a) moves something through space with weight, or (b) lets a little light pass over something that matters. Nothing bounces, nothing loops for decoration.

**Tokens** (`src/motion.ts`, mirrored as CSS variables): expo-out easing `cubic-bezier(.16,1,.3,1)`; durations fast .16 / base .32 / slow .6 / develop .9 s; springs *glide* (reflow, pills, selection ring), *soft* (entrances), *view* (photo to lightbox), *tap* (press feedback).

**Signature moments**
1. **Focus pull** (`FocusImg`): a photo arrives soft, slightly over-exposed and a touch large, then racks into focus. Used whenever a thumbnail first appears. Not replayed on re-mount.
2. **Gleam sweep** (`.glint`): one soft band of warm light crosses a photo when it becomes "kept". After analysis it runs across all kept photos in reading order, a small reveal of the picks.
3. **Developing progress**: pending tiles show a ring with the current stage (decode, quality, AI, system, thumbnail) and breathe; the top progress bar carries a glint on its leading edge; on completion the banner turns into a one-line summary for ~3 s.
4. **Fly to lightbox**: the photo travels from its tile to the lightbox (shared `layoutId`), a blurred copy of the photo becomes ambient light behind it, stepping slides the next photo in from the direction of travel, zoom eases instead of snapping.
5. **Living selection**: the selection ring glides between tiles; segmented and sidebar pills glide; grid reflow is a spring, not a jump.

**Rules**: animate transform, opacity and (only for the focus pull) filter; stagger capped at ~16 items; respects `prefers-reduced-motion` (MotionConfig + CSS); no looping motion except the pending-tile breathing and the scan sweep, both only while work is happening.

**Verification**: `pnpm dev` then open `http://localhost:1420/?mock` in any browser to run the UI with painted sample photos and a simulated scan (dev only, `src/dev/mock.ts`).

## 11. Internationalisation
- English name is **FleetingGleam** everywhere. The Chinese product name is a locale string (`brand.name` in `src/i18n/zh-CN.ts`), so it can change without touching code.
- Languages: 简体中文 and English, following the system language by default, switchable in the sidebar. New languages are one file in `src/i18n/` plus an entry in `LANGS`.
- No user-visible string lives in components. Decision reasons are message keys plus parameters (`src/decide.ts`), rendered in the current language. Plurals use i18next (`_one` / `_other`); sizes and numbers go through `Intl`.
- The native macOS menu bar is rebuilt in the active language (`set_locale` command).
- Layout must tolerate the longer English strings: the toolbar title truncates, nav labels never wrap.

## 12. Motion v2 (2026-09-21)
Feedback: the first pass felt stiff. Diagnosis: everything used the same fade and small scale, nothing reacted to the pointer, and progress was a number, not something you could watch. References: pointer-following 3D tilt and glare on cards ([Codrops tilt hover](https://tympanus.net/codrops/2016/11/23/tilt-hover-effects/), [CodeFronts tilt cards](https://codefronts.com/components/css-3d-tilt-hover-cards/)); scroll-velocity skew and smooth scroll on photography portfolios ([Awwwards](https://www.awwwards.com/inspiration/skew-distortion-effect-on-scroll)); Apple's guidance that springs keep velocity and should be interruptible ([WWDC23 Animate with springs](https://developer.apple.com/videos/play/wwdc2023/10158/)).

Added:
1. **Developing photo**: the thumbnail is made right after decoding and shown while the analysis runs, soft and over-exposed, and it comes into focus stage by stage (blur is a function of progress). The same tile element carries the photo from "pending" to "finished", so nothing jumps, and its true aspect ratio is known early so the layout does not reflow at the end.
2. **Pointer-aware tiles**: 3D tilt toward the cursor and a soft-light glare that follows it. Written to CSS variables, so hovering triggers no React renders.
3. **Light motes**: promoting a photo to "keep" pops the check and releases a few points of light. Not played when the analysis first reveals the picks (that is the gleam sweep's job).
4. **Rolling digits** (`Ticker`) for progress and counts; **word-by-word masked reveal** (`Reveal`) for hero text, using `Intl.Segmenter` so Chinese splits into words.
5. **Modules cross-fade** with a small rise instead of popping; segmented pills glide.

Deliberately not done: scroll-velocity skew on the wall. A skewed parent breaks the projection maths of layout animations, and the wall relies on those.

## 13. Window chrome
Every strip that sits in the title-bar area is a drag region (`data-tauri-drag-region`): sidebar top, toolbar, inspector header, viewer bar. Text inside a drag region has `pointer-events: none` so a press on a label still drags the window (Tauri only starts a drag when the element under the pointer carries the attribute itself).

## 14. Settings
A page, not a popover: language, appearance (system / light / dark, also applied to the native window so the translucent sidebar follows), compute (auto / CPU only), what the last analysis actually ran on, thumbnail cache size and clearing, privacy, about. Opens from the sidebar footer or the native **Settings… (⌘,)** menu item.

## 15. Projects, stable layout, quiet overlays (2026-09-21)
- **Project = a named set of folders** plus the decisions made on its photos. Projects live in the module's own workspace; you can add or remove folders, rename (double-click the title) and delete. Analysis results are cached by the backend (path + size + modified time + thumbnail present), so reopening a project or adding a folder only analyses what is new, and cached projects open instantly.
- **Tiles never change size.** The wall is laid out once from true (orientation-corrected) sizes read from file headers *before* analysis. Reflow uses position-only layout animation, so photos and progress rings are never stretched mid-move.
- **Similar shots sit together** in "All" (default sort "Group similar"): groups keep their place in the folder, the best of a group comes first, and a small gap follows each group. Other sorts: file order, capture time, score.
- **Quiet overlays**: one small icon badge per photo (amber check = keep, ? = review, x = reject; it expands to a label on hover), the score appears on hover or when sorting by score, and the selection ring is a single thin line.
- **Inspector starts closed** and slides open on the first selection; closing it by hand stops it from reopening until the user opens it again.
- **Collapsed sidebar is an icon rail** (64 px): module icons on top, Settings at the very bottom, the toggle just under the traffic lights.
- **Learning your taste** (Settings, on by default, local only): explicit keep / reject decisions train a small logistic-regression model on each photo's CLIP embedding. It nudges scores (weight grows with evidence, capped at 35%) and can never rescue blurry photos or screenshots. Files: `preferences.json` and `analysis-cache.json` in the app data folder; reset any time.

## 16. Marks made of light (2026-09-21)
Feedback: corner badges (a tick on a disc) are what every app does and have nothing of ours. The mark system now comes from the logo's own gold four-point gleam and from photography itself.
- **Keep = a lit gleam**: the logo's gold star at the photo's corner, plus a thin warm rim and soft halo. **Maybe = the same gleam, hollow.** **Reject = no icon**: the light falls away and the photo recedes (smaller, lighter). Never grayscale, never black or white as meaning.
- **Marking effects** (only when the user changes a mark, or once when an analysis reveals the picks): keep releases an anamorphic-style streak across the photo at the star's height and a bloom at the star; reject closes an aperture (a vignette collapsing inward) and then lets the photo recede; maybe draws the outline of the gleam.
- **Glass dock** appears under the cursor on hover: gleam / hollow gleam / cross. One click marks a photo, no need to open the inspector. Keys P / R / X still work.
- **Selection = a camera focus frame**: four corner brackets lock on to the photo and glide to the next one, like an autofocus point moving.
- **Never lose the selection**: in a filtered view, a decision that moves the photo elsewhere moves the selection to its neighbour; the selected photo stays listed until you move on.

## 17. Recycle bin for projects
Deleting a project moves it to the recycle bin. The bin only shows items deleted in the last 7 days (TRASH_DAYS); older ones are removed in the background when the app opens (and whenever the window regains focus). Its photo files are never touched. Trash, rename and restore live on the project cards; a toast offers Undo. Photo files themselves are not deleted by this app; a bin for rejected *photos* needs a decision on semantics first (system Trash vs exporting the keepers).

## 18. Marking interaction, second pass (2026-09-21)
Feedback: two stars in the hover dock (filled and hollow) plus a third in the corner was confusing, and nothing explained itself.
- **The dock has two actions only**: gold gleam = keep, cross = reject. "Maybe" is the system's own suggestion (shown as a hollow gleam in the corner); users can still set it with R or in the inspector.
- **Buttons explain themselves**: hovering one unfolds it into a labelled pill with its shortcut ("留下 P"). No reliance on native tooltips, which barely appear in a webview.
- **No duplicate symbols**: while the dock is up, the corner mark fades. The keep action is always the *filled* gleam; an outline always means "maybe".
- **Clicking an active manual mark clears it** and hands the photo back to the system's suggestion (the label changes to "Clear my mark").
- **Undo** with Cmd+Z (last 50 marks).
- **Symbols are taught up front**: the filter tabs carry the same glyphs (gleam / outline / cross), and a one-line tip shows until the dock has been used a few times or is dismissed.
- **The score chip** is always white on dark glass and explains itself on hover. (A class-name clash, `.keep` used by both the slider row and the keep state, had been turning its text dark on kept photos.)
- Lesson: never reuse a bare state class name (`keep`, `review`, `reject`) for anything but tile/verdict state.

## 19. Organize: exporting keepers and the system Trash (2026-09-21)
The toolbar's **整理 / Organize** menu turns decisions into files, with two rules: originals are never modified, and anything destructive is recoverable.
- **Export kept photos**: copy to a folder the user picks; same-named files get a number, nothing is overwritten. The dialog shows the count and destination, then progress, then "Show in Finder".
- **Move rejected photos to the Trash**: goes to the *system* Trash (NSFileManager, no Finder permission prompt), so Finder's Put Back works. The dialog defaults to "only the ones I marked myself"; including the system's suggestions is an explicit choice with a visible warning of how many were never confirmed. Only image files are accepted by the backend command, so it cannot be used to trash anything else.
- After a move, the project reloads and forgets decisions about files that are gone.
- Projects have their own bin (7 days). Photos do not: their bin is the system Trash.

## 20. Compute (measured 2026-09-21)
The fp16 CLIP model is now in `models/`. Auto = fp16 through CoreML; CPU only = int8. Whole pipeline with Apple Vision: 209 ms/photo on CPU int8 vs 120 ms/photo with CoreML fp16 (the CPU is freed for decoding and Vision). Model alone: CPU int8 is slightly faster (35 vs 43 ms). Whether CoreML uses the GPU or the Neural Engine is the system's choice and not observable.

## 21. Interaction and structure fixes (2026-09-21, later)
- **Tooltips are bubbles, never a control that changes size.** The dock's labels used to unfold inside the button; the button grew under the cursor and the hover flickered. Now a bubble ("留下 · P") appears above the icon and the button never moves. General rule: nothing may change its own hit area on hover.
- **One toolbar row.** Breadcrumb + project name + count | filter tabs | sort, Organize, project settings, inspector. The old second row (keep-share slider, sort, tip) is gone: keep share, default sort and per-project learning live in **project settings** (a dialog: name, folders, preferences, delete). The first-use tip is a small floating card, and "re-evaluate" appears next to sort only when there is something to re-evaluate.
- **The wall is windowed.** Justified rows are computed in code (`Wall.tsx`) and only rows near the viewport are mounted (about 40-70 tiles for a 300-photo project, and the same for 5000). Measured on 300 photos: switching the filter went from 0.8-1.8 s to 50-125 ms. Consequences: tiles have no `layoutId` (it breaks when a tile scrolls out and back in); the viewer flies from and back to the tile by measuring its rectangle instead.
- **Two model tiers.** Standard = CLIP ViT-B/32 (about 45 ms per photo). High = CLIP ViT-L/14 fp16 + the improved LAION aesthetic predictor, its five linear layers composed into one 768-d vector (about 500-800 ms per photo). Cache entries carry a tag (analysis version + model + thumbnail size), so switching tier or thumbnail size re-analyses automatically. Thumbnails are now 640 px.

## 22. Landing page first screen: the galaxy (2026-10-16)
Feedback, in order: the opening was a screenshot collage, too small and too still; the user asked for particles, glass, pointer response and a scroll-told story ("thousands of photos, a few picked out, then the one"); then for the sea to have a shape (a nebula), for the photographs to sit on its arms with a thread back to the heart and to turn with it, and for everything to live in one 3D space. Large soft "gas" discs were tried and rejected as ugly: points only.
- **A galaxy of motes** (`apps/web/src/components/motes.ts`, WebGL points): one point of light per photograph of a long shoot, in three tight arms around a dense warmer heart, with depth (near ones larger, brighter, and they slide more against the pointer) and a differential turn so slow it is felt rather than seen. The heart is the stage's centre. The plane leans back 16° in a shared perspective with the photographs, and the canvas overscans the screen so the lean never shows an edge.
- **Frames on the arms**: each frame is (arm, radius) in the same spiral the motes use (`armPoint`), rides the turn and leans with the arm's tangent; a thread of small motes runs along the arm from the heart to it. Positions are written each tick as plain transforms, outside React.
- **Scroll tells the story** (`HeroScene.tsx`, pinned for ~3 screens): the motes condense into eight frames (each taking the colour of the pixel it lands on), the photographs develop, three are kept (the gleam) and the motes of the other five come loose and drift back into the sea; then the camera comes round to face the plane, the best of the three rises with the focus brackets, and the reasons arrive on glass. Reversible, at the reader's pace.
- **Pointer**: the sea parts around it, is lit by it and carried a little in its wake, then springs back; the whole plane leans toward it; a click lets a few amber motes go.
- **Exception to §10 noted on purpose**: the galaxy is ambient motion. It is kept slow, sub-threshold in brightness, paused when off screen, and dropped entirely under `prefers-reduced-motion` (the photographs are then simply there, chosen).
- Glass (`.glass`) carries the reason card, the kept tags, the language menu, the feature tiles and the footer, so there is always something moving behind the blur.
