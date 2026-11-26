//! Quick menu keys through a low-level keyboard hook, so the menu works while the game keeps focus.
//!
//! The hook runs on its own thread with its own message loop, never on the Slint UI thread. Windows calls a
//! low-level hook for every key on the PC and waits for it: on the UI thread, every render, GL window
//! creation or snapshot would delay all keyboard input, and Windows silently removes a hook that answers
//! too slowly. So the handler runs on the hook thread. It only decides "swallow or not" from flags it can
//! read at once (atomics) and posts the work to the UI thread (`slint::invoke_from_event_loop`).
//!
//! `Swallow` holds the one rule that needs state: a key belongs to whoever got its first key-down, until it
//! is released. Its auto-repeats and its key-up go the same way. So after Esc closes the menu on its
//! key-down, the game does not get the Esc key-up either, and a key that was held before the menu opened
//! still reaches the game (repeats and key-up), so the game never keeps a stuck key.
//!
//! Injected keys (`LLKHF_INJECTED`, sent by `SendInput`, for example by Steam Input or AutoHotkey when they
//! map a controller button to a key) are treated like real keys, by choice (open for Max): a player who maps
//! a pad button to the menu key expects it to work. The event carries the flag, so M4 can change this in one
//! place.
//!
//! The hidden-desktop probe never swallows, so it can do no harm if it ever saw real input; only the visible
//! `demo` mode swallows menu keys while its menu is open. `hook_proc` itself never runs in the headless
//! tests (no real keys reach a hidden desktop); `Swallow` and `decode` are unit tested.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_BACK, VK_DELETE, VK_DOWN, VK_ESCAPE, VK_LEFT, VK_RETURN, VK_RIGHT, VK_SPACE, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, PM_NOREMOVE,
    PeekMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN,
    WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

/// A key the quick menu or zone edit uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Space,
    Esc,
    Delete,
}

/// Hook message + virtual key -> menu key. Key-up messages give None.
pub fn decode(msg: u32, vk: u32) -> Option<MenuKey> {
    if msg != WM_KEYDOWN && msg != WM_SYSKEYDOWN {
        return None;
    }
    let vk = vk as u16;
    Some(match vk {
        v if v == VK_UP.0 => MenuKey::Up,
        v if v == VK_DOWN.0 => MenuKey::Down,
        v if v == VK_LEFT.0 => MenuKey::Left,
        v if v == VK_RIGHT.0 => MenuKey::Right,
        v if v == VK_RETURN.0 => MenuKey::Enter,
        v if v == VK_SPACE.0 => MenuKey::Space,
        v if v == VK_ESCAPE.0 => MenuKey::Esc,
        v if v == VK_DELETE.0 || v == VK_BACK.0 => MenuKey::Delete,
        _ => return None,
    })
}

/// A decoded key-down for the handler: a menu key, or another key (by virtual key code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Menu(MenuKey),
    Other(u32),
}

/// One key-down as the handler sees it. Key-ups never reach the handler (`Swallow` decides them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    /// The key was already down (auto-repeat).
    pub repeat: bool,
    /// Sent by `SendInput` (`LLKHF_INJECTED`), not by a keyboard.
    pub injected: bool,
}

impl KeyEvent {
    /// A first key-down from a keyboard (what `feed` uses).
    pub fn new(key: Key) -> Self {
        KeyEvent { key, repeat: false, injected: false }
    }
}

/// Which keys are down, and which of those had their key-down swallowed. Pure, so it is unit tested.
#[derive(Default)]
pub struct Swallow {
    held: [u64; 4],
    swallowed: [u64; 4],
}

fn bit(set: &[u64; 4], vk: u32) -> bool {
    let vk = (vk & 0xFF) as usize;
    set[vk / 64] & (1 << (vk % 64)) != 0
}

fn put(set: &mut [u64; 4], vk: u32, on: bool) {
    let vk = (vk & 0xFF) as usize;
    if on {
        set[vk / 64] |= 1 << (vk % 64);
    } else {
        set[vk / 64] &= !(1 << (vk % 64));
    }
}

