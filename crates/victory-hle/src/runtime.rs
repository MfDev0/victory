//! Process-wide state the implementations need.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

static MAIN_IMAGE_BASE: AtomicUsize = AtomicUsize::new(0);

/// Records where the title's main image is mapped. The title expects
/// `GetModuleHandle(NULL)` and friends to return this, not our own exe.
pub fn set_main_image(base: usize) {
    MAIN_IMAGE_BASE.store(base, Ordering::Release);
}

pub fn main_image() -> Option<usize> {
    match MAIN_IMAGE_BASE.load(Ordering::Acquire) {
        0 => None,
        base => Some(base),
    }
}

/// Original file of each title image we mapped, by base address.
static IMAGE_PATHS: Mutex<Vec<(usize, PathBuf)>> = Mutex::new(Vec::new());

/// Records the file a title image was mapped from: the title asks for its
/// own path (`GetModuleFileName`) to find its data, and must not get our
/// cache copy.
pub fn set_image_path(base: usize, path: &Path) {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_owned());
    let mut paths = IMAGE_PATHS.lock().unwrap_or_else(|e| e.into_inner());
    paths.retain(|(b, _)| *b != base);
    paths.push((base, path));
}

/// Forgets a title image that was unmapped again.
pub fn forget_image(base: usize) {
    let mut paths = IMAGE_PATHS.lock().unwrap_or_else(|e| e.into_inner());
    paths.retain(|(b, _)| *b != base);
}

pub fn image_path(base: usize) -> Option<PathBuf> {
    let paths = IMAGE_PATHS.lock().unwrap_or_else(|e| e.into_inner());
    paths
        .iter()
        .find(|(b, _)| *b == base)
        .map(|(_, p)| p.clone())
}

/// The title image whose original file is called `file` (any case).
pub fn image_named(file: &str) -> Option<usize> {
    let paths = IMAGE_PATHS.lock().unwrap_or_else(|e| e.into_inner());
    paths
        .iter()
        .find(|(_, p)| p.file_name().is_some_and(|n| n.eq_ignore_ascii_case(file)))
        .map(|(b, _)| *b)
}

/// Maps a title DLL the title loads itself (`LoadLibrary`) and returns its
/// base; `Ok(None)` if `file` is not one of the title's DLLs.
pub type TitleDllLoader = Box<dyn Fn(&str) -> Result<Option<usize>, String> + Send + Sync>;

static TITLE_DLL_LOADER: std::sync::OnceLock<TitleDllLoader> = std::sync::OnceLock::new();

/// Set once, by whoever mapped the title (it knows the folders and the
/// resolver); later calls are ignored.
pub fn set_title_dll_loader(loader: TitleDllLoader) {
    let _ = TITLE_DLL_LOADER.set(loader);
}

pub fn load_title_dll(file: &str) -> Result<Option<usize>, String> {
    TITLE_DLL_LOADER.get().map_or(Ok(None), |load| load(file))
}

static TITLE_DLL_SETTLE: std::sync::OnceLock<Box<dyn Fn() + Send + Sync>> = std::sync::OnceLock::new();

/// Set once, by whoever maps the title's DLLs: waits until no thread is
/// mapping, binding or initialising one (the loader lock's shape).
pub fn set_title_dll_settle(settle: Box<dyn Fn() + Send + Sync>) {
    let _ = TITLE_DLL_SETTLE.set(settle);
}

/// Waits for a title DLL mapping in progress on another thread (Try 6.7:
/// a `GetModuleHandle` racing the title's load of its upscaler was refused
/// as a host DLL).
pub fn settle_title_dlls() {
    if let Some(settle) = TITLE_DLL_SETTLE.get() {
        settle();
    }
}

/// The file names of the title images whose names start as `file`'s does,
/// for the log of a refused name.
pub fn images_like(file: &str) -> Vec<String> {
    let stem: String = file.chars().take(8).collect::<String>().to_ascii_lowercase();
    let paths = IMAGE_PATHS.lock().unwrap_or_else(|e| e.into_inner());
    paths
        .iter()
        .filter_map(|(_, p)| p.file_name()?.to_str().map(str::to_owned))
        .filter(|n| n.to_ascii_lowercase().starts_with(&stem))
        .collect()
}

/// What the title's manifest (`MicrosoftGame.config` beside its executable)
/// says about it, and where its persistent local storage lives.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Title {
    pub id: u32,
    /// The store's product id (empty if the manifest has none).
    pub store_id: String,
    /// Persistent local storage: our folder for it and the title's quota.
    /// Game saves live beside it ([`Title::saves`]).
    pub storage: PathBuf,
    pub storage_bytes: u64,
}

