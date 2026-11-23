//! The two global keys (spec 2.6): the read key and the quick menu key, through low-level hooks.
//!
//! - [`Key`] is one key as the profile saves it: `"Mouse 4"`, `"F7"`, `"Ctrl+Shift+R"`. Mouse buttons 3 to 5 are
//!   the middle button and the two side buttons; they take no modifiers. Left and right click are never a key (a
//!   game needs them).
//! - [`decode_key`] and [`decode_mouse`] turn a hook message into a [`Raw`] event; [`State::feed`] decides what it
//!   means and whether the game sees it. Both are pure and unit-tested without real input.
//! - [`Hooks`] installs `WH_KEYBOARD_LL` and `WH_MOUSE_LL` on their own thread with its own message loop
//!   (never on the UI thread: Windows waits for a low-level hook on every key and drops a slow one;
//!   docs/overlay-spike.md). The callback returns at once: mouse moves and the wheel leave before any lock, and
//!   a button or key only takes a short lock and hands its action on.
//! - The read key and the quick menu key go through to the game. Only key capture ("Change" on the keys page and
//!   in Settings) swallows the key it takes, down and up, so the app page does not react to it too. A key belongs
//!   to whoever got its first key-down until it is released (the spike's rule).
//! - Injected input (`LLKHF_INJECTED`, pad mappers) counts as real input, as in the spike.

use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Ctrl, Shift, Alt and Win.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
}

/// One key or mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// 3: middle, 4: back (X1), 5: forward (X2).
    Mouse(u8),
    /// A keyboard key by virtual key code, with the modifiers held with it.
    Keyboard { vk: u16, mods: Mods },
}

/// A mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Btn {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

impl Btn {
    /// The button's number as a key (3 to 5); left and right have none.
    pub fn number(self) -> Option<u8> {
        match self {
            Btn::Middle => Some(3),
            Btn::X1 => Some(4),
            Btn::X2 => Some(5),
            Btn::Left | Btn::Right => None,
        }
    }

    fn index(self) -> usize {
        match self {
            Btn::Left => 0,
            Btn::Right => 1,
            Btn::Middle => 2,
            Btn::X1 => 3,
            Btn::X2 => 4,
        }
    }
}

/// Key names by virtual key code. Letters, digits and F1 to F24 are made in [`vk_name`].
const NAMES: &[(u16, &str)] = &[
    (0x08, "Backspace"),
    (0x09, "Tab"),
    (0x0D, "Enter"),
    (0x13, "Pause"),
    (0x14, "Caps Lock"),
    (0x1B, "Esc"),
    (0x20, "Space"),
    (0x21, "Page Up"),
    (0x22, "Page Down"),
    (0x23, "End"),
    (0x24, "Home"),
    (0x25, "Left"),
    (0x26, "Up"),
    (0x27, "Right"),
    (0x28, "Down"),
    (0x2C, "Print Screen"),
    (0x2D, "Insert"),
    (0x2E, "Delete"),
    (0x5D, "Menu"),
    (0x6A, "Num *"),
    (0x6B, "Num +"),
    (0x6D, "Num -"),
    (0x6E, "Num ."),
    (0x6F, "Num /"),
    (0x90, "Num Lock"),
    (0x91, "Scroll Lock"),
    (0xBA, ";"),
    (0xBB, "="),
    (0xBC, ","),
    (0xBD, "-"),
    (0xBE, "."),
    (0xBF, "/"),
    (0xC0, "`"),
    (0xDB, "["),
    (0xDC, "\\"),
    (0xDD, "]"),
    (0xDE, "'"),
];

/// Esc: cancels key capture.
pub const VK_ESCAPE: u16 = 0x1B;

/// The name of a virtual key ("F7", "R", "Num 5", "Page Up"); "Key 123" for one without a name.
pub fn vk_name(vk: u16) -> String {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => char::from(vk as u8).to_string(),
        0x60..=0x69 => format!("Num {}", vk - 0x60),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        _ => NAMES
            .iter()
            .find(|(v, _)| *v == vk)
            .map_or_else(|| format!("Key {vk}"), |(_, n)| n.to_string()),
    }
}

