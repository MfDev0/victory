//! The F1 settings menu and the F3 key, drawn as the box of numbers is
//! (`crate::fpsbox`): text copied onto the swap chain's buffer, here in the
//! middle of the picture.
//!
//! F1 shows and hides the menu while our window has the focus; Up and Down
//! pick a line, Left and Right change it, Enter saves or closes, Esc closes.
//! While it is shown the title sees no keyboard ([`crate::SETTINGS_MENU_OPEN`],
//! read by the input module); a pad still works. F3 goes through the box of
//! numbers' forms (all of it, the frame rate alone, none).
//!
//! What changes at once: DLSS Frame Generation's frames shown for each one
//! rendered (when it was made ready at the start: VIctory Try does that
//! whenever DLSS runs) and the box of numbers. What takes the next start:
//! the DLSS mode and the window and input switches (borderless fullscreen,
//! mouse capture, English keyboard: `VICTORY_FULLSCREEN`,
//! `VICTORY_MOUSE_CAPTURE`, `VICTORY_IME_ENGLISH`). "SAVE" writes the choices to the file `VICTORY_MENU_FILE`
//! names (VIctory Try sets it and reads it at every start, over its own
//! settings).
//!
//! Try 6.24, after trustno1-coder/VIctory-NVIDIA's settings menu
//! (`crates/victory-gpu/src/host/menu.rs`, GPL-3.0-or-later): the same keys,
//! the same file format (`VICTORY_MENU_...=value` lines) and its live frame
//! generation switch, on this fork's text box and its launcher's choices.
//! The keys are read on a thread of their own every 16 ms, not once a
//! present: at a low frame rate a short press fell between two presents.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};
use std::time::Duration;

use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use crate::fpsbox::{Place, TextBox, fit};

/// Glyphs across the menu.
const COLUMNS: usize = 46;

/// A line of the menu.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Line {
    FrameGen,
    Dlss,
    Numbers,
    /// One of [`TOGGLES`], by its index.
    Toggle(usize),
    Save,
    Close,
}

const LINES: [Line; 8] = [
    Line::FrameGen,
    Line::Dlss,
    Line::Toggle(0),
    Line::Toggle(1),
    Line::Toggle(2),
    Line::Numbers,
    Line::Save,
    Line::Close,
];

/// The on/off switches applied at the next start: the run's switch, the
/// saved file's name for it, and the menu's label (as nv's menu has them).
const TOGGLES: [(&str, &str, &str); 3] = [
    ("VICTORY_FULLSCREEN", "VICTORY_MENU_FULLSCREEN", "FULLSCREEN"),
    ("VICTORY_MOUSE_CAPTURE", "VICTORY_MENU_MOUSE_CAPTURE", "MOUSE CAPTURE"),
    ("VICTORY_IME_ENGLISH", "VICTORY_MENU_IME_ENGLISH", "ENGLISH KEYBOARD"),
];

/// The DLSS modes the menu offers, by their switch names, and how they are shown.
const DLSS: [(&str, &str); 6] = [
    ("quality", "QUALITY"),
    ("balanced", "BALANCED"),
    ("performance", "PERFORMANCE"),
    ("ultraperformance", "ULTRA PERFORMANCE"),
    ("dlaa", "DLAA (NATIVE)"),
    ("off", "OFF"),
];

/// Frame generation's choices: 0 off, else frames shown for each one rendered.
const FRAMEGEN: [u32; 6] = [0, 2, 3, 4, 5, 6];

/// What the menu shows and has changed.
#[derive(Clone, Debug, PartialEq)]
struct State {
    open: bool,
    picked: usize,
    /// Index into [`FRAMEGEN`].
    framegen: usize,
    /// Index into [`DLSS`].
    dlss: usize,
    /// [`TOGGLES`], on or off.
    toggles: [bool; 3],
    /// Presses of F3 (and changes of the line) since the start.
    numbers: u32,
    /// The note under the lines.
    note: &'static str,
    /// Whether anything changed since the last save.
    changed: bool,
}

