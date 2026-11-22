//! The live view of a game in the zone editor (pages 03, 10 and 11): a continuous Windows Graphics Capture
//! session on the capture thread, each frame made small on that thread and handed to a [`LiveSink`].
//!
//! - [`Live::start`] returns at once. The session is built on a helper thread (Windows took 39 to 271 ms for
//!   that in the review of 2026-11-22), so the caller (the UI thread) never waits. A start Windows refuses comes to the sink as `ended`.
//! - Frames come at most every `interval` (`MinimumUpdateIntervalSettings`, plus a time check before the
//!   readback, since older Windows builds ignore the setting). WGC sends a frame only when the window changed,
//!   so silence is normal (a paused game) and never an error.
//! - The GPU makes the frame the target size (a mip chain, then one draw with a trilinear sampler into a
//!   texture of exactly that size), and only that small picture is read back. The cost then does not depend on
//!   how the stage compares to the capture. If any D3D call fails, the full frame is read back and
//!   box-filtered on the CPU instead (much dearer: see crates/app/README.md). The target can change while it
//!   runs; it is never larger than the capture.
//! - The capture size (`src_w` x `src_h`) comes with every frame: zones stay in capture pixels.
//! - [`Live::stop`] (or drop) ends the session at once for the sink: no frame is handed over after it returns.
//!   The capture thread itself is stopped and joined on a helper thread, so the caller never waits for it.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};
use windows_capture::window::Window;

use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, ID3DBlob};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_COMPARISON_NEVER,
    D3D11_CPU_ACCESS_READ, D3D11_CULL_NONE, D3D11_FILL_SOLID, D3D11_FILTER_MIN_MAG_MIP_LINEAR,
    D3D11_FLOAT32_MAX, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_RASTERIZER_DESC,
    D3D11_RESOURCE_MISC_GENERATE_MIPS, D3D11_SAMPLER_DESC, D3D11_TEXTURE_ADDRESS_CLAMP,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING, D3D11_VIEWPORT, ID3D11Device,
    ID3D11PixelShader, ID3D11RasterizerState, ID3D11RenderTargetView, ID3D11SamplerState,
    ID3D11ShaderResourceView, ID3D11Texture2D, ID3D11VertexShader,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_SAMPLE_DESC};
use windows::core::PCSTR;

/// About 15 frames a second.
pub const INTERVAL: Duration = Duration::from_millis(66);

/// One small frame. `rgba` is `w * h * 4` bytes, tight rows, alpha 255.
pub struct LiveFrame<'a> {
    /// Size of the capture (the game window), the pixels zones are in.
    pub src_w: u32,
    pub src_h: u32,
    pub w: u32,
    pub h: u32,
    pub rgba: &'a [u8],
}

/// Where frames go. Runs on the capture thread (or the start helper thread, for a refused start).
pub trait LiveSink: Send + 'static {
    fn frame(&mut self, f: &LiveFrame);
    /// The window closed or the capture failed (also a start Windows refused). No frame follows.
    fn ended(&mut self, why: &str);
}

/// What the session cost so far (for the CPU budget in crates/app/README.md).
#[derive(Clone, Copy, Debug, Default)]
pub struct LiveStats {
    /// Frames handed to the sink.
    pub frames: u64,
    /// Frames WGC sent that were skipped before the readback (too soon).
    pub skipped: u64,
    /// Frames made on the GPU (the rest took the CPU path).
    pub gpu_frames: u64,
    /// Time in the readback (GPU scale, copy to a staging texture and map; or the full frame on the CPU path),
    /// summed.
    pub readback: Duration,
    /// Time in the copy out of the mapped texture (GPU path) or the CPU box filter, summed.
    pub scale: Duration,
    /// Time in the sink, summed.
    pub sink: Duration,
}

