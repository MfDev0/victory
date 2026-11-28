//! `victory report`: one zip with what a tester sends after a run. That is the
//! newest logs, the crash log, the F1 menu's settings, MFG Unlocker's own
//! settings and logs (FG UNLOCK), and a short system and summary page. It
//! goes into `%USERPROFILE%\Downloads\VIctory-reports`, or `reports` under
//! `%LOCALAPPDATA%\victory` where there is no Downloads folder, and the folder
//! is opened with the zip picked.
//!
//! `victory play` starts `victory report --watch <its pid>` beside itself, as a
//! process of its own and with no window. The watcher waits for the game to
//! end. If it ended with an exit code other than 0 (a crash, a lost device), it
//! makes the zip by itself. A restart from F1, the warm-up and a normal quit
//! end with 0. `VICTORY_CRASH_REPORT=0` turns the watcher off. Nothing here
//! runs in the game's process, so a report never takes the game down.
//!
//! Every step is best effort. A file that cannot be read is named in
//! `summary.txt` and the rest is still packed.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};

/// The most of one log that is packed: its end (a debug log runs to gigabytes).
const LOG_TAIL: u64 = 24 << 20;
/// The most of any other file packed.
const FILE_MOST: u64 = 4 << 20;
/// Newest logs packed.
const LOGS: usize = 3;
/// Exit codes a watched run ends with that are not a failure: 0, and a closed
/// console window (`STATUS_CONTROL_C_EXIT`).
const NOT_A_CRASH: [u32; 2] = [0, 0xC000_013A];

/// `victory report`: the zip made now, the folder opened with it picked.
pub(crate) fn run(watch: Option<u32>, open: bool) -> Result<()> {
    if let Some(pid) = watch {
        let Some(code) = victory_hle::save_share::wait_for_exit(pid) else { return Ok(()) };
        if NOT_A_CRASH.contains(&code) {
            return Ok(());
        }
        let zip = make(Some(code))?;
        open_folder(&zip);
        return Ok(());
    }
    let zip = make(None)?;
    println!("Report: {}", zip.display());
    println!("Send that zip with what you saw (scene, settings, what went wrong).");
    if open {
        open_folder(&zip);
    }
    Ok(())
}

/// Starts the crash watcher for this process (`victory play`, after its settings are in the
/// environment). Any failure only means no automatic report.
pub(crate) fn watch_this_run() {
    if std::env::var_os("VICTORY_CRASH_REPORT").is_some_and(|v| v == "0") {
        return;
    }
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    let Ok(program) = std::env::current_exe() else { return };
    let started = std::process::Command::new(program)
        .args(["report", "--watch", &std::process::id().to_string()])
        .env_remove("VICTORY_LOG_FILE")
        .env("VICTORY_LOG", "error")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn();
    if started.is_ok() {
        println!("Crash report: on (a zip goes to Downloads\\VIctory-reports if the game fails; VICTORY_CRASH_REPORT=0 turns it off).");
    }
}

/// `%LOCALAPPDATA%\victory`, or the folder `VICTORY_HOME` names (as `victory play` keeps it).
fn home() -> Option<PathBuf> {
    victory_core::config::home_override()
        .or_else(|| std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()).map(|b| PathBuf::from(b).join("victory")))
}

/// Where reports go: `Downloads\VIctory-reports`, else `reports` in the home folder.
fn out_dir() -> Result<PathBuf> {
    let downloads = std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()).map(|p| PathBuf::from(p).join("Downloads"));
    if let Some(d) = downloads.filter(|d| d.is_dir()) {
        let dir = d.join("VIctory-reports");
        if std::fs::create_dir_all(&dir).is_ok() {
            return Ok(dir);
        }
    }
    let dir = home().context("no %LOCALAPPDATA% or %USERPROFILE% to put the report in")?.join("reports");
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    Ok(dir)
}

fn modified(p: &Path) -> SystemTime {
    std::fs::metadata(p).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH)
}

/// The files in `dir` that `keep` accepts, newest first.
fn newest(dir: &Path, keep: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.file_name().is_some_and(|n| keep(&n.to_string_lossy())))
        .collect();
    found.sort_by_key(|p| std::cmp::Reverse(modified(p)));
    found
}