/// The start's choices, from the switches the run was given.
fn first(var: impl Fn(&str) -> Option<String>) -> State {
    let on = |name: &str| var(name).is_some_and(|v| v.trim() == "1");
    let dlss = if on("VICTORY_DLSS") {
        let mode = var("VICTORY_DLSS_MODE").unwrap_or_default().trim().to_ascii_lowercase();
        DLSS.iter().position(|(name, _)| *name == mode).unwrap_or(0)
    } else {
        DLSS.len() - 1
    };
    // (`VICTORY_FRAMEGEN=1` alone is our own frames in between, not DLSS's.)
    let times = if on("VICTORY_FRAMEGEN") && (on("VICTORY_DLSS") || var("VICTORY_FRAMEGEN_X").is_some_and(|v| !v.trim().is_empty())) {
        var("VICTORY_FRAMEGEN_X").and_then(|v| v.trim().parse::<u32>().ok()).map_or(2, |n| if n == 0 { 0 } else { n.clamp(2, 6) })
    } else {
        0
    };
    State {
        open: false,
        picked: 0,
        framegen: FRAMEGEN.iter().position(|&n| n == times).unwrap_or(0),
        dlss,
        toggles: TOGGLES.map(|(name, ..)| on(name)),
        numbers: 0,
        note: "",
        changed: false,
    }
}

/// A key the menu takes.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Key {
    F1,
    F3,
    Up,
    Down,
    Left,
    Right,
    Enter,
    Esc,
}

/// What a key asks to be done beyond the menu's own state.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Effect {
    None,
    /// Frame generation, live: frames shown for each one rendered (0 off).
    FrameGen(u32),
    /// The box of numbers goes on to its next form.
    Numbers,
    Save,
}

/// `state` after `key`, and what else it asks for. `live`: whether frame
/// generation can be switched while the title runs.
fn press(state: &mut State, key: Key, live: bool) -> Effect {
    let step = |at: &mut usize, len: usize, right: bool| *at = if right { (*at + 1) % len } else { (*at + len - 1) % len };
    match key {
        Key::F3 => {
            state.numbers += 1;
            return Effect::Numbers;
        }
        Key::F1 => {
            state.open = !state.open;
            state.note = "";
            return Effect::None;
        }
        _ if !state.open => return Effect::None,
        Key::Esc => state.open = false,
        Key::Up => step(&mut state.picked, LINES.len(), false),
        Key::Down => step(&mut state.picked, LINES.len(), true),
        Key::Left | Key::Right => {
            let right = key == Key::Right;
            match LINES[state.picked] {
                Line::FrameGen => {
                    step(&mut state.framegen, FRAMEGEN.len(), right);
                    state.changed = true;
                    state.note = if live { "" } else { "FRAME GENERATION: AT THE NEXT START (SAVE)" };
                    if live {
                        return Effect::FrameGen(FRAMEGEN[state.framegen]);
                    }
                }
                Line::Dlss => {
                    step(&mut state.dlss, DLSS.len(), right);
                    state.changed = true;
                    state.note = "DLSS: AT THE NEXT START (SAVE)";
                }
                Line::Toggle(i) => {
                    state.toggles[i] = !state.toggles[i];
                    state.changed = true;
                    state.note = "AT THE NEXT START (SAVE)";
                }
                Line::Numbers => {
                    state.numbers += 1;
                    return Effect::Numbers;
                }
                Line::Save | Line::Close => {}
            }
        }
        Key::Enter => match LINES[state.picked] {
            Line::Save => return Effect::Save,
            Line::Close => state.open = false,
            _ => {}
        },
    }
    Effect::None
}

/// The box of numbers' form after `presses` from the start's.
fn numbers_name(presses: u32) -> &'static str {
    let start = match std::env::var("VICTORY_FPS_COUNTER").as_deref() {
        Ok("0") => 2,
        Ok("fps") => 1,
        _ => 0,
    };
    ["ALL", "FPS ONLY", "OFF"][(start + presses as usize) % 3]
}