struct Shared {
    /// Cleared by `stop`. Checked early in each frame to skip work; the gate below is what makes it final.
    alive: AtomicBool,
    /// Held around the last open check and the sink call, and while `stop` clears it: after `stop` returns, no
    /// frame or end reaches the sink. Only the sink call runs under it, not the frame's work.
    gate: Mutex<bool>,
    /// The session is still being built on the start helper thread.
    starting: AtomicBool,
    /// Read back the full frame and scale on the CPU (tests and the cost measurement).
    cpu_only: AtomicBool,
    max_w: AtomicU32,
    max_h: AtomicU32,
    frames: AtomicU64,
    skipped: AtomicU64,
    gpu_frames: AtomicU64,
    readback_ns: AtomicU64,
    scale_ns: AtomicU64,
    sink_ns: AtomicU64,
}

impl Shared {
    fn gate(&self) -> MutexGuard<'_, bool> {
        self.gate.lock().unwrap_or_else(|e| e.into_inner())
    }
}

struct Flags {
    shared: Arc<Shared>,
    sink: Box<dyn LiveSink>,
    interval: Duration,
}

struct Handler {
    shared: Arc<Shared>,
    sink: Box<dyn LiveSink>,
    interval: Duration,
    last: Option<Instant>,
    out: Vec<u8>,
    ended: bool,
    gpu: Gpu,
    taps: Option<Taps>,
}

impl Handler {
    fn end(&mut self, why: &str) {
        let open = self.shared.gate();
        if !self.ended && *open {
            self.ended = true;
            self.sink.ended(why);
        }
    }
}

