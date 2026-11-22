//! Renders one page in a real window with a chosen renderer and saves it as PNG, then quits.
//! Used for the text ink check of the Skia spike (README, "Renderer"). It shows a window, so it runs only on a
//! hidden desktop (tools/hidden-run.ps1). On the normal desktop ("Default") it refuses to start.
//!
//! Build: `cargo build --release -p dr-app --features renderer-femtovg --example snapshot` (Skia, femtovg and
//! software all built in). Run through tools/hidden-run.ps1:
//! `snapshot <skia|skia-opengl|femtovg|software> <page> <out.png> [scale]`. The scale is the window scale factor
//! (SLINT_SCALE_FACTOR); the default is 1, which gives a 1440 x 900 PNG. Use 2.25 for a 225% screen.

use std::time::Duration;

use slint::ComponentHandle;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (renderer, page, out, scale) = match args.as_slice() {
        [r, p, o] => (r, p, o, "1"),
        [r, p, o, s] => (r, p, o, s.as_str()),
        _ => {
            eprintln!("usage: snapshot <renderer> <page> <out.png> [scale]");
            std::process::exit(2);
        }
    };
    let Some(page) = dr_app::page_from_name(page) else {
        eprintln!("unknown page: {page}");
        std::process::exit(2);
    };
    let scale: f32 = match scale.parse() {
        Ok(s) if s > 0.0 => s,
        _ => {
            eprintln!("bad scale: {scale}");
            std::process::exit(2);
        }
    };
    // Nothing before this line may create a window.
    match desktop::current_name() {
        Some(name) if !name.eq_ignore_ascii_case("Default") => eprintln!("desktop: {name}"),
        other => {
            eprintln!(
                "refused: this would show a window on the user's screen (desktop {other:?}). Run it through tools/hidden-run.ps1."
            );
            std::process::exit(3);
        }
    }
    // The winit backend reads the scale factor when it creates the window. Without it, the PNG follows the
    // screen scale (3240 x 2025 at 225%).
    // SAFETY: still single-threaded here; no other thread reads the environment.
    unsafe { std::env::set_var("SLINT_SCALE_FACTOR", scale.to_string()) };
    // Fails when the renderer is not built in, so the picture is always from the named renderer.
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name(renderer.clone())
        .select()
        .expect("select renderer");
    let w = dr_app::build_main_window().expect("window");
    w.set_page(page);
    w.window().set_size(slint::LogicalSize::new(1440.0, 900.0));
    w.show().expect("show");
    let weak = w.as_weak();
    let out = out.clone();
    slint::Timer::single_shot(Duration::from_millis(1500), move || {
        let w = weak.unwrap();
        let buf = w.window().take_snapshot().expect("snapshot");
        image::save_buffer(
            &out,
            buf.as_bytes(),
            buf.width(),
            buf.height(),
            image::ColorType::Rgba8,
        )
        .expect("save png");
        println!("saved {} x {} to {out}", buf.width(), buf.height());
        slint::quit_event_loop().expect("quit");
    });
    slint::run_event_loop().expect("event loop");
}

/// The name of the desktop this thread runs on, or None when it cannot be read (then the caller refuses).
#[cfg(windows)]
mod desktop {
    use std::ffi::c_void;

    const UOI_NAME: i32 = 2;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetThreadDesktop(thread_id: u32) -> *mut c_void;
        fn GetUserObjectInformationW(
            obj: *mut c_void,
            index: i32,
            info: *mut c_void,
            len: u32,
            needed: *mut u32,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }

    pub fn current_name() -> Option<String> {
        let mut buf = [0u16; 256];
        let mut needed = 0u32;
        // SAFETY: the desktop handle from GetThreadDesktop needs no close; the buffer is valid for its length in
        // bytes, and the call writes at most that many.
        let ok = unsafe {
            let desk = GetThreadDesktop(GetCurrentThreadId());
            if desk.is_null() {
                return None;
            }
            GetUserObjectInformationW(
                desk,
                UOI_NAME,
                buf.as_mut_ptr().cast(),
                size_of_val(&buf) as u32,
                &mut needed,
            )
        };
        if ok == 0 {
            return None;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..len]))
    }
}

#[cfg(not(windows))]
mod desktop {
    pub fn current_name() -> Option<String> {
        None
    }
}