impl Swallow {
    /// Decides one hook event. Returns true to swallow it (the game does not see it).
    ///
    /// - First key-down: `ask` (the handler) decides, and that decides the key until it is released.
    /// - Auto-repeat: follows the first key-down. If the key is ours, `ask` is called again (with
    ///   `repeat` = true, so arrows can repeat) but its answer is ignored. If it is the game's, `ask` is
    ///   not called.
    /// - Key-up: swallowed exactly when the first key-down was. `ask` is not called.
    pub fn decide(&mut self, msg: u32, vk: u32, injected: bool, ask: impl FnOnce(KeyEvent) -> bool) -> bool {
        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            let key = decode(msg, vk).map(Key::Menu).unwrap_or(Key::Other(vk));
            if bit(&self.held, vk) {
                let ours = bit(&self.swallowed, vk);
                if ours {
                    ask(KeyEvent { key, repeat: true, injected });
                }
                return ours;
            }
            let swallow = ask(KeyEvent { key, repeat: false, injected });
            put(&mut self.held, vk, true);
            put(&mut self.swallowed, vk, swallow);
            swallow
        } else if msg == WM_KEYUP || msg == WM_SYSKEYUP {
            let swallow = bit(&self.swallowed, vk);
            put(&mut self.held, vk, false);
            put(&mut self.swallowed, vk, false);
            swallow
        } else {
            false
        }
    }
}

type Handler = Arc<dyn Fn(KeyEvent) -> bool + Send + Sync>;

/// The handler of the installed hook. Global (not thread-local): the hook thread calls it, and `feed`
/// calls it from the probe's thread.
static HANDLER: Mutex<Option<Handler>> = Mutex::new(None);
static SEEN: AtomicU32 = AtomicU32::new(0);
static INJECTED: AtomicU32 = AtomicU32::new(0);

thread_local! {
    // Only the hook thread uses it.
    static STATE: std::cell::RefCell<Swallow> = std::cell::RefCell::new(Swallow::default());
}