impl GraphicsCaptureApiHandler for Handler {
    type Flags = Flags;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let f = ctx.flags;
        Ok(Handler {
            shared: f.shared,
            sink: f.sink,
            interval: f.interval,
            last: None,
            out: Vec::new(),
            ended: false,
            gpu: Gpu::default(),
            taps: None,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame,
        control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let s = self.shared.clone();
        if !s.alive.load(Ordering::Acquire) {
            control.stop();
            return Ok(());
        }
        let now = Instant::now();
        // a little early is fine: WGC's own interval already spaces the frames
        if self
            .last
            .is_some_and(|t| now.duration_since(t) + Duration::from_millis(8) < self.interval)
        {
            s.skipped.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        self.last = Some(now);
        let (src_w, src_h) = (frame.width(), frame.height());
        let (max_w, max_h) = (
            s.max_w.load(Ordering::Relaxed),
            s.max_h.load(Ordering::Relaxed),
        );
        let (tw, th) = fit(src_w, src_h, max_w, max_h);
        // the GPU makes the picture the target size; any D3D error falls back to the full readback
        let gpu = if s.cpu_only.load(Ordering::Relaxed) {
            None
        } else {
            self.gpu.read(frame, tw, th, &mut self.out).ok()
        };
        let (t1, t2) = match gpu {
            Some(t1) => {
                s.gpu_frames.fetch_add(1, Ordering::Relaxed);
                (t1, Instant::now())
            }
            None => {
                let mut fb = match frame.buffer() {
                    Ok(fb) => fb,
                    Err(e) => {
                        self.end(&format!("could not read the captured frame: {e}"));
                        control.stop();
                        return Ok(());
                    }
                };
                let t1 = Instant::now();
                let pitch = fb.row_pitch() as usize;
                let raw = fb.as_raw_buffer();
                self.out.resize((tw * th * 4) as usize, 0);
                let taps = match self.taps.take() {
                    Some(t) if t.is_for(src_w, src_h, tw, th) => t,
                    _ => Taps::new(src_w, src_h, tw, th),
                };
                downscale_with(&taps, raw, pitch, &mut self.out);
                self.taps = Some(taps);
                (t1, Instant::now())
            }
        };
        // stop() may have come while this frame was made: it must not reach the sink then
        let open = s.gate();
        if !*open {
            drop(open);
            control.stop();
            return Ok(());
        }
        self.sink.frame(&LiveFrame {
            src_w,
            src_h,
            w: tw,
            h: th,
            rgba: &self.out,
        });
        drop(open);
        let t3 = Instant::now();
        s.frames.fetch_add(1, Ordering::Relaxed);
        s.readback_ns
            .fetch_add((t1 - now).as_nanos() as u64, Ordering::Relaxed);
        s.scale_ns
            .fetch_add((t2 - t1).as_nanos() as u64, Ordering::Relaxed);
        s.sink_ns
            .fetch_add((t3 - t2).as_nanos() as u64, Ordering::Relaxed);
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.end("the window closed");
        Ok(())
    }
}

type Control = CaptureControl<Handler, Box<dyn std::error::Error + Send + Sync>>;

/// A running live view. Stops on drop.
pub struct Live {
    shared: Arc<Shared>,
    /// Set by the start helper thread once the session runs.
    control: Arc<Mutex<Option<Control>>>,
}

impl Live {
    /// Starts capturing `hwnd` (`HWND.0 as isize`); frames fit inside `max_w` x `max_h`. Returns at once: the
    /// session is built on a helper thread, and a start Windows refuses comes to the sink as `ended`. Err only
    /// for a window that is already gone.
    pub fn start(
        hwnd: isize,
        max_w: u32,
        max_h: u32,
        interval: Duration,
        sink: Box<dyn LiveSink>,
    ) -> Result<Live, String> {
        Self::start_with(hwnd, max_w, max_h, interval, false, sink)
    }

    /// As [`Live::start`]; `cpu_only` skips the GPU scale (tests and the cost measurement).
    pub fn start_with(
        hwnd: isize,
        max_w: u32,
        max_h: u32,
        interval: Duration,
        cpu_only: bool,
        sink: Box<dyn LiveSink>,
    ) -> Result<Live, String> {
        if !crate::windows_list::is_open(hwnd) {
            return Err("the window is gone".into());
        }
        let shared = Arc::new(Shared {
            alive: AtomicBool::new(true),
            gate: Mutex::new(true),
            starting: AtomicBool::new(true),
            cpu_only: AtomicBool::new(cpu_only),
            max_w: AtomicU32::new(max_w.max(1)),
            max_h: AtomicU32::new(max_h.max(1)),
            frames: AtomicU64::new(0),
            skipped: AtomicU64::new(0),
            gpu_frames: AtomicU64::new(0),
            readback_ns: AtomicU64::new(0),
            scale_ns: AtomicU64::new(0),
            sink_ns: AtomicU64::new(0),
        });
        let control = Arc::new(Mutex::new(None));
        let (sh, ctl) = (shared.clone(), control.clone());
        let spawned = std::thread::Builder::new()
            .name("live-start".into())
            .spawn(move || {
                // the sink moves into each attempt; a failed attempt gives it back through the slot
                let slot = Arc::new(Mutex::new(Some(sink)));
                match build(hwnd, interval, &sh, &slot) {
                    Ok(c) => {
                        let mut g = ctl.lock().unwrap_or_else(|e| e.into_inner());
                        if sh.alive.load(Ordering::Acquire) {
                            *g = Some(c);
                        } else {
                            // stopped while it started
                            drop(g);
                            let _ = c.stop();
                        }
                    }
                    Err(e) => {
                        let sink = slot.lock().ok().and_then(|mut s| s.take());
                        let open = sh.gate();
                        if let (Some(mut sink), true) = (sink, *open) {
                            sink.ended(&e);
                        }
                    }
                }
                sh.starting.store(false, Ordering::Release);
            });
        if spawned.is_err() {
            return Err("the live view could not start".into());
        }
        Ok(Live { shared, control })
    }

    /// A new target size (the stage changed size).
    pub fn set_target(&self, max_w: u32, max_h: u32) {
        self.shared.max_w.store(max_w.max(1), Ordering::Relaxed);
        self.shared.max_h.store(max_h.max(1), Ordering::Relaxed);
    }

    pub fn stats(&self) -> LiveStats {
        let s = &self.shared;
        LiveStats {
            frames: s.frames.load(Ordering::Relaxed),
            skipped: s.skipped.load(Ordering::Relaxed),
            gpu_frames: s.gpu_frames.load(Ordering::Relaxed),
            readback: Duration::from_nanos(s.readback_ns.load(Ordering::Relaxed)),
            scale: Duration::from_nanos(s.scale_ns.load(Ordering::Relaxed)),
            sink: Duration::from_nanos(s.sink_ns.load(Ordering::Relaxed)),
        }
    }

    /// The session ended: the window closed, the capture failed or the start was refused.
    pub fn is_finished(&self) -> bool {
        !self.shared.starting.load(Ordering::Acquire)
            && self
                .control
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .is_none_or(|c| c.is_finished())
    }

    /// Stops it: no frame reaches the sink after this returns. It waits at most for a sink call in progress
    /// (one copy of a small frame); the capture thread is stopped and joined on a helper thread.
    pub fn stop(mut self) {
        self.halt();
    }

    fn halt(&mut self) {
        *self.shared.gate() = false;
        self.shared.alive.store(false, Ordering::Release);
        let c = self
            .control
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        // a session still starting is stopped by the start helper thread when it sees `alive` cleared
        if let Some(c) = c {
            std::thread::spawn(move || {
                let _ = c.stop();
            });
        }
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.halt();
    }
}

/// Builds the session (on the start helper thread), with relaxed settings when Windows refuses some.
fn build(
    hwnd: isize,
    interval: Duration,
    shared: &Arc<Shared>,
    slot: &Arc<Mutex<Option<Box<dyn LiveSink>>>>,
) -> Result<Control, String> {
    let mut cursor = CursorCaptureSettings::WithoutCursor;
    let mut border = DrawBorderSettings::WithoutBorder;
    let mut every = MinimumUpdateIntervalSettings::Custom(interval);
    loop {
        if !shared.alive.load(Ordering::Acquire) {
            return Err("stopped".into());
        }
        let Some(sink) = slot.lock().ok().and_then(|mut s| s.take()) else {
            return Err("the live view could not start".into());
        };
        let back = SinkBack {
            slot: slot.clone(),
            sink: Some(sink),
        };
        let settings = Settings::new(
            Window::from_raw_hwnd(hwnd as *mut std::ffi::c_void),
            cursor,
            border,
            SecondaryWindowSettings::Default,
            every,
            DirtyRegionSettings::Default,
            ColorFormat::Rgba8,
            Flags {
                shared: shared.clone(),
                sink: Box::new(back),
                interval,
            },
        );
        match Handler::start_free_threaded(settings) {
            Ok(control) => return Ok(control),
            Err(e) => {
                if crate::screenshot::relax(&e, &mut cursor, &mut border, &mut every) {
                    continue;
                }
                return Err(format!("Windows could not capture this window ({e})"));
            }
        }
    }
}

/// Forwards to the sink, and gives it back to the start when an attempt fails.
struct SinkBack {
    slot: Arc<Mutex<Option<Box<dyn LiveSink>>>>,
    sink: Option<Box<dyn LiveSink>>,
}

impl LiveSink for SinkBack {
    fn frame(&mut self, f: &LiveFrame) {
        if let Some(s) = &mut self.sink {
            s.frame(f);
        }
    }
    fn ended(&mut self, why: &str) {
        if let Some(s) = &mut self.sink {
            s.ended(why);
        }
    }
}

impl Drop for SinkBack {
    fn drop(&mut self) {
        if let (Some(s), Ok(mut slot)) = (self.sink.take(), self.slot.lock())
            && slot.is_none()
        {
            *slot = Some(s);
        }
    }
}

/// The largest size with the aspect of `src_w` x `src_h` inside `max_w` x `max_h`, never larger than the source.
pub fn fit(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if src_w == 0 || src_h == 0 {
        return (1, 1);
    }
    let k = (max_w as f64 / src_w as f64)
        .min(max_h as f64 / src_h as f64)
        .min(1.0);
    (
        ((src_w as f64 * k).round() as u32).clamp(1, src_w),
        ((src_h as f64 * k).round() as u32).clamp(1, src_h),
    )
}

/// The scale shaders: one triangle that covers the target, sampling the mip chain with a trilinear sampler (the
/// GPU picks the mip level from the size ratio). Alpha is 1.
const SHADER: &str = r#"
Texture2D src : register(t0);
SamplerState smp : register(s0);
struct V { float4 pos : SV_Position; float2 uv : TEXCOORD0; };
V vs(uint id : SV_VertexID) {
    V o;
    float2 uv = float2((id << 1) & 2, id & 2);
    o.uv = uv;
    o.pos = float4(uv * float2(2, -2) + float2(-1, 1), 0, 1);
    return o;
}
float4 ps(V i) : SV_Target {
    return float4(src.Sample(smp, i.uv).rgb, 1);
}
"#;

/// The compiled shaders (vertex, pixel), made once per process. None: the compiler failed (CPU path).
fn shader_code() -> Option<&'static (Vec<u8>, Vec<u8>)> {
    static CODE: OnceLock<Option<(Vec<u8>, Vec<u8>)>> = OnceLock::new();
    CODE.get_or_init(|| Some((compile(c"vs", c"vs_4_0")?, compile(c"ps", c"ps_4_0")?)))
        .as_ref()
}

fn compile(entry: &std::ffi::CStr, target: &std::ffi::CStr) -> Option<Vec<u8>> {
    let mut code: Option<ID3DBlob> = None;
    unsafe {
        D3DCompile(
            SHADER.as_ptr().cast(),
            SHADER.len(),
            PCSTR::null(),
            None,
            None,
            PCSTR(entry.as_ptr().cast()),
            PCSTR(target.as_ptr().cast()),
            0,
            0,
            &mut code,
            None,
        )
        .ok()?;
        let code = code?;
        let bytes =
            std::slice::from_raw_parts(code.GetBufferPointer() as *const u8, code.GetBufferSize());
        Some(bytes.to_vec())
    }
}

/// Shaders and states, made once per session.
struct Pipeline {
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    sampler: ID3D11SamplerState,
    raster: ID3D11RasterizerState,
}

impl Pipeline {
    fn new(dev: &ID3D11Device) -> Option<Pipeline> {
        let (vcode, pcode) = shader_code()?;
        let mut vs = None;
        let mut ps = None;
        let mut sampler = None;
        let mut raster = None;
        unsafe {
            dev.CreateVertexShader(vcode, None, Some(&mut vs)).ok()?;
            dev.CreatePixelShader(pcode, None, Some(&mut ps)).ok()?;
            let sd = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                MipLODBias: 0.0,
                MaxAnisotropy: 1,
                ComparisonFunc: D3D11_COMPARISON_NEVER,
                BorderColor: [0.0; 4],
                MinLOD: 0.0,
                MaxLOD: D3D11_FLOAT32_MAX,
            };
            dev.CreateSamplerState(&sd, Some(&mut sampler)).ok()?;
            let rd = D3D11_RASTERIZER_DESC {
                FillMode: D3D11_FILL_SOLID,
                CullMode: D3D11_CULL_NONE,
                DepthClipEnable: true.into(),
                ..Default::default()
            };
            dev.CreateRasterizerState(&rd, Some(&mut raster)).ok()?;
        }
        Some(Pipeline {
            vs: vs?,
            ps: ps?,
            sampler: sampler?,
            raster: raster?,
        })
    }
}

