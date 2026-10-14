fn main() {
    println!("cargo:rerun-if-changed=assets/3dviewer.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/3dviewer.ico")
            .set("ProductName", "3DViewer")
            .set("FileDescription", "3D model viewer")
            .compile()
            .expect("Failed to embed the Windows application icon");
    }
}
