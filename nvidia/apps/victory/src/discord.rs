//! Discord Rich Presence while the game runs: "Playing Grand Theft Auto VI", with the cover,
//! the time played, a line about the cat that changes every minute, and two buttons for the
//! friends who look at it: PLAY ON PC TOO and PET THE CAT. (Both open a rickroll sung by a
//! cat. You cannot click your own buttons in Discord: only others see them work.)
//!
//! It goes through the Discord app's own pipe on this PC (`\\.\pipe\discord-ipc-N`), in the
//! Discord app's small local protocol: a frame is the operation (u32 LE), the length (u32 LE)
//! and the JSON. Nothing goes over the network from here, and nothing happens without the
//! Discord app running. `victory play` starts `victory discord --watch <its pid>` beside
//! itself, a process of its own with no window, as the crash watcher is: whatever goes wrong
//! here never touches the game, and the presence ends with the game.
//!
//! `VICTORY_DISCORD=0` (F1, DISCORD STATUS) turns it off. `VICTORY_DISCORD_APP=<id>` names
//! another Discord application (the name after "Playing" is the application's name).
//!
//! Adapted from Neptune's (github.com/zm0kiemedia/Neptune_VI, `apps/victory/src/discord.rs`,
//! GPL-3.0), whose Discord application, named Grand Theft Auto VI, this uses. Added for the
//! Try build: the cover by URL (with Neptune's `logo` art if Discord refuses the URL), the
//! buttons, the cat's lines, pings answered, a reply checked for each frame, and a watcher.

use std::io::{Read, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The Discord application the presence is shown under: Neptune's, named Grand Theft Auto VI.
const APPLICATION: &str = "1575009406737846333";
/// The cover: Rockstar's GTA VI cover art (as iGrandTheftAuto.com serves it). Discord shows
/// a picture from a URL in place of an uploaded art asset.
const COVER: &str = "https://www.igrandtheftauto.com/content/images/large/grand-theft-auto-vi-official-cover-art-hi-res.jpg";
/// The application's own art, if a URL is refused.
const COVER_ASSET: &str = "logo";
/// The small picture in the cover's corner: the singing cat.
const CAT_PICTURE: &str = "https://i.ytimg.com/vi/K7XHy8nppf4/hqdefault.jpg";
/// "Rick Astley - Never Gonna Give You Up - Cat Cover" (Malow Meme, 2020).
const RICKROLL_CAT: &str = "https://www.youtube.com/watch?v=K7XHy8nppf4";
/// "Keyboard Cat Rick Roll." (2010).
const KEYBOARD_CAT: &str = "https://www.youtube.com/watch?v=PSe5lFRiuDo";
/// The buttons (Discord allows two, labels of at most 32 characters).
const BUTTONS: [(&str, &str); 2] = [("Play on PC too", RICKROLL_CAT), ("Pet the cat =^.^=", KEYBOARD_CAT)];
const DETAILS: &str = "Vice City, on PC (yes, really)";
/// The line under it, a new one every minute (Discord takes at most 5 changes in 20 s).
const STATES: [&str; 8] = [
    "Petting the cat between missions",
    "Cat in the passenger seat, 5 stars",
    "Robbing a bank, the cat keeps watch",
    "Frame gen on, cat asleep on the GPU",
    "Never gonna give you up, Vice City",
    "Feeding the cat at the diner",
    "The cat is driving. Send help",
    "Pre-ordered a second copy for the cat",
];
const EVERY: Duration = Duration::from_secs(60);
/// Discord not running, or closed: looked for again this often.
const AGAIN: Duration = Duration::from_secs(20);

const HANDSHAKE: u32 = 0;
const FRAME: u32 = 1;
const CLOSE: u32 = 2;
const PING: u32 = 3;
const PONG: u32 = 4;

/// One frame of Discord's local protocol: the operation, the length, the JSON.
fn frame(op: u32, json: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(8 + json.len());
    bytes.extend_from_slice(&op.to_le_bytes());
    bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
    bytes.extend_from_slice(json.as_bytes());
    bytes
}

/// `text` as a JSON string's inside.
fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if u32::from(c) < 0x20 => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out
}

/// The application named by `VICTORY_DISCORD_APP` (digits only), else Neptune's.
fn application() -> String {
    std::env::var("VICTORY_DISCORD_APP")
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| (17..=20).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_digit()))
        .unwrap_or_else(|| APPLICATION.to_owned())
}

