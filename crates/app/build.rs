fn main() {
    // Debug info lets the testing backend find elements (tests/nav.rs). Release builds leave it out.
    let debug = std::env::var("PROFILE")
        .map(|p| p != "release")
        .unwrap_or(true);
    let config = slint_build::CompilerConfiguration::new().with_debug_info(debug);
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint UI failed to compile");
}