/// At most `most` bytes of `path`, from its end; with a note where it was cut.
fn tail(path: &Path, most: u64) -> std::io::Result<Vec<u8>> {
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let mut out = Vec::new();
    if len > most {
        f.seek(SeekFrom::Start(len - most))?;
        out.extend_from_slice(format!("[victory report: the first {} bytes of {len} left out]\r\n", len - most).as_bytes());
    }
    f.take(most).read_to_end(&mut out)?;
    Ok(out)
}

/// Every `crashes.log` under `data` (`<title id>\crashes.log`), newest first.
fn crash_logs(data: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = walkdir::WalkDir::new(data)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.file_name().eq_ignore_ascii_case("crashes.log"))
        .map(walkdir::DirEntry::into_path)
        .collect();
    found.sort_by_key(|p| std::cmp::Reverse(modified(p)));
    found
}

/// A command's output, for the system page (empty where it did not run).
fn output(program: &str, args: &[&str]) -> String {
    std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default()
}

fn system_page(exit: Option<u32>) -> String {
    let mut t = String::new();
    let mut line = |k: &str, v: &str| t.push_str(&format!("{k}: {v}\r\n"));
    line("victory", env!("CARGO_PKG_VERSION"));
    if let Ok(exe) = std::env::current_exe() {
        line("program", &exe.to_string_lossy());
        if let Ok(bytes) = std::fs::read(&exe) {
            line("program blake3", &blake3::hash(&bytes).to_hex()[..16]);
        }
    }
    if let Some(code) = exit {
        line("game exit code", &format!("{code:#x} ({code})"));
    }
    line("windows", &output("cmd", &["/c", "ver"]));
    line("gpu (the one the game runs on)", &victory_hle::streamline::adapter_name().unwrap_or_default());
    line("logical cpus", &std::thread::available_parallelism().map_or(0, |n| n.get()).to_string());
    let ps = "$c=Get-CimInstance Win32_ComputerSystem; $p=Get-CimInstance Win32_Processor | Select-Object -First 1; \
              'ram gb: ' + [math]::Round($c.TotalPhysicalMemory/1GB,1); 'cpu: ' + $p.Name; \
              Get-CimInstance Win32_VideoController | ForEach-Object { 'video: ' + $_.Name + ' driver ' + $_.DriverVersion }; \
              $h=(Get-ItemProperty 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers' -ErrorAction SilentlyContinue).HwSchMode; \
              'hags (2 = on): ' + $h";
    t.push_str(&output("powershell.exe", &["-NoProfile", "-NonInteractive", "-Command", ps]).replace('\n', "\r\n"));
    t.push_str("\r\n");
    t
}

/// The short page read first: the run's switches, its errors and warnings
/// (each once, with how often), and the lines about frame generation, the
/// unlocker, faces and crashes.
pub(crate) fn summary_of(log: &str) -> String {
    let mut out = String::new();
    if let Some(first) = log.lines().find(|l| l.starts_with("# console=")) {
        out.push_str("== start ==\r\n");
        out.push_str(first);
        out.push_str("\r\n\r\n");
    }
    let mut counts: Vec<(String, usize)> = Vec::new();
    let mut picked = Vec::new();
    for l in log.lines() {
        let lower = l.to_ascii_lowercase();
        if l.contains(" ERROR ") || l.contains(" WARN ") {
            // The message without its time: the same warning at other times counts once.
            let key: String = l.split_once(" WARN ").or_else(|| l.split_once(" ERROR ")).map_or(l, |(_, rest)| rest).chars().take(200).collect();
            let kind = if l.contains(" ERROR ") { "ERROR " } else { "WARN  " };
            let key = format!("{kind}{key}");
            if let Some(at) = counts.iter().position(|(k, _)| *k == key) {
                counts[at].1 += 1;
            } else if counts.len() < 400 {
                counts.push((key, 1));
            }
        }
        const NOTABLE: [&str; 9] =
            ["frame generation", "mfg unlocker", "fg unlock", "panic", "crash", "exception", "device removed", "out of memory", "dlss"];
        if picked.len() < 200 && NOTABLE.iter().any(|w| lower.contains(w)) {
            picked.push(l.chars().take(300).collect::<String>());
        }
    }
    out.push_str("== errors and warnings (times seen) ==\r\n");
    if counts.is_empty() {
        out.push_str("none\r\n");
    }
    for (k, n) in &counts {
        out.push_str(&format!("{n:>6}  {k}\r\n"));
    }
    out.push_str("\r\n== frame generation, DLSS, unlocker, crashes ==\r\n");
    for l in &picked {
        out.push_str(l);
        out.push_str("\r\n");
    }
    out
}

