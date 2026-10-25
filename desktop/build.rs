fn main() {
    println!("cargo:rerun-if-changed=src/notifications/macos.m");
    println!("cargo:rerun-if-changed=src/file_picker/macos.m");
    println!("cargo:rerun-if-changed=src/appearance/macos.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/notifications/macos.m")
            .file("src/file_picker/macos.m")
            .file("src/appearance/macos.m")
            .flag("-fobjc-arc")
            .compile("dgl_notifications");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=UserNotifications");
        println!("cargo:rustc-link-lib=framework=AppKit");
    }
}
