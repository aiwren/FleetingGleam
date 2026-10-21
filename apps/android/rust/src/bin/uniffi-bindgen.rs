//! 生成 Kotlin 绑定的命令行：`cargo run -p fleetinggleam-android --bin uniffi-bindgen -- generate --library <libfg_android.so> --language kotlin --out-dir <目录>`。
fn main() {
    uniffi::uniffi_bindgen_main()
}
