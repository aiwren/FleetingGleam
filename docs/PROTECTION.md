# Protecting the models and algorithms

Everything runs on the user's machine, so everything the app needs is also on the user's machine. **No local program can be made
impossible to reverse-engineer.** What we can do is make it expensive, and make the cheap attacks (copying a file, reading a
source map, grepping strings) fail. This is what a release does, and what it deliberately does not.

## What is protected, and how

| Asset | Threat | Measure |
|---|---|---|
| CLIP encoders, text model, aesthetic heads (`models/`) | Copying the model files out of the app or its data folder | Never in the app bundle; downloaded on demand in their **sealed** form: AES-256-GCM in 4 MiB chunks (`src-tauri/src/seal.rs`), checked against a SHA-256 manifest, then decrypted **in memory only** (`commit_from_memory`); no plaintext file is ever written. Any tampering makes opening fail. |
| The sealing key | Finding it with `strings` | Never in the repo. Passed as `FG_SEAL_KEY` at build time, split into two XOR shares by `build.rs`, joined only inside `seal::cipher()`. |
| Scoring, grouping, thresholds (`scoring.rs`, `analyze.rs`) | Reading the logic | Compiled Rust: LTO, `codegen-units = 1`, `strip = true`, no debug info, `panic = "abort"`. Build paths are remapped so panic messages do not reveal the build machine. |
| Front-end code | Reading source / source maps | Production bundle is minified and mangled, no source maps, `console` and `debugger` removed (`vite.config.ts`). |
| Apple Vision helper | Symbol names | Built with `-O` and `strip -x`. |
| Debugger attach | `lldb` on the running app | `PT_DENY_ATTACH` in release builds on macOS. |
| Development paths | The repo location leaking, or a release using dev models | Release builds have no `CARGO_MANIFEST_DIR` fallback; only debug builds may read `models/`. |

## What it does not stop

* Someone who **runs** the app under a debugger or a memory dumper can recover the model bytes: the key and the plaintext must
  exist in memory to run inference. `PT_DENY_ATTACH` only stops the casual attempt.
* Anyone can **use the model as an oracle**: feed it images and record the scores. A model can be approximated from that.
* The front-end decision logic (`decide.ts`) is minified, not secret. If it is commercially sensitive, it should move into Rust.

## Doing a release

1. `openssl rand -hex 32` once, store the value in a password manager. **Never commit it.** Keep the same key for every release
   (an old sealed file needs the key it was sealed with).
2. `FG_SEAL_KEY=<key> FG_MODEL_BASE=<https url> scripts/release.sh` — builds the helper, seals `models/` into `dist-models/` (with
   `manifest.json`), builds the app with `tauri.release.conf.json` (the app contains **no** models), and fails if a model file is found
   in the bundle. Upload `dist-models/` to `FG_MODEL_BASE`; the app downloads a pack (standard, high, search) the first time it is needed.
3. Sign with a Developer ID and the **hardened runtime**, then notarise. The hardened runtime (without `get-task-allow`) is what
   actually blocks debuggers and code injection, and it is the strongest measure on the list.
4. Check the result: `find MyApp.app -name '*.onnx' -o -name '*.fgm'` must find nothing, and `strings` on the binary must show
   no `/Users/…` paths.

If `FG_SEAL_KEY` is missing the build falls back to a *public development key* and prints a warning: such a build is not protected.

## Later, if the risk grows

* Move `decide.ts` and the preference model into Rust.
* Bind the sealed models to a licence check (a per-user key), so a leaked build cannot be used freely.
* Use a smaller, distilled model of our own: what cannot be copied cannot be leaked.
