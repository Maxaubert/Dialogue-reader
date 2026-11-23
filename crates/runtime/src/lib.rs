//! DialogReader runtime: everything the app needs that is not UI. No Slint here (docs/app-spec.md 2.1).
//!
//! M2: profiles on disk ([`profile`], [`paths`]), the list of windows to read ([`windows_list`]) and one
//! screenshot of a window with Windows Graphics Capture ([`screenshot`]), and the live view of the zone editor
//! ([`live`]). M3a: the speech queue, the speech thread and audio out ([`speech`]). M3b: frames for the reader
//! ([`capture`]), the reader thread ([`reader`]), the global keys ([`keys`]), elevated and fullscreen games
//! ([`elevation`]), the activity log ([`activity`]) and the handle that runs them ([`runtime`]).

pub mod activity;
pub mod capture;
pub mod keys;
pub mod paths;
pub mod profile;
pub mod reader;
pub mod shot;
pub mod speech;

#[cfg(windows)]
pub mod elevation;
#[cfg(windows)]
pub mod live;
#[cfg(windows)]
pub mod runtime;
#[cfg(windows)]
pub mod screenshot;
#[cfg(windows)]
pub mod testwin;
#[cfg(windows)]
pub mod windows_list;

pub use paths::Paths;
pub use profile::{Kind, Mode, Profile, ZoneCfg};
pub use shot::Shot;
