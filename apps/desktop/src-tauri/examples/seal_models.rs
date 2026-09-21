//! Release-time tool: seals every model file of a directory.
//!
//!   FG_SEAL_KEY=<64 hex chars> cargo run --release --example seal_models -- ../models ../sealed-models
//!
//! The same FG_SEAL_KEY must be set when the app itself is built, or it cannot open what this tool wrote.

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (src, dst) = match args.as_slice() {
        [a, b] => (Path::new(a), Path::new(b)),
        _ => { eprintln!("usage: seal_models <models dir> <output dir>"); std::process::exit(2) }
    };
    std::fs::create_dir_all(dst).expect("create output dir");
    let mut n = 0u32;
    let mut manifest: Vec<String> = Vec::new();
    for e in std::fs::read_dir(src).expect("read models dir").filter_map(Result::ok) {
        let p = e.path();
        let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
        if !matches!(ext, "onnx" | "json" | "bin") { continue; }
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        // a fresh nonce for each file; time + name is plenty, it only has to be unique per key
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let mut nonce = [0u8; 12];
        for (i, b) in format!("{t}{name}").bytes().enumerate() { nonce[i % 12] = nonce[i % 12].wrapping_mul(31).wrapping_add(b); }
        let out = dst.join(format!("{name}.{}", fleetinggleam_lib::seal::EXT));
        fleetinggleam_lib::seal::seal_file(&p, &out, nonce).unwrap_or_else(|e| panic!("{name}: {e}"));
        println!("sealed {name} -> {}", out.display());
        let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
        let sha = fleetinggleam_lib::seal::sha256_file(&out).expect("checksum");
        manifest.push(format!("    \"{}\": {{ \"size\": {size}, \"sha256\": \"{sha}\" }}", out.file_name().unwrap().to_string_lossy()));
        n += 1;
    }
    manifest.sort();
    std::fs::write(dst.join("manifest.json"), format!("{{\n  \"files\": {{\n{}\n  }}\n}}\n", manifest.join(",\n"))).expect("write manifest.json");
    println!("{n} files sealed; upload the whole folder (with manifest.json) to the model server");
}
