fn main() {
    // Debug info lets the testing backend find elements (tests/nav.rs). Release builds leave it out.
    let debug = std::env::var("PROFILE")
        .map(|p| p != "release")
        .unwrap_or(true);
    let config = slint_build::CompilerConfiguration::new().with_debug_info(debug);
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint UI failed to compile");

    // DialogReader.exe gets the manifest (asInvoker, per-monitor DPI v2), the icon and the version block (M5).
    // The other bins (render-screens, overlay-probe) and the tests do not.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=app.rc");
        println!("cargo:rerun-if-changed=app.manifest");
        println!("cargo:rerun-if-changed=app.ico");
        let version = env!("CARGO_PKG_VERSION");
        let n: Vec<u32> = version
            .split(['.', '-', '+'])
            .take(3)
            .map(|p| p.parse().unwrap_or(0))
            .collect();
        let macros = [
            format!("DR_V1={}", n.first().copied().unwrap_or(0)),
            format!("DR_V2={}", n.get(1).copied().unwrap_or(0)),
            format!("DR_V3={}", n.get(2).copied().unwrap_or(0)),
            format!("DR_VERSION=\"{version}\""),
        ];
        embed_resource::compile_for("app.rc", ["DialogReader"], &macros)
            .manifest_required()
            .expect("app.rc failed to compile");
    }
}