/// The virtual key of a name (case does not matter): the reverse of [`vk_name`].
pub fn vk_of(name: &str) -> Option<u16> {
    let n = name.trim();
    let up = n.to_ascii_uppercase();
    if up.len() == 1 {
        let c = up.as_bytes()[0];
        if c.is_ascii_digit() || c.is_ascii_uppercase() {
            return Some(c as u16);
        }
    }
    if let Some(f) = up.strip_prefix('F').and_then(|d| d.parse::<u16>().ok())
        && (1..=24).contains(&f)
    {
        return Some(0x6F + f);
    }
    if let Some(d) = up.strip_prefix("NUM ").and_then(|d| d.parse::<u16>().ok())
        && d <= 9
    {
        return Some(0x60 + d);
    }
    if let Some(v) = up.strip_prefix("KEY ").and_then(|d| d.parse::<u16>().ok()) {
        return Some(v);
    }
    NAMES
        .iter()
        .find(|(_, name)| name.eq_ignore_ascii_case(n))
        .map(|(v, _)| *v)
}

/// The modifier a virtual key is, if it is one (left, right or either).
fn modifier(vk: u16) -> Option<fn(&mut Mods) -> &mut bool> {
    match vk {
        0x10 | 0xA0 | 0xA1 => Some(|m| &mut m.shift),
        0x11 | 0xA2 | 0xA3 => Some(|m| &mut m.ctrl),
        0x12 | 0xA4 | 0xA5 => Some(|m| &mut m.alt),
        0x5B | 0x5C => Some(|m| &mut m.win),
        _ => None,
    }
}

/// The key is Ctrl, Shift, Alt or Win (alone they are never a key).
pub fn is_modifier(vk: u16) -> bool {
    modifier(vk).is_some()
}

impl Key {
    /// The saved form: "Mouse 4", "F7", "Ctrl+Shift+R".
    pub fn text(&self) -> String {
        match *self {
            Key::Mouse(n) => format!("Mouse {n}"),
            Key::Keyboard { vk, mods } => {
                let mut s = String::new();
                for (on, name) in [
                    (mods.ctrl, "Ctrl+"),
                    (mods.shift, "Shift+"),
                    (mods.alt, "Alt+"),
                    (mods.win, "Win+"),
                ] {
                    if on {
                        s.push_str(name);
                    }
                }
                s + &vk_name(vk)
            }
        }
    }

    /// Reads the saved form (case and spaces around `+` do not matter). None for left or right click, a modifier
    /// alone, a mouse button with modifiers, or a name that is not known.
    pub fn parse(text: &str) -> Option<Key> {
        let mut rest = text.trim();
        let mut mods = Mods::default();
        loop {
            let lower = rest.to_ascii_lowercase();
            let mut found = false;
            for (p, f) in [
                ("ctrl", (|m| &mut m.ctrl) as fn(&mut Mods) -> &mut bool),
                ("shift", |m| &mut m.shift),
                ("alt", |m| &mut m.alt),
                ("win", |m| &mut m.win),
            ] {
                if let Some(after) = lower.strip_prefix(p)
                    && let Some(after) = after.trim_start().strip_prefix('+')
                {
                    *f(&mut mods) = true;
                    rest = rest[rest.len() - after.len()..].trim_start();
                    found = true;
                }
            }
            if !found {
                break;
            }
        }
        if let Some(n) = rest
            .to_ascii_lowercase()
            .strip_prefix("mouse")
            .and_then(|d| d.trim().parse::<u8>().ok())
        {
            return ((3..=5).contains(&n) && mods == Mods::default()).then_some(Key::Mouse(n));
        }
        let vk = vk_of(rest)?;
        (!is_modifier(vk)).then_some(Key::Keyboard { vk, mods })
    }
}

/// One hook event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Raw {
    /// A key went down (also an auto-repeat).
    KeyDown(u16),
    KeyUp(u16),
    Button {
        b: Btn,
        down: bool,
    },
}

const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MBUTTONDOWN: u32 = 0x0207;
const WM_MBUTTONUP: u32 = 0x0208;
const WM_XBUTTONDOWN: u32 = 0x020B;
const WM_XBUTTONUP: u32 = 0x020C;
/// `HIWORD(mouseData)` of an X button message.
const XBUTTON1: u32 = 1;
const XBUTTON2: u32 = 2;