/// The menu's text: lines of [`COLUMNS`] glyphs.
fn text(state: &State, live: bool, running: (bool, u32)) -> String {
    let times = FRAMEGEN[state.framegen];
    let framegen = match (times, live) {
        (0, _) => "OFF".to_owned(),
        (n, true) => format!("{n}X"),
        (n, false) => format!("{n}X (NEXT START)"),
    };
    let values = LINES.map(|line| match line {
        Line::FrameGen => ("FRAME GENERATION", framegen.clone()),
        Line::Dlss => ("DLSS (NEXT START)", DLSS[state.dlss].1.to_owned()),
        Line::Toggle(i) => (TOGGLES[i].2, if state.toggles[i] { "ON" } else { "OFF" }.to_owned()),
        Line::Numbers => ("NUMBERS BOX (F3)", numbers_name(state.numbers).to_owned()),
        Line::Save => ("SAVE", if state.changed { "(NOT SAVED YET)".to_owned() } else { String::new() }),
        Line::Close => ("CLOSE", String::new()),
    });
    let mut out = vec![
        "VICTORY SETTINGS (F1)".to_owned(),
        "UP DOWN: PICK  LEFT RIGHT: CHANGE  ENTER: DO".to_owned(),
        match running {
            (true, per) if per > 1 => format!("NOW: DLSS ON, FRAME GENERATION {per}X"),
            (true, _) => "NOW: DLSS ON".to_owned(),
            (false, _) => "NOW: DLSS NOT RUNNING".to_owned(),
        },
        String::new(),
    ];
    for (i, (label, value)) in values.into_iter().enumerate() {
        let mark = if i == state.picked { ">" } else { " " };
        out.push(if value.is_empty() { format!("{mark} {label}") } else { format!("{mark} {label:<18}< {value} >") });
    }
    out.push(String::new());
    out.push(state.note.to_owned());
    out.into_iter().map(|line| fit(&line, COLUMNS)).collect::<Vec<_>>().join("\n")
}

/// Lines in [`text`].
const ROWS: usize = 4 + LINES.len() + 2;

/// The saved file's text: the menu's choices over the lines `old` had.
fn saved(old: &str, state: &State) -> String {
    let mut ours = vec![
        ("VICTORY_MENU_DLSS", DLSS[state.dlss].0.to_owned()),
        ("VICTORY_MENU_FRAMEGEN", FRAMEGEN[state.framegen].to_string()),
    ];
    for (i, (_, name, _)) in TOGGLES.iter().enumerate() {
        ours.push((name, u8::from(state.toggles[i]).to_string()));
    }
    let mut out: Vec<String> = old
        .lines()
        .filter(|line| !ours.iter().any(|(name, _)| line.trim_start().starts_with(&format!("{name}="))))
        .map(str::to_owned)
        .collect();
    out.extend(ours.iter().map(|(name, value)| format!("{name}={value}")));
    out.join("\r\n") + "\r\n"
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static KEYS: Once = Once::new();

/// Whether our process has the window with the keyboard focus.
fn focused() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    let mut pid = 0u32;
    // SAFETY: plain queries.
    unsafe { GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut pid)) };
    pid == std::process::id()
}

fn save(state: &mut State) {
    let Some(path) = std::env::var_os("VICTORY_MENU_FILE").filter(|p| !p.is_empty()) else {
        state.note = "NOT SAVED: STARTED WITHOUT VICTORY TRY";
        return;
    };
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    match std::fs::write(&path, saved(&old, state)) {
        Ok(()) => {
            state.changed = false;
            state.note = "SAVED: THE NEXT START USES THEM";
            tracing::info!(file = %std::path::Path::new(&path).display(), "settings menu: saved");
        }
        Err(e) => {
            state.note = "NOT SAVED (SEE THE LOG)";
            tracing::warn!(error = %e, "settings menu: not saved");
        }
    }
}

