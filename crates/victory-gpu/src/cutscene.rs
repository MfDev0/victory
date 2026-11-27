//! Whether a cutscene is playing, from the title's own camera cuts (Try 6.25).
//!
//! The title tells its upscaler when the picture does not follow from the
//! last one (`reset` in its FidelityFX upscale description, which our
//! stand-in, the PC FidelityFX path and DLSS all read). A cutscene cuts every
//! few seconds; play almost never does. Several cuts close together start the
//! cutscene look, and a while without one ends it.
//!
//! What follows it: the stand-in for the indirect light (`crate::ambient`).
//! During play the world gets trustno1-coder/VIctory-NVIDIA's estimate from
//! the frame (with the air and reflections); in cutscenes the constant fill
//! light this fork has always had, which the owner found better there. A cut
//! also makes the estimate start again from that frame (no light of the shot
//! before carried over).
//!
//! `VICTORY_CUTSCENE_LOOK`:
//! - unset or `auto`: as above.
//! - `world`: the world's look always (VIctory-NVIDIA's).
//! - `ours`: this fork's look always (as before Try 6.25).
//!
//! Without the title's upscale (no DLSS, no upscaler path) no cuts are seen,
//! and the world's look stays on.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Cuts within this many seconds that start the cutscene look.
const CUTS: usize = 3;
const WITHIN_S: f32 = 20.0;
/// Seconds without a cut that end it.
const QUIET_S: f32 = 15.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    Auto,
    World,
    Ours,
}

fn parse(set: Option<&str>) -> Look {
    match set.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("world") => Look::World,
        Some("ours") | Some("always") => Look::Ours,
        _ => Look::Auto,
    }
}

pub fn look() -> Look {
    static LOOK: std::sync::OnceLock<Look> = std::sync::OnceLock::new();
    *LOOK.get_or_init(|| parse(std::env::var("VICTORY_CUTSCENE_LOOK").ok().as_deref()))
}

/// The cuts of the last seconds and whether the cutscene look is on, in
/// seconds since some start (pure, for tests).
#[derive(Default, Debug)]
struct Cuts {
    at: Vec<f32>,
    on: bool,
}

impl Cuts {
    /// A cut at `now`; whether the look changed.
    fn cut(&mut self, now: f32) -> bool {
        self.at.retain(|&t| now - t <= WITHIN_S);
        self.at.push(now);
        let was = self.on;
        if self.at.len() >= CUTS {
            self.on = true;
        }
        was != self.on
    }

    /// Whether the look is on at `now`; ends it after the quiet time.
    fn on(&mut self, now: f32) -> bool {
        if self.on && self.at.last().is_none_or(|&t| now - t > QUIET_S) {
            self.on = false;
            self.at.clear();
        }
        self.on
    }
}

struct State {
    start: Instant,
    cuts: Cuts,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// The frame (`crate::frame::current`) of the last cut, plus one (0: none).
static CUT_FRAME: AtomicU64 = AtomicU64::new(0);
static CUTS_SEEN: AtomicU64 = AtomicU64::new(0);
static UPSCALES: AtomicU64 = AtomicU64::new(0);
/// The frame (plus one) of the last upscale noted: the hooks that can see
/// one dispatch (`victory-hle`'s DLSS interposer, then the FidelityFX that
/// answers what it leaves) note it once.
static NOTED_FRAME: AtomicU64 = AtomicU64::new(0);
/// Whether a reset was noted in that frame.
static NOTED_RESET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn with<R>(f: impl FnOnce(&mut Cuts, f32) -> R) -> R {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = state.get_or_insert_with(|| State { start: Instant::now(), cuts: Cuts::default() });
    let now = s.start.elapsed().as_secs_f32();
    f(&mut s.cuts, now)
}

/// One upscale of the title's; `reset`: the title said the picture does not
/// follow from the last one.
pub fn upscaled(reset: bool) {
    let frame = crate::frame::current() + 1;
    if NOTED_FRAME.swap(frame, Ordering::Relaxed) == frame {
        // The same upscale again (another hook), or a second one in the
        // frame: a cut counts once a frame.
        if !reset || NOTED_RESET.swap(true, Ordering::Relaxed) {
            return;
        }
    } else {
        UPSCALES.fetch_add(1, Ordering::Relaxed);
        NOTED_RESET.store(reset, Ordering::Relaxed);
        if !reset {
            return;
        }
    }
    CUTS_SEEN.fetch_add(1, Ordering::Relaxed);
    CUT_FRAME.store(frame, Ordering::Relaxed);
    let changed = with(|c, now| c.cut(now));
    if changed && look() == Look::Auto {
        tracing::info!(
            "cutscene look: on (camera cuts close together; this fork's fill light, VICTORY_CUTSCENE_LOOK=world keeps the world's)"
        );
    }
}

/// Whether the cutscene look is on now.
pub fn now() -> bool {
    match look() {
        Look::World => false,
        Look::Ours => true,
        Look::Auto => {
            let (was, is) = with(|c, now| {
                let was = c.on;
                (was, c.on(now))
            });
            if was && !is {
                tracing::info!("cutscene look: off (no camera cut for {QUIET_S:.0} s: the world's light estimate)");
            }
            is
        }
    }
}

/// Whether the title cut the camera in `frame`.
pub fn cut_in(frame: u64) -> bool {
    CUT_FRAME.load(Ordering::Relaxed) == frame + 1
}

/// Upscales and cuts seen so far, for the log.
pub fn seen() -> (u64, u64) {
    (UPSCALES.load(Ordering::Relaxed), CUTS_SEEN.load(Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switches() {
        assert_eq!(parse(None), Look::Auto);
        assert_eq!(parse(Some("auto")), Look::Auto);
        assert_eq!(parse(Some(" World ")), Look::World);
        assert_eq!(parse(Some("ours")), Look::Ours);
        assert_eq!(parse(Some("x")), Look::Auto);
    }

    #[test]
    fn cuts_close_together_start_it_and_quiet_ends_it() {
        let mut c = Cuts::default();
        // Play: a cut now and then (a pause menu, a teleport) is not one.
        assert!(!c.cut(0.0));
        assert!(!c.cut(30.0));
        assert!(!c.cut(55.0));
        assert!(!c.on(56.0));
        // A cutscene: cuts every few seconds.
        assert!(!c.cut(100.0));
        assert!(!c.cut(104.0));
        assert!(c.cut(109.0));
        assert!(c.on(110.0));
        // A long shot keeps it.
        assert!(!c.cut(122.0));
        assert!(c.on(136.0));
        // Play again.
        assert!(!c.on(137.5));
        assert!(!c.on(200.0));
        // And a new cutscene needs its own cuts.
        assert!(!c.cut(201.0));
        assert!(!c.on(202.0));
    }
}