impl Title {
    /// Id, store id and storage quota from the manifest (zero or empty
    /// where it has none); the storage folder is left for the caller.
    pub fn from_manifest(text: &str) -> Title {
        let elements = elements(text);
        let get = |path: &str| {
            elements
                .iter()
                .find(|(p, _)| p == path)
                .map(|(_, text)| text.as_str())
        };
        let id = get("Game/TitleId").and_then(|v| u32::from_str_radix(v, 16).ok());
        let storage_mb = get("Game/PersistentLocalStorage/SizeMB").and_then(|v| v.parse().ok());
        Title {
            id: id.unwrap_or(0),
            store_id: get("Game/StoreId").unwrap_or_default().to_owned(),
            storage: PathBuf::new(),
            storage_bytes: storage_mb.unwrap_or(0) << 20,
        }
    }

    /// Where game saves live: next to persistent local storage, under the
    /// data directory, never the cache.
    pub fn saves(&self) -> PathBuf {
        self.storage.with_file_name("saves")
    }
}

/// Each element of an XML text as its path from the root (`Game/TitleId`)
/// and its trimmed text up to the first child, the predefined entities
/// decoded; comments, CDATA sections, declarations and attributes are
/// skipped.
fn elements(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut path: Vec<&str> = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        rest = &rest[open..];
        let end = if rest.starts_with("<!--") {
            rest.find("-->").map(|e| e + 3)
        } else if rest.starts_with("<![CDATA[") {
            rest.find("]]>").map(|e| e + 3)
        } else {
            rest.find('>').map(|e| e + 1)
        };
        let Some(end) = end else { break };
        let tag = &rest[1..end - 1];
        rest = &rest[end..];
        if tag.starts_with(['!', '?']) {
            continue;
        }
        if let Some(name) = tag.strip_prefix('/') {
            if let Some(at) = path.iter().rposition(|n| *n == name.trim()) {
                path.truncate(at);
            }
            continue;
        }
        let closed = tag.ends_with('/');
        let name = tag
            .trim_end_matches('/')
            .split_whitespace()
            .next()
            .unwrap_or("");
        path.push(name);
        let body = rest[..rest.find('<').unwrap_or(rest.len())].trim();
        let body = [("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&apos;", "'"), ("&amp;", "&")]
            .iter()
            .fold(body.to_owned(), |b, (entity, c)| b.replace(entity, c));
        out.push((path.join("/"), body));
        if closed {
            path.pop();
        }
    }
    out
}

static TITLE: Mutex<Option<Title>> = Mutex::new(None);

pub fn set_title(title: Title) {
    *TITLE.lock().unwrap_or_else(|e| e.into_inner()) = Some(title);
}

/// The running title; the default (id 0, no storage) until set.
pub fn title() -> Title {
    TITLE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default()
}

/// Gives the process the console's millisecond timers. Titles pace
/// themselves with short waits (the title's audio mixer sleeps
/// about 4 ms per 5.3 ms block) and cannot tune the timer on the console;
/// a desktop process waits in 15.6 ms ticks unless it asks for finer ones,
/// and Windows 11 drops that request while the window is hidden unless the
/// process opts out. Process-wide, for the whole run.
#[cfg(windows)]
pub fn use_console_timers() {
    use windows_sys::Win32::Media::timeBeginPeriod;
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION, PROCESS_POWER_THROTTLING_STATE,
        ProcessPowerThrottling, SetProcessInformation,
    };
    // Also: the system does not run this process at reduced speed (its
    // "efficiency mode": lower clock, the slow cores of a hybrid processor)
    // when its window is not the one in front, as while its owner works in
    // another window or streams. The console has eight cores alike and no
    // such mode. `VICTORY_EFFICIENCY_MODE=1` leaves the system its choice.
    let execution = if std::env::var_os("VICTORY_EFFICIENCY_MODE").is_some_and(|v| v == "1") {
        0
    } else {
        PROCESS_POWER_THROTTLING_EXECUTION_SPEED
    };
    let keep = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION | execution,
        StateMask: 0,
    };
    // SAFETY: plain process settings; `keep` outlives the call.
    unsafe {
        SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            (&raw const keep).cast(),
            size_of_val(&keep) as u32,
        );
        timeBeginPeriod(1);
    }
    use_whole_machine();
}

#[cfg(windows)]
fn whole_machine() -> bool {
    std::env::var_os("VICTORY_WHOLE_MACHINE").is_none_or(|v| v != "0")
}