/// The key thread: every 16 ms, the keys pressed since the last look.
fn watch_keys() {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_DOWN, VK_ESCAPE, VK_F1, VK_F3, VK_LEFT, VK_RETURN, VK_RIGHT, VK_UP,
    };
    let keys = [
        (VK_F1, Key::F1),
        (VK_F3, Key::F3),
        (VK_UP, Key::Up),
        (VK_DOWN, Key::Down),
        (VK_LEFT, Key::Left),
        (VK_RIGHT, Key::Right),
        (VK_RETURN, Key::Enter),
        (VK_ESCAPE, Key::Esc),
    ];
    let mut held = [false; 8];
    loop {
        std::thread::sleep(Duration::from_millis(16));
        let focus = focused();
        for (i, &(vk, key)) in keys.iter().enumerate() {
            // SAFETY: plain query.
            let down = focus && unsafe { GetAsyncKeyState(i32::from(vk)) } < 0;
            let pressed = down && !held[i];
            held[i] = down;
            if !pressed {
                continue;
            }
            let live = crate::streamline::framegen_live();
            let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
            let state = guard.get_or_insert_with(|| first(|name| std::env::var(name).ok()));
            match press(state, key, live) {
                Effect::None => {}
                Effect::FrameGen(times) => crate::streamline::set_framegen(times),
                Effect::Numbers => crate::fpsbox::next_mode(),
                Effect::Save => save(state),
            }
            crate::SETTINGS_MENU_OPEN.store(state.open, Ordering::Relaxed);
        }
    }
}

static BOX: Mutex<Option<TextBox>> = Mutex::new(None);
static OFF: AtomicBool = AtomicBool::new(false);

