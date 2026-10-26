fn main() {
    println!("cargo:rerun-if-changed=resources/windows/app.rc");
    println!("cargo:rerun-if-changed=resources/windows/dgl.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // GPUI 0.2.2 loads resource ID 1 from the executable for its windows.
        embed_resource::compile_for(
            "resources/windows/app.rc",
            ["dgl-desktop"],
            embed_resource::NONE,
        )
        .manifest_required()
        .expect("compile the DGL Windows app icon");
    }
    println!("cargo:rerun-if-changed=src/notifications/macos.m");
    println!("cargo:rerun-if-changed=src/file_picker/macos.m");
    println!("cargo:rerun-if-changed=src/appearance/macos.m");
    println!("cargo:rerun-if-changed=src/native_tooltips/macos.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/notifications/macos.m")
            .file("src/file_picker/macos.m")
            .file("src/appearance/macos.m")
            .file("src/native_tooltips/macos.m")
            .flag("-fobjc-arc")
            .compile("dgl_notifications");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=UserNotifications");
        println!("cargo:rustc-link-lib=framework=AppKit");
    }
}