fn handler() -> Option<Handler> {
    HANDLER.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The installed hook and its thread. Dropping it removes the hook, ends the thread and clears the handler.
pub struct Hook {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Hook {
    fn drop(&mut self) {
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// Installs the hook on a new thread with its own message loop. `on_key` runs on that thread for every
/// key-down; it returns true to swallow the key. It must answer at once: read flags, post the work to the
/// UI thread with `slint::invoke_from_event_loop`, return.
pub fn install(on_key: impl Fn(KeyEvent) -> bool + Send + Sync + 'static) -> windows::core::Result<Hook> {
    *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(on_key));
    let (tx, rx) = std::sync::mpsc::channel::<windows::core::Result<u32>>();
    let thread = std::thread::Builder::new()
        .name("dr-key-hook".into())
        .spawn(move || unsafe {
            // Make the thread's message queue before anyone posts WM_QUIT to it.
            let mut msg = MSG::default();
            let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
            let hook = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) {
                Ok(h) => h,
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            };
            let _ = tx.send(Ok(GetCurrentThreadId()));
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                DispatchMessageW(&msg);
            }
            let _ = UnhookWindowsHookEx(hook);
        })
        .map_err(|e| windows::core::Error::new(windows::core::HRESULT(-1), e.to_string()))?;
    let thread_id = rx.recv().unwrap_or_else(|_| Err(windows::core::Error::from_thread()));
    match thread_id {
        Ok(thread_id) => Ok(Hook { thread_id, thread: Some(thread) }),
        Err(e) => {
            let _ = thread.join();
            *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = None;
            Err(e)
        }
    }
}

/// How many key events the hook has seen (any key, down or up).
pub fn seen() -> u32 {
    SEEN.load(Ordering::Relaxed)
}

/// How many of those were injected (`LLKHF_INJECTED`).
pub fn seen_injected() -> u32 {
    INJECTED.load(Ordering::Relaxed)
}

/// Runs the handler as the hook would for a first key-down. The probe uses it instead of `SendInput`,
/// which could reach the real desktop. It skips `hook_proc` and `Swallow`. Returns the handler's answer.
pub fn feed(key: Key) -> bool {
    handler().map(|f| f(KeyEvent::new(key))).unwrap_or(false)
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        SEEN.fetch_add(1, Ordering::Relaxed);
        let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let injected = kb.flags.0 & LLKHF_INJECTED.0 != 0;
        if injected {
            INJECTED.fetch_add(1, Ordering::Relaxed);
        }
        let msg = wparam.0 as u32;
        let swallow = match handler() {
            Some(f) => STATE.with(|s| {
                s.borrow_mut().decide(msg, kb.vkCode, injected, |ev| {
                    // A panic must not cross this extern "system" function (it would abort the process).
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(ev))).unwrap_or(false)
                })
            }),
            None => false,
        };
        if swallow {
            return LRESULT(1);
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ESC: u32 = VK_ESCAPE.0 as u32;
    const DOWN: u32 = VK_DOWN.0 as u32;

    #[test]
    fn decodes_menu_keys_on_key_down_only() {
        assert_eq!(decode(WM_KEYDOWN, DOWN), Some(MenuKey::Down));
        assert_eq!(decode(WM_SYSKEYDOWN, VK_RETURN.0 as u32), Some(MenuKey::Enter));
        assert_eq!(decode(WM_KEYDOWN, VK_BACK.0 as u32), Some(MenuKey::Delete));
        assert_eq!(decode(WM_KEYUP, DOWN), None);
        assert_eq!(decode(WM_KEYDOWN, 0x41), None); // A
    }

    #[test]
    fn esc_that_closes_the_menu_swallows_its_key_up() {
        let mut s = Swallow::default();
        let mut menu_open = true;
        // Esc down: the menu is open, so the handler swallows it and closes the menu.
        let down = s.decide(WM_KEYDOWN, ESC, false, |ev| {
            assert_eq!(ev.key, Key::Menu(MenuKey::Esc));
            let open = menu_open;
            menu_open = false;
            open
        });
        assert!(down);
        assert!(!menu_open);
        // Esc up: the menu is closed now, but the key-down was ours, so the game must not see the key-up.
        assert!(s.decide(WM_KEYUP, ESC, false, |_| panic!("key-ups never reach the handler")));
        // The next Esc (menu closed) goes to the game, down and up.
        assert!(!s.decide(WM_KEYDOWN, ESC, false, |_| false));
        assert!(!s.decide(WM_KEYUP, ESC, false, |_| unreachable!()));
    }

    #[test]
    fn key_held_before_the_menu_opened_stays_with_the_game() {
        let mut s = Swallow::default();
        // Down goes to the game (menu closed), then the menu opens while it is held.
        assert!(!s.decide(WM_KEYDOWN, DOWN, false, |_| false));
        // Its auto-repeats and key-up still go to the game; the handler is not asked.
        assert!(!s.decide(WM_KEYDOWN, DOWN, false, |_| panic!("the game's key")));
        assert!(!s.decide(WM_KEYUP, DOWN, false, |_| unreachable!()));
        // The next press is a new key-down: now the open menu gets it.
        assert!(s.decide(WM_KEYDOWN, DOWN, false, |ev| !ev.repeat));
        assert!(s.decide(WM_KEYUP, DOWN, false, |_| unreachable!()));
    }

    #[test]
    fn repeat_of_a_swallowed_key_stays_swallowed() {
        let mut s = Swallow::default();
        assert!(s.decide(WM_KEYDOWN, ESC, false, |_| true));
        // The menu closed on the first down; the handler is still told about the repeat, and its answer
        // ("not mine any more") does not give the key to the game halfway.
        let mut asked = false;
        assert!(s.decide(WM_KEYDOWN, ESC, false, |ev| {
            asked = ev.repeat;
            false
        }));
        assert!(asked);
        assert!(s.decide(WM_KEYUP, ESC, false, |_| unreachable!()));
    }

    #[test]
    fn other_keys_pass_and_injected_is_reported() {
        let mut s = Swallow::default();
        assert!(!s.decide(WM_SYSKEYDOWN, 0x41, true, |ev| {
            assert_eq!(ev.key, Key::Other(0x41));
            assert!(ev.injected);
            false
        }));
        assert!(!s.decide(WM_SYSKEYUP, 0x41, true, |_| unreachable!()));
    }
}
