use std::{env, fs};

fn main() {
    build_hdr_overlay();
    tauri_build::build()
}

/// macOS only: the native parts (native/overlay.swift, the HDR layer of the viewer, see hdrview.rs; native/clip.swift, the CLIP encoder on the
/// GPU, see clipgpu.rs) are compiled here together and linked into the app.
fn build_hdr_overlay() {
    println!("cargo:rerun-if-changed=native/overlay.swift");
    println!("cargo:rerun-if-changed=native/clip.swift");
    if env::var("CARGO_CFG_TARGET_OS").map_or(true, |o| o != "macos") {
        return;
    }
    use std::process::Command;
    let out = env::var("OUT_DIR").unwrap();
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        _ => "x86_64",
    };
    let obj = format!("{out}/overlay.o");
    let ok = Command::new("swiftc")
        .args(["-O", "-wmo", "-parse-as-library", "-emit-object", "-target", &format!("{arch}-apple-macos15.0"), "native/overlay.swift", "native/clip.swift", "-o", &obj])
        .status()
        .map_or(false, |s| s.success());
    assert!(ok, "swiftc failed on native/overlay.swift and native/clip.swift (needs the Xcode command line tools)");
    let lib = format!("{out}/libfgoverlay.a");
    let _ = fs::remove_file(&lib);
    assert!(Command::new("ar").args(["rcs", &lib, &obj]).status().map_or(false, |s| s.success()), "ar failed");
    let sdk = Command::new("xcrun").args(["--sdk", "macosx", "--show-sdk-path"]).output().ok().and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
    println!("cargo:rustc-link-search=native={out}");
    println!("cargo:rustc-link-lib=static=fgoverlay");
    println!("cargo:rustc-link-search=native={}/usr/lib/swift", sdk.trim());
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    for f in ["AppKit", "QuartzCore", "ImageIO", "CoreGraphics", "Foundation", "Metal", "MetalPerformanceShaders", "MetalPerformanceShadersGraph"] {
        println!("cargo:rustc-link-lib=framework={f}");
    }
}