/// The SET_ACTIVITY frame's JSON: for the game's process `pid`, played since `started`
/// (seconds since 1970), the `turn`th state, the cover by URL or as the application's art.
fn activity(pid: u32, started: u64, turn: usize, by_url: bool, nonce: &str) -> String {
    let state = STATES[turn % STATES.len()];
    let assets = if by_url {
        format!(
            r#""large_image":"{}","large_text":"Grand Theft Auto VI","small_image":"{}","small_text":"The cat is helping =^.^=""#,
            escaped(COVER),
            escaped(CAT_PICTURE)
        )
    } else {
        format!(r#""large_image":"{}","large_text":"Grand Theft Auto VI""#, escaped(COVER_ASSET))
    };
    let buttons = BUTTONS
        .iter()
        .map(|(label, url)| format!(r#"{{"label":"{}","url":"{}"}}"#, escaped(label), escaped(url)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"cmd":"SET_ACTIVITY","args":{{"pid":{pid},"activity":{{"details":"{}","state":"{}","timestamps":{{"start":{started}}},"assets":{{{assets}}},"buttons":[{buttons}],"instance":false}}}},"nonce":"{}"}}"#,
        escaped(DETAILS),
        escaped(state),
        escaped(nonce)
    )
}

/// Reads frames until one that is not a ping (answered with a pong): its operation and JSON.
fn reply<P: Read + Write>(pipe: &mut P) -> std::io::Result<(u32, String)> {
    loop {
        let mut head = [0u8; 8];
        pipe.read_exact(&mut head)?;
        let op = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
        let len = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
        if len > 1 << 20 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "a Discord frame of more than 1 MB"));
        }
        let mut body = vec![0u8; len];
        pipe.read_exact(&mut body)?;
        let body = String::from_utf8_lossy(&body).into_owned();
        if op == PING {
            pipe.write_all(&frame(PONG, &body))?;
            continue;
        }
        return Ok((op, body));
    }
}