/// Try 6.0: asks Windows for the whole machine while the title runs.
/// `VICTORY_WHOLE_MACHINE=0`: none of it.
///
/// - The process runs above normal priority: the title's threads and ours
///   come before the browser, the launcher and updaters on a busy CPU.
/// - Windows is told not to throttle its execution speed (no EcoQoS): on
///   CPUs with performance and efficiency cores (Intel 12th generation and
///   later) a thread Windows judges to be in the background is put on the
///   slow cores, and our helper threads that wait a lot looked like that.
/// - A soft minimum working set of a quarter of the PC's memory (at most
///   4 GiB): on a PC whose memory is nearly all in use (under 1 GB free in
///   the 2026-11-26 report) Windows takes pages away from the biggest
///   process first, and the title's memory then came back from the page
///   file a page at a time. Soft: Windows still takes them if it must.
#[cfg(windows)]
fn use_whole_machine() {
    use windows_sys::Win32::System::Memory::{
        QUOTA_LIMITS_HARDWS_MAX_DISABLE, QUOTA_LIMITS_HARDWS_MIN_DISABLE, SetProcessWorkingSetSizeEx,
    };
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows_sys::Win32::System::Threading::{
        ABOVE_NORMAL_PRIORITY_CLASS, GetCurrentProcess, HIGH_PRIORITY_CLASS, SetPriorityClass,
    };
    if !whole_machine() {
        return;
    }
    // Try 6.12 (all-out mode): `VICTORY_PRIORITY=high` runs the process at
    // high priority (never realtime: the mouse, the keyboard and the driver
    // still come first), and `VICTORY_WORKING_SET_PERCENT` sets the soft
    // minimum working set as a share of the PC's memory (1 to 95, no 4 GiB
    // cap) instead of a quarter.
    let high = std::env::var("VICTORY_PRIORITY").is_ok_and(|v| v.trim().eq_ignore_ascii_case("high"));
    let percent = std::env::var("VICTORY_WORKING_SET_PERCENT")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|p| p.clamp(1, 95));
    // SAFETY: plain process settings and a query that fills our structure.
    unsafe {
        let process = GetCurrentProcess();
        let class = if high { HIGH_PRIORITY_CLASS } else { ABOVE_NORMAL_PRIORITY_CLASS };
        let priority = SetPriorityClass(process, class) != 0;
        let mut status: MEMORYSTATUSEX = std::mem::zeroed();
        status.dwLength = size_of_val(&status) as u32;
        let mut kept_mb = 0;
        if GlobalMemoryStatusEx(&mut status) != 0 {
            let min = match percent {
                Some(p) => (status.ullTotalPhys / 100 * p) as usize,
                None => (status.ullTotalPhys / 4).min(4 << 30) as usize,
            };
            // The maximum only has to be above the minimum: it is not enforced.
            let max = min.saturating_mul(4);
            if SetProcessWorkingSetSizeEx(
                process,
                min,
                max,
                QUOTA_LIMITS_HARDWS_MIN_DISABLE | QUOTA_LIMITS_HARDWS_MAX_DISABLE,
            ) != 0
            {
                kept_mb = min >> 20;
            }
        }
        tracing::info!(
            above_normal = priority && !high,
            high = priority && high,
            soft_min_working_set_mb = kept_mb,
            pc_memory_mb = status.ullTotalPhys >> 20,
            pc_memory_free_mb = status.ullAvailPhys >> 20,
            "the whole machine for the title (VICTORY_WHOLE_MACHINE=0: none of it)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_title_id_store_id_and_storage_quota() {
        let text = "\u{feff}<?xml version=\"1.0\"?>\n<Game configVersion=\"1\">\
                    <!-- <TitleId>00000000</TitleId> -->\
                    <Identity Name=\"x\"/>\n  <TitleId>639AA8DD</TitleId>\n\
                    <![CDATA[ <TitleId>1</TitleId> ]]>                    <StoreId>9&amp;C</StoreId><Other><SizeMB>7</SizeMB></Other>\
                    <PersistentLocalStorage>\n  <SizeMB>1024</SizeMB> <!-- c -->\n\
                    </PersistentLocalStorage></Game>";
        let t = Title::from_manifest(text);
        assert_eq!((t.id, t.storage_bytes), (0x639A_A8DD, 1024 << 20));
        assert_eq!(t.store_id, "9&C");
        assert_eq!(Title::from_manifest("").storage_bytes, 0);
    }
}
