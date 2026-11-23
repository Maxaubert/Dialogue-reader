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
//! - Matching (Max, 2026-11-23): a key saved with modifiers (Ctrl+F7) needs exactly those modifiers. A key saved
//!   without Ctrl, Shift or Alt fires while any of them is held (Mouse 4 or F7 while sprinting with Shift). When both
//!   keys could match, the exact one wins. Not part of that decision, kept as before: a keyboard key does not fire
//!   while Win is held (Win+key is a Windows shortcut).
//! - Key capture never takes Alt+Tab, Alt+Esc, Ctrl+Esc, Win or Win+anything: they go on to Windows, and capture
//!   keeps waiting. The app ends capture when its window loses the focus (`dr_app::focus_lost`).
//! - In-game overlays (M4, spec 2.7): the quick menu key opens the quick menu only while the game window is in
//!   front, and goes through to the game then. While the menu is open the hook takes the arrows, Enter, Space,
//!   Esc and the quick menu key itself (it closes the menu); while zone edit is open it takes Delete, Backspace,
//!   Esc and the quick menu key (back to the menu). Taken keys come out as [`Action::Nav`]. The open overlay
//!   ([`Overlay`]) changes on the hook thread at once (the quick menu key, Esc), so the next key is already judged
//!   by it; the app reads it back and makes its windows match. Only arrows and Delete repeat while held; a held
//!   Enter, Space or Esc acts once.

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
/// Tab (Alt+Tab is never taken by key capture).
const VK_TAB: u16 = 0x09;

/// A Windows shortcut that key capture never takes: Win+anything, Alt+Tab, Alt+Esc, Ctrl+Esc.
fn system_combo(vk: u16, mods: Mods) -> bool {
    mods.win || (mods.alt && (vk == VK_TAB || vk == VK_ESCAPE)) || (mods.ctrl && vk == VK_ESCAPE)
}

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

/// The in-game overlay that is open: it decides which keys DialogReader takes from the game.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overlay {
    #[default]
    None,
    /// The quick menu: arrows, Enter, Space, Esc.
    Menu,
    /// Zone edit: Delete, Backspace, Esc.
    Zones,
}

impl Overlay {
    /// The overlay after the quick menu key: closed opens the menu, the menu closes, zone edit goes back to the
    /// menu.
    pub fn after_menu_key(self) -> Overlay {
        match self {
            Overlay::None | Overlay::Zones => Overlay::Menu,
            Overlay::Menu => Overlay::None,
        }
    }
}

/// A key the quick menu or zone edit takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Space,
    Esc,
    /// Delete or Backspace.
    Delete,
}

impl Nav {
    /// The key this overlay takes for `vk`, if any.
    pub fn of(vk: u16, o: Overlay) -> Option<Nav> {
        let n = match vk {
            0x26 => Nav::Up,
            0x28 => Nav::Down,
            0x25 => Nav::Left,
            0x27 => Nav::Right,
            0x0D => Nav::Enter,
            0x20 => Nav::Space,
            0x1B => Nav::Esc,
            0x2E | 0x08 => Nav::Delete,
            _ => return None,
        };
        let taken = match o {
            Overlay::None => false,
            Overlay::Menu => n != Nav::Delete,
            Overlay::Zones => matches!(n, Nav::Delete | Nav::Esc),
        };
        taken.then_some(n)
    }

    /// Held down, the key acts again (arrows move on, Delete removes the next zone); Enter, Space and Esc act once.
    fn repeats(self) -> bool {
        matches!(
            self,
            Nav::Up | Nav::Down | Nav::Left | Nav::Right | Nav::Delete
        )
    }
}

/// The keys the hook looks for now.
#[derive(Clone, Debug, PartialEq)]
pub struct Bindings {
    pub read: Option<Key>,
    pub menu: Option<Key>,
    /// Key capture is on: the next key or button (not left or right click) is taken.
    pub capture: bool,
    /// The overlay that is open.
    pub overlay: Overlay,
    /// Esc closes the open overlay (false while the quick menu's voice list or a new zone's kind picker is open:
    /// Esc goes back there).
    pub esc_closes: bool,
    /// The game window (`HWND` as a number): the quick menu opens only while it is in front.
    pub game: Option<isize>,
    /// The game window is not in front now (the hook sets it from the foreground window before a key-down): the
    /// quick menu key does not open the menu, and an open overlay takes no keys.
    pub game_away: bool,
}

