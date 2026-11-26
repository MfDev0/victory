//! How short the PC and the card are of memory, for the work that can wait.
//!
//! Our take on what codec_d's cool_fox (DoubleGhosty/cool_fox ab75ac3e,
//! f4b48ad) does with its residency guard and pool back-off, for this
//! fork, which has neither its block pool nor its residency module:
//!
//! - three levels ([`Level`]) from what Windows has left to commit, the free
//!   physical memory and the card's budget, looked at most every 32 ms; a
//!   refused allocation makes it [`Level::Critical`] for two seconds
//!   (cool_fox's `critical_commit`, apart from mild pressure, so that mild
//!   pressure no longer starves textures);
//! - after a refused upload buffer, uploads that can wait (texture staging)
//!   are not asked of the driver for half a second ([`backing_off`]): the
//!   driver refused one, and asking again for every texture is a burst of
//!   failing calls, felt as a stutter (cool_fox: eight refused block
//!   creations in one burst);
//! - the sweeps let go of idle memory sooner as the level rises, the texture
//!   pass takes fewer bytes a frame, and kept upload buffers are fewer.
//!
//! `VICTORY_PRESSURE=0`: always [`Level::Normal`], and no back-off.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// How short of memory the host is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Level {
    /// Room: everything as it was.
    Normal = 0,
    /// Getting short: idle memory goes sooner, fewer upload buffers kept.
    Mild = 1,
    /// Nearly out, or a refusal just now: no texture catch-up, idle memory
    /// goes after a second, nothing kept.
    Critical = 2,
}

const MIB: u64 = 1 << 20;
const GIB: u64 = 1 << 30;

/// How long after a refused allocation the level stays critical.
const REFUSED_CRITICAL_MS: u64 = 2000;
/// How long uploads that can wait are not asked for after a refusal.
const BACK_OFF_MS: u64 = 500;

/// The level from the figures (pure, for the tests): bytes Windows has
/// left to commit, free physical bytes, the card's budget and use, and how
/// long ago an allocation was refused.
pub(crate) fn classify(
    commit_left: Option<u64>,
    ram_left: Option<u64>,
    card: Option<(u64, u64)>,
    refused_ago_ms: Option<u64>,
) -> Level {
    let card_pct = card
        .filter(|&(budget, _)| budget > 0)
        .map(|(budget, used)| (u128::from(used) * 100 / u128::from(budget)) as u64);
    if refused_ago_ms.is_some_and(|ago| ago < REFUSED_CRITICAL_MS)
        || commit_left.is_some_and(|c| c < 1536 * MIB)
        || ram_left.is_some_and(|r| r < GIB)
        || card_pct.is_some_and(|p| p >= 97)
    {
        Level::Critical
    } else if commit_left.is_some_and(|c| c < 4 * GIB)
        || ram_left.is_some_and(|r| r < 3 * GIB)
        || card_pct.is_some_and(|p| p >= 85)
    {
        Level::Mild
    } else {
        Level::Normal
    }
}

fn on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_PRESSURE").is_none_or(|v| v != "0"))
}

/// The level now (looked at again at most every 32 ms).
static LEVEL: AtomicU64 = AtomicU64::new(0);
static LOOKED: AtomicU64 = AtomicU64::new(0);
/// When an allocation was last refused (`crate::host::now_ms` + 1; 0: never).
static REFUSED_AT: AtomicU64 = AtomicU64::new(0);
/// Since the start: refused allocations; uploads not asked for while
/// backing off; looks at each level.
pub(crate) static COUNTS: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];
/// At the last look: MiB left to commit, MiB of free physical memory, the
/// card's budget and use in MiB.
static FIGURES: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];

fn refused_ago(now: u64) -> Option<u64> {
    match REFUSED_AT.load(Relaxed) {
        0 => None,
        at => Some(now.saturating_sub(at - 1)),
    }
}

/// How short of memory the host is now.
pub(crate) fn level() -> Level {
    if !on() {
        return Level::Normal;
    }
    let now = crate::host::now_ms();
    let looked = LOOKED.load(Relaxed);
    if (looked == 0 || now >= looked + 32) && LOOKED.compare_exchange(looked, now.max(1), Relaxed, Relaxed).is_ok() {
        look(now);
    }
    match LEVEL.load(Relaxed) {
        0 => Level::Normal,
        1 => Level::Mild,
        _ => Level::Critical,
    }
}