/// At a present: the key thread started (the first time), and the menu,
/// if open, copied onto the back buffer `back` (a copy's destination in
/// `list`). `VICTORY_MENU=0`: no menu and no keys of ours.
pub(crate) fn draw(host: &crate::host::Host, list: &ID3D12GraphicsCommandList, back: &ID3D12Resource, dims: (u32, u32, DXGI_FORMAT)) {
    KEYS.call_once(|| {
        if std::env::var("VICTORY_MENU").is_ok_and(|v| v.trim() == "0") {
            OFF.store(true, Ordering::Relaxed);
            return;
        }
        if let Err(e) = std::thread::Builder::new().name("victory-menu-keys".into()).spawn(watch_keys) {
            tracing::warn!(error = %e, "settings menu: no key thread");
        }
    });
    if OFF.load(Ordering::Relaxed) {
        return;
    }
    let Some(state) = STATE.lock().unwrap_or_else(|e| e.into_inner()).clone().filter(|s| s.open) else {
        return;
    };
    let live = crate::streamline::framegen_live();
    let running = (crate::streamline::upscaling() || live, crate::streamline::shown_per_frame());
    let text = text(&state, live, running);
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut h);
        h.finish()
    };
    let mut panel = BOX.lock().unwrap_or_else(|e| e.into_inner());
    panel.get_or_insert_with(TextBox::default).draw(host, list, back, dims, (COLUMNS, ROWS), key, || text, Place::Middle);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| pairs.iter().find(|(n, _)| *n == name).map(|(_, v)| v.to_string())
    }

    #[test]
    fn the_start_s_choices_come_from_the_switches() {
        let s = first(vars(&[("VICTORY_DLSS", "1"), ("VICTORY_DLSS_MODE", "performance"), ("VICTORY_FRAMEGEN", "1"), ("VICTORY_FRAMEGEN_X", "4")]));
        assert_eq!((DLSS[s.dlss].0, FRAMEGEN[s.framegen]), ("performance", 4));
        let s = first(vars(&[("VICTORY_DLSS", "1"), ("VICTORY_FRAMEGEN", "1"), ("VICTORY_FRAMEGEN_X", "0")]));
        assert_eq!((DLSS[s.dlss].0, FRAMEGEN[s.framegen]), ("quality", 0));
        let s = first(vars(&[]));
        assert_eq!((DLSS[s.dlss].0, FRAMEGEN[s.framegen]), ("off", 0));
        // Our own frames in between (VIctory Try's M) are not DLSS's.
        let s = first(vars(&[("VICTORY_FRAMEGEN", "1")]));
        assert_eq!((DLSS[s.dlss].0, FRAMEGEN[s.framegen]), ("off", 0));
        let s = first(vars(&[("VICTORY_DLSS", "1"), ("VICTORY_FRAMEGEN", "1")]));
        assert_eq!(FRAMEGEN[s.framegen], 2);
        assert_eq!(s.toggles, [false; 3]);
        let s = first(vars(&[("VICTORY_MOUSE_CAPTURE", "1")]));
        assert_eq!(s.toggles, [false, true, false]);
    }

    #[test]
    fn keys_move_change_and_close() {
        let mut s = first(vars(&[("VICTORY_DLSS", "1")]));
        // Closed: only F1 and F3 do anything.
        assert_eq!(press(&mut s, Key::Right, true), Effect::None);
        assert_eq!(s.framegen, 0);
        assert_eq!(press(&mut s, Key::F3, true), Effect::Numbers);
        press(&mut s, Key::F1, true);
        assert!(s.open);
        // Frame generation, live: off -> 2x -> 3x; back past off to 6x.
        assert_eq!(press(&mut s, Key::Right, true), Effect::FrameGen(2));
        assert_eq!(press(&mut s, Key::Right, true), Effect::FrameGen(3));
        press(&mut s, Key::Left, true);
        press(&mut s, Key::Left, true);
        assert_eq!(press(&mut s, Key::Left, true), Effect::FrameGen(6));
        // Not live: kept for the next start.
        assert_eq!(press(&mut s, Key::Right, false), Effect::None);
        assert_eq!(FRAMEGEN[s.framegen], 0);
        // Down to DLSS, then up past the top to CLOSE.
        press(&mut s, Key::Down, true);
        press(&mut s, Key::Right, true);
        assert_eq!(DLSS[s.dlss].0, "balanced");
        // The window and input switches flip.
        press(&mut s, Key::Down, true);
        assert_eq!(LINES[s.picked], Line::Toggle(0));
        assert_eq!(press(&mut s, Key::Left, true), Effect::None);
        assert_eq!(s.toggles, [true, false, false]);
        press(&mut s, Key::Down, true);
        press(&mut s, Key::Down, true);
        press(&mut s, Key::Down, true);
        press(&mut s, Key::Down, true);
        assert_eq!(press(&mut s, Key::Enter, true), Effect::Save);
        press(&mut s, Key::Up, true);
        press(&mut s, Key::Up, true);
        press(&mut s, Key::Up, true);
        press(&mut s, Key::Up, true);
        press(&mut s, Key::Up, true);
        press(&mut s, Key::Up, true);
        press(&mut s, Key::Up, true);
        assert_eq!(LINES[s.picked], Line::Close);
        press(&mut s, Key::Enter, true);
        assert!(!s.open);
        press(&mut s, Key::F1, true);
        press(&mut s, Key::Esc, true);
        assert!(!s.open);
    }

    #[test]
    fn every_line_fits_and_has_glyphs() {
        let mut s = first(vars(&[("VICTORY_DLSS", "1"), ("VICTORY_FRAMEGEN", "1")]));
        s.dlss = 3;
        s.note = "FRAME GENERATION: AT THE NEXT START (SAVE)";
        s.changed = true;
        for (live, running) in [(true, (true, 4)), (false, (false, 1))] {
            let t = text(&s, live, running);
            let lines: Vec<&str> = t.split('\n').collect();
            assert_eq!(lines.len(), ROWS);
            for line in lines {
                assert_eq!(line.chars().count(), COLUMNS, "{line:?}");
                for c in line.chars().filter(|c| *c != ' ') {
                    assert!(crate::fpsbox::glyph(c).iter().any(|&r| r != 0), "no glyph for {c:?} in {line:?}");
                }
            }
        }
    }

    #[test]
    fn saving_keeps_other_lines_and_replaces_ours() {
        let mut s = first(vars(&[]));
        s.dlss = 1;
        s.framegen = 2;
        s.toggles = [false, true, false];
        let toggles = "VICTORY_MENU_FULLSCREEN=0\r\nVICTORY_MENU_MOUSE_CAPTURE=1\r\nVICTORY_MENU_IME_ENGLISH=0\r\n";
        let old = "VICTORY_AMBIENT=0.5\r\nVICTORY_MENU_DLSS=quality\r\nVICTORY_MENU_FULLSCREEN=1\r\n";
        assert_eq!(saved(old, &s), format!("VICTORY_AMBIENT=0.5\r\nVICTORY_MENU_DLSS=balanced\r\nVICTORY_MENU_FRAMEGEN=3\r\n{toggles}"));
        assert_eq!(saved("", &s), format!("VICTORY_MENU_DLSS=balanced\r\nVICTORY_MENU_FRAMEGEN=3\r\n{toggles}"));
    }
}