/// Makes the zip; its path. `exit`: the watched run's exit code.
fn make(exit: Option<u32>) -> Result<PathBuf> {
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut notes = String::new();
    let mut add = |entries: &mut Vec<(String, Vec<u8>)>, name: String, path: &Path, most: u64| match tail(path, most) {
        Ok(bytes) => entries.push((name, bytes)),
        Err(e) => notes.push_str(&format!("not packed: {} ({e})\r\n", path.display())),
    };
    let home = home();
    let mut newest_log = None;
    if let Some(home) = &home {
        let logs = newest(&home.join("logs"), |n| n.ends_with(".log"));
        newest_log = logs.first().cloned();
        for log in logs.iter().take(LOGS) {
            let name = format!("logs/{}", log.file_name().unwrap_or_default().to_string_lossy());
            add(&mut entries, name, log, LOG_TAIL);
        }
        let settings = home.join("victory-settings.txt");
        if settings.is_file() {
            add(&mut entries, "victory-settings.txt".to_owned(), &settings, FILE_MOST);
        }
        let data = std::env::var_os("VICTORY_DATA_DIR").filter(|v| !v.is_empty()).map_or_else(|| home.join("data"), PathBuf::from);
        if let Some(crashes) = crash_logs(&data).first() {
            add(&mut entries, "crashes.log".to_owned(), crashes, FILE_MOST);
        }
    }
    // MFG Unlocker's own settings and logs beside victory.exe (FG UNLOCK).
    if let Some(here) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
        for f in newest(&here, |n| n.to_ascii_uppercase().starts_with("RTXMFG")).iter().take(8) {
            let name = format!("mfg-unlocker/{}", f.file_name().unwrap_or_default().to_string_lossy());
            add(&mut entries, name, f, FILE_MOST);
        }
    }
    let summary = match &newest_log {
        Some(log) => {
            let text = tail(log, LOG_TAIL).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
            format!("log: {}\r\n\r\n{}", log.display(), summary_of(&text))
        }
        None => "no log found under the home folder's logs (was the game started with victory play?)\r\n".to_owned(),
    };
    entries.push(("summary.txt".to_owned(), format!("{notes}{summary}").into_bytes()));
    entries.push(("system.txt".to_owned(), system_page(exit).into_bytes()));
    let packed = victory_hle::save_share::zip_of(&entries).context("packing the report")?;
    let kind = if exit.is_some() { "crash" } else { "report" };
    let zip = out_dir()?.join(format!("VIctory-{kind}-{}.zip", victory_hle::save_share::stamp()));
    std::fs::write(&zip, packed).with_context(|| format!("writing {}", zip.display()))?;
    Ok(zip)
}

/// Opens Explorer on the zip's folder with the zip picked.
fn open_folder(zip: &Path) {
    let _ = std::process::Command::new("explorer.exe").arg(format!("/select,{}", zip.display())).spawn();
}

#[cfg(test)]
mod tests {
    use super::summary_of;

    #[test]
    fn the_summary_counts_each_warning_once() {
        let log = "# console=series-x build=release switches: VICTORY_DLSS=1\n\
                   2026-11-28T10:00:00Z  WARN victory: a thing went odd\n\
                   2026-11-28T10:00:01Z  WARN victory: a thing went odd\n\
                   2026-11-28T10:00:02Z ERROR streamline: NGX evaluate feature failed\n\
                   2026-11-28T10:00:03Z  INFO victory_gpu: DLSS Frame Generation switched on=true times=4\n";
        let s = summary_of(log);
        assert!(s.contains("# console=series-x"));
        assert!(s.contains("     2  WARN  victory: a thing went odd"), "{s}");
        assert!(s.contains("     1  ERROR streamline: NGX evaluate feature failed"), "{s}");
        assert!(s.contains("DLSS Frame Generation switched on=true times=4"));
    }
}