impl Bindings {
    /// What an action changes at once, on the hook thread, so the next event is judged by it: capture ends with
    /// the key it took (the next key is the game's again), the quick menu key opens or closes the menu, and Esc
    /// closes the open overlay (when it is not going back to the menu).
    pub fn after(&mut self, action: Action) {
        match action {
            Action::Captured(_) | Action::Cancelled => self.capture = false,
            Action::Menu => self.overlay = self.overlay.after_menu_key(),
            Action::Nav(Nav::Esc) if self.esc_closes => self.overlay = Overlay::None,
            _ => {}
        }
    }
}

impl Default for Bindings {
    fn default() -> Self {
        Bindings {
            read: None,
            menu: None,
            capture: false,
            overlay: Overlay::None,
            esc_closes: true,
            game: None,
            game_away: false,
        }
    }
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
    /// A key the open quick menu or zone edit takes (also an auto-repeat of an arrow or Delete).
    Nav(Nav),
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
                    // auto-repeat: as its first key-down; a held arrow or Delete keeps acting in an overlay
                    let swallowed = bit(&self.swallowed, vk);
                    let again = self
                        .nav(vk, b)
                        .filter(|n| swallowed && !b.capture && n.repeats())
                        .map_or(Action::None, Action::Nav);
                    return (again, swallowed);
                }
                let (action, swallow) = if is_modifier(vk) {
                    (Action::None, false)
                } else if b.capture {
                    if system_combo(vk, self.mods) {
                        // Windows gets it (the task switcher, Start); the app ends capture when it loses the focus
                        (Action::None, false)
                    } else if vk == VK_ESCAPE {
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
                    let bound = Self::bound(b, k);
                    match Self::overlay_key(b, bound) {
                        Some(r) => r,
                        None => match self.nav(vk, b) {
                            Some(n) => (Action::Nav(n), true),
                            None => (bound, false),
                        },
                    }
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
                    // Win+anything is never taken
                    (true, _) if self.mods.win => (Action::None, false),
                    // a game needs left and right click: they are never taken
                    (true, None) => (Action::Refused(btn), false),
                    (true, Some(n)) => (Action::Captured(Key::Mouse(n)), true),
                    (false, Some(n)) => {
                        let bound = Self::bound(b, Key::Mouse(n));
                        Self::overlay_key(b, bound).unwrap_or((bound, false))
                    }
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

    /// The key the open overlay takes for `vk`, if any. Never with Win, Ctrl or Alt held (Ctrl+Esc, Alt+Esc,
    /// Ctrl+Shift+Esc, Win+arrows and Alt+Enter stay Windows' and the game's), and never while the game is not in
    /// front (the UI closes the overlay on its next tick; until then the keys go where the user is).
    fn nav(&self, vk: u16, b: &Bindings) -> Option<Nav> {
        if self.mods.win || self.mods.ctrl || self.mods.alt || b.game_away {
            return None;
        }
        Nav::of(vk, b.overlay)
    }

    /// What the read key and the quick menu key do with the overlays (None: nothing special). The quick menu key
    /// opens the menu only while the game is in front (and goes through to the game then); while an overlay is
    /// open it is taken (it closes the menu, or goes from zone edit back to the menu). The read key does nothing
    /// while an overlay is open (reading waits then) and goes through to the game.
    fn overlay_key(b: &Bindings, bound: Action) -> Option<(Action, bool)> {
        match (bound, b.overlay) {
            (Action::Menu, Overlay::None) if b.game_away => Some((Action::None, false)),
            (Action::Menu, Overlay::None) => None,
            (Action::Menu, _) => Some((Action::Menu, true)),
            (Action::Read, Overlay::Menu | Overlay::Zones) => Some((Action::None, false)),
            _ => None,
        }
    }

    /// The binding a key press fires. An exact match first (the more specific key wins), then, for a keyboard
    /// key pressed with Ctrl, Shift or Alt but not Win, the key saved without modifiers. Mouse buttons take no
    /// modifiers, so they fire whatever is held.
    fn bound(b: &Bindings, k: Key) -> Action {
        let hit = |k: Key| {
            if b.read == Some(k) {
                Action::Read
            } else if b.menu == Some(k) {
                Action::Menu
            } else {
                Action::None
            }
        };
        match (hit(k), k) {
            (Action::None, Key::Keyboard { vk, mods }) if mods != Mods::default() && !mods.win => {
                hit(Key::Keyboard {
                    vk,
                    mods: Mods::default(),
                })
            }
            (a, _) => a,
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
    #[allow(unused_mut)]
    let mut b = s.bindings.lock().unwrap_or_else(|e| e.into_inner()).clone();
    // the quick menu opens only over the game in front, and an open overlay takes keys only while it is in front
    // (after Alt+Tab or a click on another monitor the keys go there at once, not after the UI's next tick)
    #[cfg(windows)]
    if (b.overlay != Overlay::None || b.menu.is_some())
        && matches!(raw, Raw::KeyDown(_) | Raw::Button { down: true, .. })
    {
        b.game_away = !b.game.is_some_and(win::is_front);
    }
    let (action, swallow) = {
        let mut st = s.state.lock().unwrap_or_else(|e| e.into_inner());
        #[cfg(windows)]
        st.set_mods(win::mods_now());
        st.feed(raw, &b)
    };
    if action != Action::None {
        s.bindings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .after(action);
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

    /// `game` (an `HWND` as a number) is the window in front, or owns the window in front.
    pub fn is_front(game: isize) -> bool {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            GA_ROOTOWNER, GetAncestor, GetForegroundWindow,
        };
        let fg = unsafe { GetForegroundWindow() };
        if fg.is_invalid() {
            return false;
        }
        fg.0 as isize == game || unsafe { GetAncestor(fg, GA_ROOTOWNER) } == HWND(game as *mut _)
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
            ..Bindings::default()
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
    fn a_key_with_modifiers_needs_exactly_those() {
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
        // Shift+R is not Ctrl+R either
        s.feed(Raw::KeyUp(0xA2), &b);
        s.feed(Raw::KeyDown(0xA1), &b);
        assert_eq!(s.feed(Raw::KeyDown(0x52), &b).0, Action::None);
        s.feed(Raw::KeyUp(0x52), &b);
        s.feed(Raw::KeyUp(0xA1), &b);
        assert_eq!(s.feed(Raw::KeyDown(0x52), &b).0, Action::None);
    }

    #[test]
    fn a_plain_key_fires_while_ctrl_shift_or_alt_is_held() {
        let b = Bindings {
            read: Key::parse("Mouse 4"),
            menu: Key::parse("F7"),
            capture: false,
            ..Bindings::default()
        };
        let x1 = |down| Raw::Button { b: Btn::X1, down };
        for m in [0xA0u16, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0x10, 0x11, 0x12] {
            let mut s = State::default();
            s.feed(Raw::KeyDown(m), &b);
            // sprinting with Shift (or crouching with Ctrl) and pressing Mouse 4 or F7
            assert_eq!(s.feed(x1(true), &b), (Action::Read, false), "{m:#x}");
            s.feed(x1(false), &b);
            assert_eq!(
                s.feed(Raw::KeyDown(0x76), &b),
                (Action::Menu, false),
                "{m:#x}"
            );
            s.feed(Raw::KeyUp(0x76), &b);
        }
        // all three at once
        let mut s = State::default();
        s.set_mods(Mods {
            ctrl: true,
            shift: true,
            alt: true,
            win: false,
        });
        assert_eq!(s.feed(Raw::KeyDown(0x76), &b).0, Action::Menu);
        // Win+F7 is a Windows shortcut, not F7; Mouse 4 takes no modifiers and still fires
        let mut s = State::default();
        s.feed(Raw::KeyDown(0x5B), &b);
        assert_eq!(s.feed(Raw::KeyDown(0x76), &b).0, Action::None);
        assert_eq!(s.feed(x1(true), &b).0, Action::Read);
    }

    #[test]
    fn the_more_specific_key_wins() {
        // F7 reads, Ctrl+F7 opens the menu
        let b = Bindings {
            read: Key::parse("F7"),
            menu: Key::parse("Ctrl+F7"),
            capture: false,
            ..Bindings::default()
        };
        let mut s = State::default();
        assert_eq!(s.feed(Raw::KeyDown(0x76), &b).0, Action::Read);
        s.feed(Raw::KeyUp(0x76), &b);
        s.feed(Raw::KeyDown(0xA2), &b);
        assert_eq!(s.feed(Raw::KeyDown(0x76), &b).0, Action::Menu);
        s.feed(Raw::KeyUp(0x76), &b);
        // Ctrl+Shift+F7 is not Ctrl+F7: the plain F7 fires
        s.feed(Raw::KeyDown(0xA0), &b);
        assert_eq!(s.feed(Raw::KeyDown(0x76), &b).0, Action::Read);
        s.feed(Raw::KeyUp(0x76), &b);
        s.feed(Raw::KeyUp(0xA0), &b);
        s.feed(Raw::KeyUp(0xA2), &b);
        // the other way round: the menu key plain, the read key with Shift
        let b = Bindings {
            read: Key::parse("Shift+F7"),
            menu: Key::parse("F7"),
            capture: false,
            ..Bindings::default()
        };
        let mut s = State::default();
        assert_eq!(s.feed(Raw::KeyDown(0x76), &b).0, Action::Menu);
        s.feed(Raw::KeyUp(0x76), &b);
        s.feed(Raw::KeyDown(0xA1), &b);
        assert_eq!(s.feed(Raw::KeyDown(0x76), &b).0, Action::Read);
        s.feed(Raw::KeyUp(0x76), &b);
        s.feed(Raw::KeyDown(0xA4), &b);
        assert_eq!(
            s.feed(Raw::KeyDown(0x76), &b).0,
            Action::Menu,
            "Shift+Alt+F7"
        );
    }

    #[test]
    fn capture_ignores_windows_shortcuts_and_keeps_waiting() {
        let on = Bindings {
            capture: true,
            ..Bindings::default()
        };
        // (modifier key, key): Alt+Tab, Alt+Esc, Ctrl+Esc, Win+R, Win+Tab, right Win+D
        for (m, vk) in [
            (0xA4u16, VK_TAB),
            (0xA5, VK_ESCAPE),
            (0xA2, VK_ESCAPE),
            (0x5B, 0x52),
            (0x5B, VK_TAB),
            (0x5C, 0x44),
        ] {
            let mut s = State::default();
            assert_eq!(s.feed(Raw::KeyDown(m), &on), (Action::None, false));
            // not taken and not swallowed: Windows gets it
            assert_eq!(
                s.feed(Raw::KeyDown(vk), &on),
                (Action::None, false),
                "{m:#x} {vk:#x}"
            );
            assert_eq!(s.feed(Raw::KeyUp(vk), &on), (Action::None, false));
            assert_eq!(s.feed(Raw::KeyUp(m), &on), (Action::None, false));
            // capture still waits: the next plain key is taken
            assert_eq!(
                s.feed(Raw::KeyDown(0x76), &on),
                (Action::Captured(kb(0x76)), true),
                "{m:#x} {vk:#x}"
            );
        }
        // Win alone is never a key, and Win with a mouse button is not taken either
        let mut s = State::default();
        assert_eq!(s.feed(Raw::KeyDown(0x5B), &on), (Action::None, false));
        assert_eq!(s.feed(Raw::KeyUp(0x5B), &on), (Action::None, false));
        s.feed(Raw::KeyDown(0x5B), &on);
        let x1 = |down| Raw::Button { b: Btn::X1, down };
        assert_eq!(s.feed(x1(true), &on), (Action::None, false));
        assert_eq!(s.feed(x1(false), &on), (Action::None, false));
        s.feed(Raw::KeyUp(0x5B), &on);
        // other combos are still keys: Ctrl+Tab is taken, Shift+Esc cancels as Esc does
        let mut s = State::default();
        s.feed(Raw::KeyDown(0xA2), &on);
        assert!(matches!(
            s.feed(Raw::KeyDown(VK_TAB), &on).0,
            Action::Captured(_)
        ));
        let mut s = State::default();
        s.feed(Raw::KeyDown(0xA0), &on);
        assert_eq!(
            s.feed(Raw::KeyDown(VK_ESCAPE), &on),
            (Action::Cancelled, true)
        );
        // a mouse button while Ctrl is held is still taken (it takes no modifiers)
        let mut s = State::default();
        s.feed(Raw::KeyDown(0xA2), &on);
        assert_eq!(
            s.feed(x1(true), &on),
            (Action::Captured(Key::Mouse(4)), true)
        );
    }

    #[test]
    fn a_lost_key_up_of_a_modifier_is_fixed_by_the_state_from_windows() {
        let b = Bindings {
            read: Key::parse("R"),
            ..Bindings::default()
        };
        let mut s = State::default();
        // Win went down, its key-up never came (Win+L, a UAC prompt)
        s.feed(Raw::KeyDown(0x5B), &b);
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

    fn with(o: Overlay) -> Bindings {
        Bindings {
            read: Some(Key::Mouse(4)),
            menu: Key::parse("F7"),
            overlay: o,
            ..Bindings::default()
        }
    }

    const UP: u16 = 0x26;
    const DOWN: u16 = 0x28;
    const ENTER: u16 = 0x0D;
    const ESC: u16 = 0x1B;
    const DEL: u16 = 0x2E;
    const F7: u16 = 0x76;

    #[test]
    fn the_open_menu_takes_its_keys_and_the_game_gets_the_rest() {
        let mut s = State::default();
        let m = with(Overlay::Menu);
        for (vk, n) in [
            (UP, Nav::Up),
            (DOWN, Nav::Down),
            (0x25, Nav::Left),
            (0x27, Nav::Right),
            (ENTER, Nav::Enter),
            (0x20, Nav::Space),
            (ESC, Nav::Esc),
        ] {
            assert_eq!(s.feed(Raw::KeyDown(vk), &m), (Action::Nav(n), true));
            assert_eq!(s.feed(Raw::KeyUp(vk), &m), (Action::None, true));
        }
        // W, Delete and the read key go to the game; the read key does nothing while the menu is open
        assert_eq!(s.feed(Raw::KeyDown(0x57), &m), (Action::None, false));
        assert_eq!(s.feed(Raw::KeyDown(DEL), &m), (Action::None, false));
        let x1 = Raw::Button {
            b: Btn::X1,
            down: true,
        };
        assert_eq!(s.feed(x1, &m), (Action::None, false));
        // the quick menu key closes it, and is taken
        assert_eq!(s.feed(Raw::KeyDown(F7), &m), (Action::Menu, true));
        assert_eq!(s.feed(Raw::KeyUp(F7), &m), (Action::None, true));
        // closed: every key goes to the game; the quick menu key opens the menu and goes through too
        let none = with(Overlay::None);
        assert_eq!(s.feed(Raw::KeyDown(UP), &none), (Action::None, false));
        assert_eq!(s.feed(Raw::KeyDown(F7), &none), (Action::Menu, false));
    }

    #[test]
    fn zone_edit_takes_delete_backspace_and_esc_only() {
        let mut s = State::default();
        let z = with(Overlay::Zones);
        assert_eq!(
            s.feed(Raw::KeyDown(DEL), &z),
            (Action::Nav(Nav::Delete), true)
        );
        assert_eq!(
            s.feed(Raw::KeyDown(0x08), &z),
            (Action::Nav(Nav::Delete), true)
        );
        assert_eq!(s.feed(Raw::KeyDown(ESC), &z), (Action::Nav(Nav::Esc), true));
        assert_eq!(s.feed(Raw::KeyDown(UP), &z), (Action::None, false));
        assert_eq!(s.feed(Raw::KeyDown(ENTER), &z), (Action::None, false));
        // the quick menu key goes back to the menu
        assert_eq!(s.feed(Raw::KeyDown(F7), &z), (Action::Menu, true));
        assert_eq!(Overlay::Zones.after_menu_key(), Overlay::Menu);
        assert_eq!(Overlay::Menu.after_menu_key(), Overlay::None);
        assert_eq!(Overlay::None.after_menu_key(), Overlay::Menu);
    }

    #[test]
    fn held_arrows_repeat_but_a_held_enter_or_esc_acts_once() {
        let mut s = State::default();
        let m = with(Overlay::Menu);
        assert_eq!(
            s.feed(Raw::KeyDown(DOWN), &m),
            (Action::Nav(Nav::Down), true)
        );
        assert_eq!(
            s.feed(Raw::KeyDown(DOWN), &m),
            (Action::Nav(Nav::Down), true)
        );
        assert_eq!(s.feed(Raw::KeyUp(DOWN), &m), (Action::None, true));
        assert_eq!(
            s.feed(Raw::KeyDown(ENTER), &m),
            (Action::Nav(Nav::Enter), true)
        );
        // Enter held on Quit must not press it twice
        assert_eq!(s.feed(Raw::KeyDown(ENTER), &m), (Action::None, true));
        assert_eq!(s.feed(Raw::KeyUp(ENTER), &m), (Action::None, true));
        // Esc closed the menu on its key-down: its repeats and key-up stay with DialogReader, and do nothing
        assert_eq!(s.feed(Raw::KeyDown(ESC), &m), (Action::Nav(Nav::Esc), true));
        let none = with(Overlay::None);
        assert_eq!(s.feed(Raw::KeyDown(ESC), &none), (Action::None, true));
        assert_eq!(s.feed(Raw::KeyUp(ESC), &none), (Action::None, true));
        // the next Esc is the game's
        assert_eq!(s.feed(Raw::KeyDown(ESC), &none), (Action::None, false));
    }

    #[test]
    fn a_key_held_before_the_menu_opened_stays_with_the_game() {
        let mut s = State::default();
        assert_eq!(
            s.feed(Raw::KeyDown(UP), &with(Overlay::None)),
            (Action::None, false)
        );
        let m = with(Overlay::Menu);
        assert_eq!(s.feed(Raw::KeyDown(UP), &m), (Action::None, false));
        assert_eq!(s.feed(Raw::KeyUp(UP), &m), (Action::None, false));
        assert_eq!(s.feed(Raw::KeyDown(UP), &m), (Action::Nav(Nav::Up), true));
    }

    #[test]
    fn the_menu_opens_only_over_the_game_in_front() {
        let mut s = State::default();
        let away = Bindings {
            game_away: true,
            ..with(Overlay::None)
        };
        assert_eq!(s.feed(Raw::KeyDown(F7), &away), (Action::None, false));
        assert_eq!(s.feed(Raw::KeyUp(F7), &away), (Action::None, false));
        // a side button as the quick menu key: the same rules
        let mouse = Bindings {
            menu: Some(Key::Mouse(5)),
            ..Bindings::default()
        };
        let x2 = |down| Raw::Button { b: Btn::X2, down };
        assert_eq!(s.feed(x2(true), &mouse), (Action::Menu, false));
        assert_eq!(s.feed(x2(false), &mouse), (Action::None, false));
        let open = Bindings {
            overlay: Overlay::Menu,
            ..mouse.clone()
        };
        assert_eq!(s.feed(x2(true), &open), (Action::Menu, true));
        assert_eq!(s.feed(x2(false), &open), (Action::None, true));
        let away = Bindings {
            game_away: true,
            ..mouse
        };
        assert_eq!(s.feed(x2(true), &away), (Action::None, false));
    }

    #[test]
    fn the_hook_changes_the_overlay_at_once() {
        let mut b = with(Overlay::None);
        b.after(Action::Menu);
        assert_eq!(b.overlay, Overlay::Menu);
        // Esc in the voice list goes back to the menu
        b.esc_closes = false;
        b.after(Action::Nav(Nav::Esc));
        assert_eq!(b.overlay, Overlay::Menu);
        b.esc_closes = true;
        b.after(Action::Nav(Nav::Esc));
        assert_eq!(b.overlay, Overlay::None);
        b.overlay = Overlay::Zones;
        b.after(Action::Menu);
        assert_eq!(b.overlay, Overlay::Menu);
        b.after(Action::Menu);
        assert_eq!(b.overlay, Overlay::None);
        b.capture = true;
        b.after(Action::Cancelled);
        assert!(!b.capture);
    }

    #[test]
    fn the_open_menu_lets_windows_shortcuts_through() {
        const CTRL: u16 = 0xA2;
        const SHIFT: u16 = 0xA0;
        const ALT: u16 = 0xA4;
        const WIN: u16 = 0x5B;
        const LEFT: u16 = 0x25;
        const SPACE: u16 = 0x20;
        let m = with(Overlay::Menu);
        for (held, vk) in [
            (&[CTRL][..], ESC),
            (&[CTRL, SHIFT][..], ESC),
            (&[ALT][..], ESC),
            (&[WIN][..], UP),
            (&[WIN, SHIFT][..], LEFT),
            (&[ALT][..], ENTER),
            (&[ALT][..], SPACE),
            (&[CTRL][..], DOWN),
        ] {
            let mut s = State::default();
            for &k in held {
                assert_eq!(s.feed(Raw::KeyDown(k), &m), (Action::None, false));
            }
            assert_eq!(
                s.feed(Raw::KeyDown(vk), &m),
                (Action::None, false),
                "{held:?} + {vk:#x}"
            );
            // its repeats and key-up go through too
            assert_eq!(s.feed(Raw::KeyDown(vk), &m), (Action::None, false));
            assert_eq!(s.feed(Raw::KeyUp(vk), &m), (Action::None, false));
            for &k in held {
                assert_eq!(s.feed(Raw::KeyUp(k), &m), (Action::None, false));
            }
            // with the modifiers let go, the key is the menu's again
            let n = Nav::of(vk, Overlay::Menu).unwrap();
            assert_eq!(s.feed(Raw::KeyDown(vk), &m), (Action::Nav(n), true));
        }
        // Shift alone does not stop a key
        let mut s = State::default();
        s.feed(Raw::KeyDown(SHIFT), &m);
        assert_eq!(s.feed(Raw::KeyDown(UP), &m), (Action::Nav(Nav::Up), true));
        // an arrow held before Win went down: still DialogReader's, but its repeats no longer move the menu
        let mut s = State::default();
        assert_eq!(
            s.feed(Raw::KeyDown(DOWN), &m),
            (Action::Nav(Nav::Down), true)
        );
        s.feed(Raw::KeyDown(WIN), &m);
        assert_eq!(s.feed(Raw::KeyDown(DOWN), &m), (Action::None, true));
        assert_eq!(s.feed(Raw::KeyUp(DOWN), &m), (Action::None, true));
        // zone edit: Ctrl+Backspace and Alt+Esc are not its keys either
        let z = with(Overlay::Zones);
        let mut s = State::default();
        s.feed(Raw::KeyDown(CTRL), &z);
        assert_eq!(s.feed(Raw::KeyDown(0x08), &z), (Action::None, false));
        s.feed(Raw::KeyUp(CTRL), &z);
        s.feed(Raw::KeyDown(ALT), &z);
        assert_eq!(s.feed(Raw::KeyDown(ESC), &z), (Action::None, false));
    }

    #[test]
    fn an_open_overlay_takes_no_keys_while_the_game_is_away() {
        let mut s = State::default();
        let away = Bindings {
            game_away: true,
            ..with(Overlay::Menu)
        };
        for vk in [UP, DOWN, ENTER, ESC] {
            assert_eq!(s.feed(Raw::KeyDown(vk), &away), (Action::None, false));
            assert_eq!(s.feed(Raw::KeyUp(vk), &away), (Action::None, false));
        }
        let z = Bindings {
            game_away: true,
            ..with(Overlay::Zones)
        };
        assert_eq!(s.feed(Raw::KeyDown(DEL), &z), (Action::None, false));
        // the quick menu key still closes the menu (the UI closes it anyway)
        assert_eq!(s.feed(Raw::KeyDown(F7), &away), (Action::Menu, true));
    }

    #[test]
    fn capture_wins_over_an_open_overlay() {
        let mut s = State::default();
        let b = Bindings {
            capture: true,
            ..with(Overlay::Menu)
        };
        assert_eq!(
            s.feed(Raw::KeyDown(UP), &b),
            (Action::Captured(kb(UP)), true)
        );
    }
}