/// A keyboard hook message (`wParam`) and its virtual key.
pub fn decode_key(msg: u32, vk: u32) -> Option<Raw> {
    let vk = u16::try_from(vk).ok()?;
    match msg {
        WM_KEYDOWN | WM_SYSKEYDOWN => Some(Raw::KeyDown(vk)),
        WM_KEYUP | WM_SYSKEYUP => Some(Raw::KeyUp(vk)),
        _ => None,
    }
}

/// A mouse hook message (`wParam`) and its `mouseData`. Moves, the wheel and anything else give None.
pub fn decode_mouse(msg: u32, mouse_data: u32) -> Option<Raw> {
    let (b, down) = match msg {
        WM_LBUTTONDOWN => (Btn::Left, true),
        WM_LBUTTONUP => (Btn::Left, false),
        WM_RBUTTONDOWN => (Btn::Right, true),
        WM_RBUTTONUP => (Btn::Right, false),
        WM_MBUTTONDOWN => (Btn::Middle, true),
        WM_MBUTTONUP => (Btn::Middle, false),
        WM_XBUTTONDOWN | WM_XBUTTONUP => {
            let b = match mouse_data >> 16 {
                XBUTTON1 => Btn::X1,
                XBUTTON2 => Btn::X2,
                _ => return None,
            };
            (b, msg == WM_XBUTTONDOWN)
        }
        _ => return None,
    };
    Some(Raw::Button { b, down })
}

/// The keys the hook looks for now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bindings {
    pub read: Option<Key>,
    pub menu: Option<Key>,
    /// Key capture is on: the next key or button (not left or right click) is taken.
    pub capture: bool,
}

/// What an event means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    /// The read key went down.
    Read,
    /// The quick menu key went down.
    Menu,
    /// Key capture took this key.
    Captured(Key),
    /// Key capture saw a button that cannot be a key (left or right click).
    Refused(Btn),
    /// Esc during key capture.
    Cancelled,
}

/// Which keys and buttons are down, which of those were swallowed, and the modifiers held. Pure.
#[derive(Default)]
pub struct State {
    mods: Mods,
    held: [u64; 4],
    swallowed: [u64; 4],
    btn_held: [bool; 5],
    btn_swallowed: [bool; 5],
}

fn bit(set: &[u64; 4], vk: u16) -> bool {
    let vk = (vk & 0xFF) as usize;
    set[vk / 64] & (1 << (vk % 64)) != 0
}

fn put(set: &mut [u64; 4], vk: u16, on: bool) {
    let vk = (vk & 0xFF) as usize;
    if on {
        set[vk / 64] |= 1 << (vk % 64);
    } else {
        set[vk / 64] &= !(1 << (vk % 64));
    }
}

impl State {
    /// The modifiers held now, from Windows (the hook may miss a key-up: Win+L, a UAC prompt, an elevated
    /// window in front). The next event then judges with them.
    pub fn set_mods(&mut self, mods: Mods) {
        self.mods = mods;
    }

    /// Decides one event: what it means, and whether to swallow it (the game and other apps do not see it).
    pub fn feed(&mut self, raw: Raw, b: &Bindings) -> (Action, bool) {
        match raw {
            Raw::KeyDown(vk) => {
                if let Some(m) = modifier(vk) {
                    *m(&mut self.mods) = true;
                }
                if bit(&self.held, vk) {
                    // auto-repeat: as its first key-down
                    return (Action::None, bit(&self.swallowed, vk));
                }
                let (action, swallow) = if is_modifier(vk) {
                    (Action::None, false)
                } else if b.capture {
                    if vk == VK_ESCAPE {
                        (Action::Cancelled, true)
                    } else {
                        (
                            Action::Captured(Key::Keyboard {
                                vk,
                                mods: self.mods,
                            }),
                            true,
                        )
                    }
                } else {
                    let k = Key::Keyboard {
                        vk,
                        mods: self.mods,
                    };
                    (Self::bound(b, k), false)
                };
                put(&mut self.held, vk, true);
                put(&mut self.swallowed, vk, swallow);
                (action, swallow)
            }
            Raw::KeyUp(vk) => {
                if let Some(m) = modifier(vk) {
                    *m(&mut self.mods) = false;
                }
                let swallow = bit(&self.swallowed, vk);
                put(&mut self.held, vk, false);
                put(&mut self.swallowed, vk, false);
                (Action::None, swallow)
            }
            Raw::Button { b: btn, down: true } => {
                let i = btn.index();
                if self.btn_held[i] {
                    return (Action::None, self.btn_swallowed[i]);
                }
                let (action, swallow) = match (b.capture, btn.number()) {
                    // a game needs left and right click: they are never taken
                    (true, None) => (Action::Refused(btn), false),
                    (true, Some(n)) => (Action::Captured(Key::Mouse(n)), true),
                    (false, Some(n)) => (Self::bound(b, Key::Mouse(n)), false),
                    (false, None) => (Action::None, false),
                };
                self.btn_held[i] = true;
                self.btn_swallowed[i] = swallow;
                (action, swallow)
            }
            Raw::Button {
                b: btn,
                down: false,
            } => {
                let i = btn.index();
                let swallow = self.btn_swallowed[i];
                self.btn_held[i] = false;
                self.btn_swallowed[i] = false;
                (Action::None, swallow)
            }
        }
    }