/// Says hello to the Discord app on `pipe`: whether it took the application.
fn hello<P: Read + Write>(pipe: &mut P, application: &str) -> std::io::Result<bool> {
    pipe.write_all(&frame(HANDSHAKE, &format!(r#"{{"v":1,"client_id":"{}"}}"#, escaped(application))))?;
    let (op, body) = reply(pipe)?;
    Ok(op == FRAME && body.contains("READY"))
}

/// Sets the activity: whether Discord took it (an `ERROR` event: not).
fn set<P: Read + Write>(pipe: &mut P, json: &str, nonce: &str) -> std::io::Result<bool> {
    pipe.write_all(&frame(FRAME, json))?;
    loop {
        let (op, body) = reply(pipe)?;
        if op == CLOSE {
            return Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, body));
        }
        // Other events (none asked for) are let by.
        if body.contains(nonce) {
            return Ok(!body.contains(r#""evt":"ERROR""#));
        }
    }
}

/// Connects to the first Discord pipe that takes the application.
#[cfg(windows)]
fn connect(application: &str) -> Option<std::fs::File> {
    (0..10).find_map(|n| {
        let mut pipe = std::fs::OpenOptions::new().read(true).write(true).open(format!(r"\\.\pipe\discord-ipc-{n}")).ok()?;
        hello(&mut pipe, application).ok().filter(|took| *took).map(|_| pipe)
    })
}

/// Starts the presence for this process (`victory play`, after its settings are in the
/// environment): `victory discord --watch <pid>`, a process of its own with no window.
#[cfg(windows)]
pub(crate) fn start_for_this_run() {
    if std::env::var_os("VICTORY_DISCORD").is_some_and(|v| v == "0") {
        return;
    }
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    let Ok(program) = std::env::current_exe() else { return };
    let started = std::process::Command::new(program)
        .args(["discord", "--watch", &std::process::id().to_string()])
        .env_remove("VICTORY_LOG_FILE")
        .env("VICTORY_LOG", "error")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn();
    if started.is_ok() {
        println!("Discord: Playing Grand Theft Auto VI, with the cat =^.^= (VICTORY_DISCORD=0 turns it off).");
    }
}

/// `victory discord --watch <pid>`: the presence for as long as that process runs.
#[cfg(windows)]
pub(crate) fn run(watch: u32) -> anyhow::Result<()> {
    // The game gone, so is its presence (Discord also drops it with the process `pid`).
    std::thread::spawn(move || {
        victory_hle::save_share::wait_for_exit(watch);
        std::process::exit(0);
    });
    let application = application();
    let started = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let mut turn = started as usize / 60;
    let mut by_url = true;
    loop {
        if let Some(mut pipe) = connect(&application) {
            loop {
                let nonce = format!("victory-try-{started}-{turn}");
                let took = match set(&mut pipe, &activity(watch, started, turn, by_url, &nonce), &nonce) {
                    Ok(took) => took,
                    Err(_) => break,
                };
                if !took && by_url {
                    // The cover by URL refused: the application's own art from now on.
                    by_url = false;
                    continue;
                }
                turn += 1;
                std::thread::sleep(EVERY);
            }
        }
        std::thread::sleep(AGAIN);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pipe that answers from `input` and keeps what is written.
    struct Pipe {
        input: std::io::Cursor<Vec<u8>>,
        output: Vec<u8>,
    }

    impl Read for Pipe {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.input.read(buf)
        }
    }

    impl Write for Pipe {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.output.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn pipe(frames: &[(u32, &str)]) -> Pipe {
        Pipe { input: std::io::Cursor::new(frames.iter().flat_map(|(op, j)| frame(*op, j)).collect()), output: Vec::new() }
    }

    #[test]
    fn a_frame_is_the_operation_the_length_and_the_json() {
        let f = frame(1, "{}");
        assert_eq!(f, [1, 0, 0, 0, 2, 0, 0, 0, b'{', b'}']);
    }

    #[test]
    fn the_activity_is_what_discord_takes() {
        let json = activity(4242, 1_764_000_000, 3, true, "n-1");
        for part in [
            r#""cmd":"SET_ACTIVITY""#,
            r#""pid":4242"#,
            r#""start":1764000000"#,
            r#""large_text":"Grand Theft Auto VI""#,
            r#""label":"Play on PC too""#,
            RICKROLL_CAT,
            COVER,
            STATES[3],
            r#""nonce":"n-1""#,
        ] {
            assert!(json.contains(part), "{part} in {json}");
        }
        // The braces and brackets pair up, outside strings.
        let (mut depth, mut inside, mut escape) = (0i32, false, false);
        for c in json.chars() {
            match (inside, escape, c) {
                (true, true, _) => escape = false,
                (true, false, '\\') => escape = true,
                (_, false, '"') => inside = !inside,
                (false, _, '{' | '[') => depth += 1,
                (false, _, '}' | ']') => depth -= 1,
                _ => {}
            }
            assert!(depth >= 0, "{json}");
        }
        assert_eq!((depth, inside), (0, false), "{json}");
        let art = activity(1, 2, 0, false, "n");
        assert!(art.contains(r#""large_image":"logo""#) && !art.contains("small_image"), "{art}");
    }

    #[test]
    fn what_discord_shows_fits_its_limits() {
        for (label, url) in BUTTONS {
            assert!(!label.is_empty() && label.len() <= 32 && url.starts_with("https://"), "{label}");
        }
        for text in STATES.iter().chain(&[DETAILS]) {
            assert!((2..=128).contains(&text.len()), "{text}");
        }
        assert!(APPLICATION.bytes().all(|b| b.is_ascii_digit()));
    }

    #[test]
    fn json_strings_are_escaped() {
        assert_eq!(escaped(r#"a "cat" \ b"#), r#"a \"cat\" \\ b"#);
        assert_eq!(escaped("\n"), "\\u000a");
    }

    #[test]
    fn hello_wants_ready_and_a_ping_is_answered() {
        let mut p = pipe(&[(PING, "ping"), (FRAME, r#"{"cmd":"DISPATCH","evt":"READY"}"#)]);
        assert!(hello(&mut p, APPLICATION).unwrap());
        // The handshake, then the pong with the ping's body.
        let shake = frame(HANDSHAKE, &format!(r#"{{"v":1,"client_id":"{APPLICATION}"}}"#));
        assert_eq!(&p.output[..shake.len()], &shake[..]);
        assert_eq!(&p.output[shake.len()..], &frame(PONG, "ping")[..]);
        let mut refused = pipe(&[(CLOSE, r#"{"code":4000,"message":"Invalid Client ID"}"#)]);
        assert!(!hello(&mut refused, "1").unwrap());
        assert!(hello(&mut pipe(&[]), "1").is_err());
    }

    #[test]
    fn a_refused_activity_is_told_from_a_taken_one() {
        let taken = r#"{"cmd":"SET_ACTIVITY","data":{},"evt":null,"nonce":"n-7"}"#;
        let refused = r#"{"cmd":"SET_ACTIVITY","data":{"code":4000},"evt":"ERROR","nonce":"n-8"}"#;
        assert!(set(&mut pipe(&[(FRAME, "{\"evt\":\"OTHER\"}"), (FRAME, taken)]), "{}", "n-7").unwrap());
        assert!(!set(&mut pipe(&[(FRAME, refused)]), "{}", "n-8").unwrap());
        assert!(set(&mut pipe(&[(CLOSE, "{}")]), "{}", "n-9").is_err());
    }
}
