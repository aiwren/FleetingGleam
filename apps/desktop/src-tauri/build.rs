use std::{env, fs, path::Path};

/// The model-sealing key is never written into the source as one piece. It comes from the environment at build time (`FG_SEAL_KEY`,
/// 64 hex characters, kept out of the repository), is split into two shares here, and is only joined again in memory while a model is opened.
fn main() {
    println!("cargo:rerun-if-env-changed=FG_SEAL_KEY");
    let dev = "5f1e0c8a3b7d94a2c6e1f0839d2b47a6e8c15d3f7a90b2e4c6d81f3a5b7c9e01";
    let hex = env::var("FG_SEAL_KEY").unwrap_or_else(|_| {
        if env::var("PROFILE").map_or(false, |p| p == "release") {
            println!("cargo:warning=FG_SEAL_KEY is not set: this release build uses the public development key");
        }
        dev.to_string()
    });
    assert!(hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()), "FG_SEAL_KEY must be 64 hex characters");
    let key: Vec<u8> = (0..32).map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap()).collect();

    // Deterministic per key, so sealed files and the app built from the same key always agree.
    let mut seed = key.iter().fold(0xcbf29ce484222325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    let mut next = || {
        seed = seed.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    };
    let a: Vec<u8> = (0..32).map(|_| (next() >> 24) as u8).collect();
    let b: Vec<u8> = key.iter().zip(&a).map(|(k, x)| k ^ x).collect();
    let fmt = |v: &[u8]| v.iter().map(|x| format!("0x{x:02x}")).collect::<Vec<_>>().join(", ");
    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("seal_key.rs");
    fs::write(out, format!("const SHARE_A: [u8; 32] = [{}];\nconst SHARE_B: [u8; 32] = [{}];\n", fmt(&a), fmt(&b))).unwrap();

    tauri_build::build()
}