/// D3D11 objects of the GPU path, kept from frame to frame (made again when a size changes).
#[derive(Default)]
struct Gpu {
    pipe: Option<Pipeline>,
    /// A texture with a full mip chain, the capture size.
    mips: Option<(u32, u32, ID3D11Texture2D, ID3D11ShaderResourceView)>,
    /// The target-size texture the scale draws into.
    target: Option<(u32, u32, ID3D11Texture2D, ID3D11RenderTargetView)>,
    /// A CPU-readable texture of the target size.
    staging: Option<(u32, u32, ID3D11Texture2D)>,
    /// The GPU path failed once: use the CPU path from now on.
    broken: bool,
}

fn tex_desc(w: u32, h: u32, format: DXGI_FORMAT) -> D3D11_TEXTURE2D_DESC {
    D3D11_TEXTURE2D_DESC {
        Width: w,
        Height: h,
        MipLevels: 1,
        ArraySize: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: 0,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    }
}

impl Gpu {
    /// Makes the frame `tw` x `th` on the GPU and reads it into `out` (tight RGBA). Gives when the readback
    /// ended.
    fn read(
        &mut self,
        frame: &mut Frame,
        tw: u32,
        th: u32,
        out: &mut Vec<u8>,
    ) -> Result<Instant, ()> {
        if self.broken {
            return Err(());
        }
        let r = self.try_read(frame, tw, th, out);
        if r.is_err() {
            self.broken = true;
            self.pipe = None;
            self.mips = None;
            self.target = None;
            self.staging = None;
        }
        r
    }

