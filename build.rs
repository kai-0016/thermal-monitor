fn main() {
    // 注册表、GDI、Shell API 所在的系统库
    for lib in ["advapi32", "gdi32", "shell32", "user32", "ole32"] {
        println!("cargo:rustc-link-lib={}", lib);
    }
}
