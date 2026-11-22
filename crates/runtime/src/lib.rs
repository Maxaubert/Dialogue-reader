//! DialogReader runtime: everything the app needs that is not UI. No Slint here (docs/app-spec.md 2.1).
//!
//! M2: profiles on disk ([`profile`], [`paths`]), the list of windows to read ([`windows_list`]) and one
//! screenshot of a window with Windows Graphics Capture ([`screenshot`]), and the live view of the zone editor
//! ([`live`]). Capture, the reader thread, speech and
//! keys come in M3.

pub mod paths;
pub mod profile;
pub mod shot;

#[cfg(windows)]
pub mod live;
#[cfg(windows)]
pub mod screenshot;
#[cfg(windows)]
pub mod testwin;
#[cfg(windows)]
pub mod windows_list;

pub use paths::Paths;
pub use profile::{Kind, Mode, Profile, ZoneCfg};
pub use shot::Shot;