    fn try_read(
        &mut self,
        frame: &mut Frame,
        tw: u32,
        th: u32,
        out: &mut Vec<u8>,
    ) -> Result<Instant, ()> {
        let (sw, sh) = (frame.width(), frame.height());
        let format = frame.desc().Format;
        let dev = frame.device().clone();
        let ctx = frame.device_context().clone();
        if self.staging.as_ref().is_none_or(|s| (s.0, s.1) != (tw, th)) {
            self.staging = None;
            let d = D3D11_TEXTURE2D_DESC {
                Usage: D3D11_USAGE_STAGING,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                ..tex_desc(tw, th, format)
            };
            let mut tex = None;
            unsafe { dev.CreateTexture2D(&d, None, Some(&mut tex)) }.map_err(|_| ())?;
            self.staging = Some((tw, th, tex.ok_or(())?));
        }
        if (tw, th) != (sw, sh) {
            if self.pipe.is_none() {
                self.pipe = Some(Pipeline::new(&dev).ok_or(())?);
            }
            if self.mips.as_ref().is_none_or(|m| (m.0, m.1) != (sw, sh)) {
                self.mips = None;
                let d = D3D11_TEXTURE2D_DESC {
                    MipLevels: 0,
                    BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
                    MiscFlags: D3D11_RESOURCE_MISC_GENERATE_MIPS.0 as u32,
                    ..tex_desc(sw, sh, format)
                };
                let mut tex = None;
                unsafe { dev.CreateTexture2D(&d, None, Some(&mut tex)) }.map_err(|_| ())?;
                let tex = tex.ok_or(())?;
                let mut srv = None;
                unsafe { dev.CreateShaderResourceView(&tex, None, Some(&mut srv)) }
                    .map_err(|_| ())?;
                self.mips = Some((sw, sh, tex, srv.ok_or(())?));
            }
            if self.target.as_ref().is_none_or(|t| (t.0, t.1) != (tw, th)) {
                self.target = None;
                let d = D3D11_TEXTURE2D_DESC {
                    BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                    ..tex_desc(tw, th, format)
                };
                let mut tex = None;
                unsafe { dev.CreateTexture2D(&d, None, Some(&mut tex)) }.map_err(|_| ())?;
                let tex = tex.ok_or(())?;
                let mut rtv = None;
                unsafe { dev.CreateRenderTargetView(&tex, None, Some(&mut rtv)) }
                    .map_err(|_| ())?;
                self.target = Some((tw, th, tex, rtv.ok_or(())?));
            }
        }
        let Some((_, _, staging)) = &self.staging else {
            return Err(());
        };
        if (tw, th) == (sw, sh) {
            // no scale (a capture smaller than the stage): copy the frame as it is
            unsafe { ctx.CopyResource(staging, frame.as_raw_texture()) };
        } else {
            let (Some(p), Some((_, _, mips, srv)), Some((_, _, target, rtv))) =
                (&self.pipe, &self.mips, &self.target)
            else {
                return Err(());
            };
            let vp = D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: tw as f32,
                Height: th as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            };
            unsafe {
                ctx.CopySubresourceRegion(mips, 0, 0, 0, 0, frame.as_raw_texture(), 0, None);
                ctx.GenerateMips(srv);
                // every state the draw needs, each frame (the context is shared with windows-capture)
                ctx.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
                ctx.OMSetBlendState(None, None, 0xffff_ffff);
                ctx.OMSetDepthStencilState(None, 0);
                ctx.RSSetViewports(Some(&[vp]));
                ctx.RSSetState(&p.raster);
                ctx.IASetInputLayout(None);
                ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                ctx.VSSetShader(&p.vs, None);
                ctx.PSSetShader(&p.ps, None);
                ctx.PSSetShaderResources(0, Some(&[Some(srv.clone())]));
                ctx.PSSetSamplers(0, Some(&[Some(p.sampler.clone())]));
                ctx.Draw(3, 0);
                // unbound, so the next frame can write the mip texture and the target freely
                ctx.PSSetShaderResources(0, Some(&[None]));
                ctx.OMSetRenderTargets(None, None);
                ctx.CopyResource(staging, target);
            }
        }
        let mut m = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe { ctx.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut m)) }.map_err(|_| ())?;
        let t1 = Instant::now();
        let pitch = m.RowPitch as usize;
        let len = pitch * (th as usize - 1) + tw as usize * 4;
        let raw = unsafe { std::slice::from_raw_parts(m.pData as *const u8, len) };
        out.resize((tw * th * 4) as usize, 0);
        copy_rows(raw, pitch, tw, th, out);
        unsafe { ctx.Unmap(staging, 0) };
        Ok(t1)
    }
}