    fn bound(b: &Bindings, k: Key) -> Action {
        if b.read == Some(k) {
            Action::Read
        } else if b.menu == Some(k) {
            Action::Menu
        } else {
            Action::None
        }
    }
}

/// Runs on the hook thread for every action other than `None`. Must not wait.
pub type OnAction = Arc<dyn Fn(Action) + Send + Sync>;

/// What the hook callbacks share: the bindings, the state and where actions go.
struct Shared {
    bindings: Arc<Mutex<Bindings>>,
    state: Mutex<State>,
    on_action: OnAction,
}

/// The installed hooks. Only one set per process; a newer [`Hooks::install`] takes over.
static HOOKED: Mutex<Option<Arc<Shared>>> = Mutex::new(None);

fn shared() -> Option<Arc<Shared>> {
    HOOKED.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Runs one event through the installed hooks' state. True: swallow it.
fn handle(raw: Raw) -> bool {
    let Some(s) = shared() else { return false };
    let b = s.bindings.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let (action, swallow) = {
        let mut st = s.state.lock().unwrap_or_else(|e| e.into_inner());
        #[cfg(windows)]
        st.set_mods(win::mods_now());
        st.feed(raw, &b)
    };
    if action != Action::None {
        if matches!(action, Action::Captured(_) | Action::Cancelled) {
            // the next key is the game's again
            s.bindings.lock().unwrap_or_else(|e| e.into_inner()).capture = false;
        }
        // a panic must not cross the hook's extern "system" function
        let f = s.on_action.clone();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(action)));
    }
    swallow
}

/// The keyboard and mouse hooks on their thread. Dropping it removes them and ends the thread.
pub struct Hooks {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

#[cfg(windows)]
mod win {
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, HC_ACTION, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT, WM_MOUSEHWHEEL, WM_MOUSEMOVE,
        WM_MOUSEWHEEL,
    };

    /// The modifiers held now (a key-down of a modifier itself is added by `State::feed`).
    pub fn mods_now() -> super::Mods {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
        };
        let down = |vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY| unsafe {
            GetAsyncKeyState(vk.0 as i32) < 0
        };
        super::Mods {
            ctrl: down(VK_CONTROL),
            shift: down(VK_SHIFT),
            alt: down(VK_MENU),
            win: down(VK_LWIN) || down(VK_RWIN),
        }
    }

    pub unsafe extern "system" fn keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            if let Some(raw) = super::decode_key(wparam.0 as u32, kb.vkCode)
                && super::handle(raw)
            {
                return LRESULT(1);
            }
        }
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    pub unsafe extern "system" fn mouse(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let msg = wparam.0 as u32;
        // moves and the wheel leave at once: no lock, no work
        if code == HC_ACTION as i32
            && msg != WM_MOUSEMOVE
            && msg != WM_MOUSEWHEEL
            && msg != WM_MOUSEHWHEEL
        {
            let m = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
            if let Some(raw) = super::decode_mouse(msg, m.mouseData)
                && super::handle(raw)
            {
                return LRESULT(1);
            }
        }
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }
}