fn look(now: u64) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: plain data the call fills in.
    let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    // SAFETY: a sized structure.
    let ok = unsafe { GlobalMemoryStatusEx(&mut m) } != 0;
    // (ullAvailPageFile: what the process may still commit, the limit an
    // upload buffer or a staging buffer runs into first)
    let (commit, ram) = if ok { (Some(m.ullAvailPageFile), Some(m.ullAvailPhys)) } else { (None, None) };
    let card = crate::host::host()
        .and_then(|h| h.video_memory())
        .map(|[budget, used, ..]| (budget, used));
    let level = classify(commit, ram, card, refused_ago(now));
    LEVEL.store(level as u64, Relaxed);
    COUNTS[2 + level as usize].fetch_add(1, Relaxed);
    FIGURES[0].store(commit.unwrap_or(0) / MIB, Relaxed);
    FIGURES[1].store(ram.unwrap_or(0) / MIB, Relaxed);
    FIGURES[2].store(card.map_or(0, |c| c.0) / MIB, Relaxed);
    FIGURES[3].store(card.map_or(0, |c| c.1) / MIB, Relaxed);
}

/// The driver refused an allocation: critical for a while, uploads that
/// can wait back off, and the next sweep comes at once.
pub(crate) fn refused() {
    if !on() {
        return;
    }
    let now = crate::host::now_ms();
    let first = refused_ago(now).is_none_or(|ago| ago >= BACK_OFF_MS);
    REFUSED_AT.store(now + 1, Relaxed);
    COUNTS[0].fetch_add(1, Relaxed);
    LEVEL.store(Level::Critical as u64, Relaxed);
    crate::host::sweep_soon();
    if first {
        crate::host::trail(|| "an allocation was refused: uploads that can wait back off for 500 ms".to_owned());
    }
}

/// Whether uploads that can wait are not to be asked of the driver now
/// (half a second after a refusal); counts the one not asked.
pub(crate) fn backing_off() -> bool {
    if !on() {
        return false;
    }
    let off = refused_ago(crate::host::now_ms()).is_some_and(|ago| ago < BACK_OFF_MS);
    if off {
        COUNTS[1].fetch_add(1, Relaxed);
    }
    off
}

/// The longest a sweep keeps idle memory at this level (`u64::MAX`: as the
/// sweep itself decides), for memory in the PC (`card` false) or the card.
pub(crate) fn idle_cap_ms(card: bool) -> u64 {
    match (level(), card) {
        (Level::Normal, _) => u64::MAX,
        (Level::Mild, false) => 5_000,
        (Level::Mild, true) => 2_500,
        (Level::Critical, _) => 1_000,
    }
}

/// The share of its bytes a frame's texture pass takes at this level, in
/// quarters, and whether a backlog may take more (mild pressure leaves the
/// textures alone: cool_fox found that starving them on mild pressure left
/// surfaces black).
pub(crate) fn texture_pass_quarters() -> (u64, bool) {
    match level() {
        Level::Normal => (4, true),
        Level::Mild => (4, true),
        // (Not halved: on 6 and 8 GB cards the waiting textures were the
        // black roads, buildings and people; only the catch-up goes.)
        Level::Critical => (4, false),
    }
}

/// For the frame report: one line of what the levels did.
pub(crate) fn report() -> String {
    let c = |i: usize| COUNTS[i].load(Relaxed);
    let f = |i: usize| FIGURES[i].load(Relaxed);
    format!(
        "memory pressure: {:?} now ({} MB left to commit, {} MB free, card {} of {} MB); looks normal {} mild {} critical {}; \
         allocations refused {}, uploads held back after a refusal {}",
        level(),
        f(0),
        f(1),
        f(3),
        f(2),
        c(2),
        c(3),
        c(4),
        c(0),
        c(1)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_commit_memory_card_and_refusals() {
        let plenty = (Some(20 * GIB), Some(16 * GIB), Some((8 * GIB, 4 * GIB)), None);
        assert_eq!(classify(plenty.0, plenty.1, plenty.2, plenty.3), Level::Normal);
        assert_eq!(classify(None, None, None, None), Level::Normal);
        // Mild: under 4 GiB to commit, under 3 GiB free, the card 85% used.
        assert_eq!(classify(Some(3 * GIB), plenty.1, plenty.2, None), Level::Mild);
        assert_eq!(classify(plenty.0, Some(2 * GIB), plenty.2, None), Level::Mild);
        assert_eq!(classify(plenty.0, plenty.1, Some((100, 85)), None), Level::Mild);
        // Critical: under 1.5 GiB to commit, the card full, a refusal now.
        assert_eq!(classify(Some(GIB), plenty.1, plenty.2, None), Level::Critical);
        assert_eq!(classify(plenty.0, plenty.1, Some((100, 97)), None), Level::Critical);
        assert_eq!(classify(plenty.0, plenty.1, plenty.2, Some(10)), Level::Critical);
        // A refusal long ago no longer counts.
        assert_eq!(classify(plenty.0, plenty.1, plenty.2, Some(5_000)), Level::Normal);
        // A card with no budget said is no pressure.
        assert_eq!(classify(plenty.0, plenty.1, Some((0, 5)), None), Level::Normal);
    }
}