/// Tight RGBA rows out of `src` (rows `pitch` bytes apart), alpha 255.
fn copy_rows(src: &[u8], pitch: usize, w: u32, h: u32, dst: &mut [u8]) {
    let row = w as usize * 4;
    for (y, out) in dst.chunks_exact_mut(row).enumerate().take(h as usize) {
        out.copy_from_slice(&src[y * pitch..y * pitch + row]);
        for p in out.as_chunks_mut::<4>().0 {
            p[3] = 255;
        }
    }
}

/// Most samples per axis of one box. A box wider than this (more than 4x smaller) is sampled evenly, which
/// keeps the cost bounded for small stages.
const MAX_TAPS: u32 = 4;

/// Which source columns (as byte offsets) and rows each output pixel averages, for one pair of sizes. Kept from
/// frame to frame while the sizes stay.
pub struct Taps {
    key: (u32, u32, u32, u32),
    cols: Vec<Vec<usize>>,
    rows: Vec<Vec<usize>>,
}

impl Taps {
    pub fn new(sw: u32, sh: u32, dw: u32, dh: u32) -> Taps {
        // up to MAX_TAPS source positions for each of `dn` outputs
        let taps = |dn: u32, sn: u32| -> Vec<Vec<usize>> {
            (0..dn)
                .map(|d| {
                    let a = (d as u64 * sn as u64 / dn as u64) as u32;
                    let b =
                        (((d + 1) as u64 * sn as u64).div_ceil(dn as u64) as u32).clamp(a + 1, sn);
                    let n = (b - a).min(MAX_TAPS);
                    (0..n)
                        .map(|k| (a + (k * (b - a) + (b - a) / 2) / n.max(1)).min(sn - 1) as usize)
                        .collect()
                })
                .collect()
        };
        let (cols, rows) = if dw == 0 || dh == 0 || sw == 0 || sh == 0 {
            (Vec::new(), Vec::new())
        } else {
            (
                taps(dw, sw)
                    .into_iter()
                    .map(|c| c.into_iter().map(|x| x * 4).collect())
                    .collect(),
                taps(dh, sh),
            )
        };
        Taps {
            key: (sw, sh, dw, dh),
            cols,
            rows,
        }
    }