impl Hooks {
    /// Installs both hooks on a new thread. `bindings` can change while they run (the next event uses the new
    /// ones). `on_action` runs on the hook thread and must return at once.
    #[cfg(windows)]
    pub fn install(bindings: Arc<Mutex<Bindings>>, on_action: OnAction) -> Result<Hooks, String> {
        use windows::Win32::System::Threading::GetCurrentThreadId;
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, SetWindowsHookExW,
            UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL,
        };
        *HOOKED.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(Shared {
            bindings,
            state: Mutex::new(State::default()),
            on_action,
        }));
        let (tx, rx) = std::sync::mpsc::channel::<Result<u32, String>>();
        let thread = std::thread::Builder::new()
            .name("key-hooks".into())
            .spawn(move || unsafe {
                // the thread's message queue exists before anyone posts WM_QUIT to it
                let mut msg = MSG::default();
                let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
                let kb = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(win::keyboard), None, 0) {
                    Ok(h) => h,
                    Err(e) => {
                        let _ = tx.send(Err(format!("keyboard hook: {e}")));
                        return;
                    }
                };
                let mouse = match SetWindowsHookExW(WH_MOUSE_LL, Some(win::mouse), None, 0) {
                    Ok(h) => h,
                    Err(e) => {
                        let _ = UnhookWindowsHookEx(kb);
                        let _ = tx.send(Err(format!("mouse hook: {e}")));
                        return;
                    }
                };
                let _ = tx.send(Ok(GetCurrentThreadId()));
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    DispatchMessageW(&msg);
                }
                let _ = UnhookWindowsHookEx(mouse);
                let _ = UnhookWindowsHookEx(kb);
            })
            .map_err(|e| format!("the key thread could not start: {e}"))?;
        match rx.recv() {
            Ok(Ok(thread_id)) => Ok(Hooks {
                thread_id,
                thread: Some(thread),
            }),
            Ok(Err(e)) => {
                let _ = thread.join();
                *HOOKED.lock().unwrap_or_else(|e| e.into_inner()) = None;
                Err(e)
            }
            Err(_) => {
                let _ = thread.join();
                *HOOKED.lock().unwrap_or_else(|e| e.into_inner()) = None;
                Err("the key thread ended".into())
            }
        }
    }
}

impl Drop for Hooks {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::Foundation::{LPARAM, WPARAM};
            use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        *HOOKED.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kb(vk: u16) -> Key {
        Key::Keyboard {
            vk,
            mods: Mods::default(),
        }
    }

    #[test]
    fn decode_table() {
        // keyboard: down and up, the system variants (Alt held, F10) too
        assert_eq!(decode_key(WM_KEYDOWN, 0x76), Some(Raw::KeyDown(0x76)));
        assert_eq!(decode_key(WM_SYSKEYDOWN, 0x52), Some(Raw::KeyDown(0x52)));
        assert_eq!(decode_key(WM_KEYUP, 0x76), Some(Raw::KeyUp(0x76)));
        assert_eq!(decode_key(WM_SYSKEYUP, 0x12), Some(Raw::KeyUp(0x12)));
        assert_eq!(decode_key(0x0102, 0x41), None, "WM_CHAR is no hook message");
        // mouse: every button, both X buttons by their high word, moves and the wheel are nothing
        let btn = |b, down| Some(Raw::Button { b, down });
        assert_eq!(decode_mouse(WM_LBUTTONDOWN, 0), btn(Btn::Left, true));
        assert_eq!(decode_mouse(WM_LBUTTONUP, 0), btn(Btn::Left, false));
        assert_eq!(decode_mouse(WM_RBUTTONDOWN, 0), btn(Btn::Right, true));
        assert_eq!(decode_mouse(WM_RBUTTONUP, 0), btn(Btn::Right, false));
        assert_eq!(decode_mouse(WM_MBUTTONDOWN, 0), btn(Btn::Middle, true));
        assert_eq!(decode_mouse(WM_MBUTTONUP, 0), btn(Btn::Middle, false));
        assert_eq!(decode_mouse(WM_XBUTTONDOWN, 1 << 16), btn(Btn::X1, true));
        assert_eq!(decode_mouse(WM_XBUTTONUP, 1 << 16), btn(Btn::X1, false));
        assert_eq!(decode_mouse(WM_XBUTTONDOWN, 2 << 16), btn(Btn::X2, true));
        assert_eq!(decode_mouse(WM_XBUTTONDOWN, 3 << 16), None);
        assert_eq!(decode_mouse(0x0200, 0), None, "move");
        assert_eq!(decode_mouse(0x020A, 120 << 16), None, "wheel");
        assert_eq!(Btn::X1.number(), Some(4));
        assert_eq!(Btn::X2.number(), Some(5));
        assert_eq!(Btn::Middle.number(), Some(3));
        assert_eq!(Btn::Left.number(), None);
    }