    pub fn is_for(&self, sw: u32, sh: u32, dw: u32, dh: u32) -> bool {
        self.key == (sw, sh, dw, dh)
    }
}

/// Box filter from `src` (RGBA rows `pitch` bytes apart) to tight RGBA `dst` of `dw` x `dh`. Alpha is 255.
pub fn downscale(src: &[u8], pitch: usize, sw: u32, sh: u32, dst: &mut [u8], dw: u32, dh: u32) {
    downscale_with(&Taps::new(sw, sh, dw, dh), src, pitch, dst);
}

/// [`downscale`] with taps made before.
pub fn downscale_with(t: &Taps, src: &[u8], pitch: usize, dst: &mut [u8]) {
    let (sw, sh, dw, dh) = t.key;
    if dw == 0 || dh == 0 || sw == 0 || sh == 0 {
        return;
    }
    if (dw, dh) == (sw, sh) {
        // same size: copy the rows
        copy_rows(src, pitch, sw, sh, dst);
        return;
    }
    for (y, ry) in t.rows.iter().enumerate() {
        let out = &mut dst[y * dw as usize * 4..(y + 1) * dw as usize * 4];
        for (x, cx) in t.cols.iter().enumerate() {
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for &sy in ry {
                let row = &src[sy * pitch..];
                for &o in cx {
                    r += row[o] as u32;
                    g += row[o + 1] as u32;
                    b += row[o + 2] as u32;
                }
            }
            let n = (ry.len() * cx.len()) as u32;
            let o = x * 4;
            out[o] = ((r + n / 2) / n) as u8;
            out[o + 1] = ((g + n / 2) / n) as u8;
            out[o + 2] = ((b + n / 2) / n) as u8;
            out[o + 3] = 255;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_keeps_the_aspect_and_never_grows() {
        assert_eq!(fit(3840, 2160, 1962, 1102), (1959, 1102));
        assert_eq!(fit(3840, 2160, 1000, 2000), (1000, 563));
        assert_eq!(fit(800, 600, 4000, 4000), (800, 600));
        assert_eq!(fit(0, 10, 100, 100), (1, 1));
    }

    #[test]
    fn downscale_averages_boxes_and_skips_row_padding() {
        // 4 x 2, left half red, right half blue, 8 bytes of padding per row
        let pitch = 4 * 4 + 8;
        let mut src = vec![7u8; pitch * 2];
        for y in 0..2 {
            for x in 0..4 {
                let px = if x < 2 {
                    [200, 0, 0, 255]
                } else {
                    [0, 0, 100, 255]
                };
                src[y * pitch + x * 4..y * pitch + x * 4 + 4].copy_from_slice(&px);
            }
        }
        let mut dst = vec![0u8; 2 * 4];
        downscale(&src, pitch, 4, 2, &mut dst, 2, 1);
        assert_eq!(dst, vec![200, 0, 0, 255, 0, 0, 100, 255]);
        // one pixel: the mean of all
        let mut one = vec![0u8; 4];
        downscale(&src, pitch, 4, 2, &mut one, 1, 1);
        assert_eq!(one, vec![100, 0, 50, 255]);
    }

    #[test]
    fn downscale_to_the_same_size_copies_rows() {
        let pitch = 2 * 4 + 4;
        let src = [
            1u8, 2, 3, 0, 4, 5, 6, 0, 9, 9, 9, 9, 7, 8, 9, 0, 10, 11, 12, 0, 9, 9, 9, 9,
        ];
        let mut dst = vec![0u8; 16];
        downscale(&src, pitch, 2, 2, &mut dst, 2, 2);
        assert_eq!(
            dst,
            [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
        );
    }

    #[test]
    fn downscale_of_a_big_factor_stays_in_bounds() {
        let (sw, sh) = (37u32, 23u32);
        let src = vec![255u8; (sw * sh * 4) as usize];
        let mut dst = vec![0u8; 3 * 2 * 4];
        downscale(&src, (sw * 4) as usize, sw, sh, &mut dst, 3, 2);
        assert!(dst.iter().all(|&v| v == 255));
    }
}