    #[test]
    fn names_and_text_both_ways() {
        let cases = [
            ("Mouse 4", Key::Mouse(4)),
            ("Mouse 5", Key::Mouse(5)),
            ("Mouse 3", Key::Mouse(3)),
            ("F7", kb(0x76)),
            ("F24", kb(0x87)),
            ("R", kb(0x52)),
            ("5", kb(0x35)),
            ("Num 5", kb(0x65)),
            ("Num +", kb(0x6B)),
            ("Page Up", kb(0x21)),
            ("Space", kb(0x20)),
            ("`", kb(0xC0)),
            ("Key 255", kb(0xFF)),
            (
                "Ctrl+Shift+R",
                Key::Keyboard {
                    vk: 0x52,
                    mods: Mods {
                        ctrl: true,
                        shift: true,
                        ..Mods::default()
                    },
                },
            ),
            (
                "Alt+Num +",
                Key::Keyboard {
                    vk: 0x6B,
                    mods: Mods {
                        alt: true,
                        ..Mods::default()
                    },
                },
            ),
        ];
        for (text, key) in cases {
            assert_eq!(Key::parse(text), Some(key), "{text}");
            assert_eq!(key.text(), text);
        }
        // case and spaces do not matter
        assert_eq!(Key::parse(" ctrl + shift + r "), Key::parse("Ctrl+Shift+R"));
        assert_eq!(Key::parse("mouse4"), Some(Key::Mouse(4)));
        // never a key: left and right click, a modifier alone, a mouse button with modifiers, nonsense
        for bad in [
            "Mouse 1",
            "Mouse 2",
            "Mouse 6",
            "Ctrl",
            "Shift+",
            "Ctrl+Mouse 4",
            "Hyper",
            "",
        ] {
            assert_eq!(Key::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn read_and_menu_keys_go_through_to_the_game() {
        let b = Bindings {
            read: Some(Key::Mouse(4)),
            menu: Key::parse("F7"),
            capture: false,
        };
        let mut s = State::default();
        let x1 = |down| Raw::Button { b: Btn::X1, down };
        assert_eq!(s.feed(x1(true), &b), (Action::Read, false));
        assert_eq!(s.feed(x1(false), &b), (Action::None, false));
        assert_eq!(s.feed(Raw::KeyDown(0x76), &b), (Action::Menu, false));
        // auto-repeat does not fire again
        assert_eq!(s.feed(Raw::KeyDown(0x76), &b), (Action::None, false));
        assert_eq!(s.feed(Raw::KeyUp(0x76), &b), (Action::None, false));
        // other keys and buttons: nothing
        assert_eq!(s.feed(Raw::KeyDown(0x41), &b), (Action::None, false));
        let left = Raw::Button {
            b: Btn::Left,
            down: true,
        };
        assert_eq!(s.feed(left, &b), (Action::None, false));
    }

    #[test]
    fn modifiers_must_match_exactly() {
        let b = Bindings {
            read: Key::parse("Ctrl+R"),
            ..Bindings::default()
        };
        let mut s = State::default();
        // R alone is not Ctrl+R
        assert_eq!(s.feed(Raw::KeyDown(0x52), &b).0, Action::None);
        s.feed(Raw::KeyUp(0x52), &b);
        // left Ctrl then R
        s.feed(Raw::KeyDown(0xA2), &b);
        assert_eq!(s.feed(Raw::KeyDown(0x52), &b).0, Action::Read);
        s.feed(Raw::KeyUp(0x52), &b);
        // Ctrl+Shift+R is not Ctrl+R
        s.feed(Raw::KeyDown(0xA0), &b);
        assert_eq!(s.feed(Raw::KeyDown(0x52), &b).0, Action::None);
        s.feed(Raw::KeyUp(0x52), &b);
        s.feed(Raw::KeyUp(0xA0), &b);
        s.feed(Raw::KeyUp(0xA2), &b);
        assert_eq!(s.feed(Raw::KeyDown(0x52), &b).0, Action::None);
    }

    #[test]
    fn a_lost_key_up_of_a_modifier_is_fixed_by_the_state_from_windows() {
        let b = Bindings {
            read: Key::parse("R"),
            ..Bindings::default()
        };
        let mut s = State::default();
        // Ctrl went down, its key-up never came (Win+L, a UAC prompt)
        s.feed(Raw::KeyDown(0xA2), &b);
        assert_eq!(s.feed(Raw::KeyDown(0x52), &b).0, Action::None);
        s.feed(Raw::KeyUp(0x52), &b);
        // the hook reads the modifiers from Windows before each event
        s.set_mods(Mods::default());
        assert_eq!(s.feed(Raw::KeyDown(0x52), &b).0, Action::Read);
    }

    #[test]
    fn capture_takes_the_next_key_and_swallows_it() {
        let b = Bindings {
            capture: true,
            ..Bindings::default()
        };
        let mut s = State::default();
        // a modifier alone is not taken, and goes on to the app
        assert_eq!(s.feed(Raw::KeyDown(0xA2), &b), (Action::None, false));
        // Ctrl+F8 is taken; its key-up is swallowed too, the Ctrl key-up is not
        let want = Key::Keyboard {
            vk: 0x77,
            mods: Mods {
                ctrl: true,
                ..Mods::default()
            },
        };
        assert_eq!(
            s.feed(Raw::KeyDown(0x77), &b),
            (Action::Captured(want), true)
        );
        assert_eq!(want.text(), "Ctrl+F8");
        let off = Bindings::default();
        assert_eq!(
            s.feed(Raw::KeyDown(0x77), &off),
            (Action::None, true),
            "repeat"
        );
        assert_eq!(s.feed(Raw::KeyUp(0x77), &off), (Action::None, true));
        assert_eq!(s.feed(Raw::KeyUp(0xA2), &off), (Action::None, false));
        // Esc cancels, swallowed down and up
        assert_eq!(
            s.feed(Raw::KeyDown(VK_ESCAPE), &b),
            (Action::Cancelled, true)
        );
        assert_eq!(s.feed(Raw::KeyUp(VK_ESCAPE), &off), (Action::None, true));
    }

    #[test]
    fn capture_refuses_left_and_right_click_and_takes_side_buttons() {
        let b = Bindings {
            capture: true,
            ..Bindings::default()
        };
        let mut s = State::default();
        for btn in [Btn::Left, Btn::Right] {
            // refused, and the click still reaches the app (it may be on the Cancel button)
            assert_eq!(
                s.feed(Raw::Button { b: btn, down: true }, &b),
                (Action::Refused(btn), false)
            );
            assert_eq!(
                s.feed(
                    Raw::Button {
                        b: btn,
                        down: false
                    },
                    &b
                ),
                (Action::None, false)
            );
        }
        assert_eq!(
            s.feed(
                Raw::Button {
                    b: Btn::X2,
                    down: true
                },
                &b
            ),
            (Action::Captured(Key::Mouse(5)), true)
        );
        assert_eq!(
            s.feed(
                Raw::Button {
                    b: Btn::X2,
                    down: false
                },
                &Bindings::default()
            ),
            (Action::None, true)
        );
        assert_eq!(
            s.feed(
                Raw::Button {
                    b: Btn::Middle,
                    down: true
                },
                &b
            ),
            (Action::Captured(Key::Mouse(3)), true)
        );
    }

    #[test]
    fn a_key_held_before_capture_stays_with_the_app() {
        let mut s = State::default();
        let off = Bindings::default();
        let on = Bindings {
            capture: true,
            ..Bindings::default()
        };
        assert_eq!(s.feed(Raw::KeyDown(0x41), &off), (Action::None, false));
        // capture starts while A is held: its repeats and key-up are not taken
        assert_eq!(s.feed(Raw::KeyDown(0x41), &on), (Action::None, false));
        assert_eq!(s.feed(Raw::KeyUp(0x41), &on), (Action::None, false));
        // the next press of A is
        assert_eq!(
            s.feed(Raw::KeyDown(0x41), &on),
            (Action::Captured(kb(0x41)), true)
        );
    }
}
