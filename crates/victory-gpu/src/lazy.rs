//! Lazy copy-back (`VICTORY_LAZY_BACK=0` turns it off): with the relay
//! (`crate::host::relay`), when the card has run a batch of flights, the
//! title memory their shaders may have written is made no-access, the
//! fences behind them are signalled at once, and only then is what the
//! shaders wrote copied back into it. Before, the title heard that the card
//! was done only after the whole copy-back, 200-800 ms a frame in the
//! 2026-11-24 reports: every title thread that waited for a fence to reuse
//! memory, pace its frames or submit more waited for it, even when it never
//! read what the shaders wrote.
//!
//! Now a title thread that touches held memory takes an access violation;
//! our handler holds it until the batch is copied back (or
//! `VICTORY_LAZY_BACK_WAIT_MS`, 1000 unless set, have passed: then that
//! piece is opened as it is, so no thread waits for ever on one that waits
//! for it), then the access runs again. Our own reads and writes of title
//! memory through the kernel (`crate::host::read_into`,
//! `crate::host::write_mapped`) fail on held memory; they open the 64 KiB
//! pieces they touch and try again, which is how the copy-back itself gets
//! in, piece by piece, right before it merges one. The pieces nothing
//! wrote are opened together when the batch is done, each with the
//! protection it had (write-watch guards of `crate::watch` included).
//!
//! shortcut: a title thread that first touches a piece in the microseconds
//! between the copy-back opening it and writing it reads the bytes from
//! before (the copy-back already races title writes the same way). A
//! system call that reads held memory (a file write from it) fails rather
//! than waits; none is known to read what shaders wrote. If any page of a
//! batch cannot be made no-access, nothing of the batch is held and the
//! fences wait for the copy-back as before.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// The kernel's page, the unit of protection.
const PAGE: usize = 4096;
/// What a merge opens at a time, and what the held memory is noted in.
const PIECE: usize = 64 << 10;

/// One held piece of title memory: where it ends, the protection it had,
/// and the start of the region it was protected with (pieces of one
/// region are opened together).
struct Held {
    end: usize,
    old: u32,
    region: usize,
}

struct State {
    /// Piece start → piece; the pieces never overlap.
    held: BTreeMap<usize, Held>,
}

static STATE: Mutex<State> = Mutex::new(State { held: BTreeMap::new() });
/// Told when a batch is done (everything opened).
static DONE: Condvar = Condvar::new();
/// Something is held: the cheap test before the lock.
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// For the frame line: batches held, batches that could not be (copied
/// back first, as before), fence signals and writes sent before the
/// copy-back, title accesses held, microseconds they were held, accesses
/// let through after the longest wait.
pub(crate) static LAZY_STATS: [AtomicU64; 6] = [const { AtomicU64::new(0) }; 6];

thread_local! {
    /// The thread that lands flights: it opens what it touches, it never
    /// waits (the copy-back it would wait for is its own).
    static LANDING: Cell<bool> = const { Cell::new(false) };
}

/// Whether lazy copy-back is on (`VICTORY_LAZY_BACK=0` turns it off).
pub(crate) fn on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        let on = std::env::var_os("VICTORY_LAZY_BACK").is_none_or(|v| v != "0");
        if on {
            tracing::info!(
                wait_most_ms = wait_most().as_millis() as u64,
                "the card's results are copied back after the title hears it is done; title threads that touch them first wait (VICTORY_LAZY_BACK=0: copied back first)"
            );
        }
        on
    })
}

/// The longest a title thread is held (`VICTORY_LAZY_BACK_WAIT_MS`).
fn wait_most() -> Duration {
    static MS: OnceLock<u64> = OnceLock::new();
    Duration::from_millis(*MS.get_or_init(|| {
        std::env::var("VICTORY_LAZY_BACK_WAIT_MS").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(1000)
    }))
}

/// Held memory touched by a title thread or by our own code (not by the
/// thread that lands flights, whose touches are the copy-back), as
/// (start, end), for `crate::host`'s copy-back of only what is read; a
/// fixed array, as the exception handler notes into it, and whether more
/// touches came than it holds (then everything counts as touched).
static TOUCHED: Mutex<([(usize, usize); 256], usize, bool)> = Mutex::new(([(0, 0); 256], 0, false));

/// Notes a touch of held `[at, at + len)` ([`TOUCHED`]).
fn touched(at: usize, len: usize) {
    if LANDING.get() {
        return;
    }
    let mut t = TOUCHED.lock().unwrap_or_else(|e| e.into_inner());
    let n = t.1;
    if n < t.0.len() {
        t.0[n] = (at, at.saturating_add(len.max(1)));
        t.1 = n + 1;
    } else {
        t.2 = true;
    }
}

/// Whether held memory in `[from, to)` was touched since the last
/// [`take_touched`].
pub(crate) fn touched_in(from: usize, to: usize) -> bool {
    let t = TOUCHED.lock().unwrap_or_else(|e| e.into_inner());
    t.2 || t.0[..t.1].iter().any(|&(a, b)| a < to && b > from)
}

/// The touches since the last call (None: more than are noted, so take
/// everything as touched), forgotten.
pub(crate) fn take_touched() -> Option<Vec<(usize, usize)>> {
    let mut t = TOUCHED.lock().unwrap_or_else(|e| e.into_inner());
    let out = (!t.2).then(|| t.0[..t.1].to_vec());
    t.1 = 0;
    t.2 = false;
    out
}

/// Called once by the thread that lands flights.
pub(crate) fn landing_thread() {
    LANDING.set(true);
}

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Sets the protection of `[from, to)` back to `old`.
fn restore(from: usize, to: usize, old: u32) -> bool {
    use windows_sys::Win32::System::Memory::VirtualProtect;
    let mut was = 0;
    // SAFETY: title memory we made no-access; only its protection changes.
    unsafe { VirtualProtect(from as *const _, to - from, old, &mut was) != 0 }
}

/// Makes the title memory of `ranges` (address, end) no-access and notes
/// it held. False, with nothing held, if some committed page of it cannot
/// be (or a batch is held already): the caller copies back first, as
/// before.
pub(crate) fn hold(ranges: &[(usize, usize)]) -> bool {
    use windows_sys::Win32::System::Memory::{
        MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_GUARD, PAGE_NOACCESS, VirtualProtect, VirtualQuery,
    };
    install();
    let mut pages: Vec<(usize, usize)> = ranges
        .iter()
        .filter(|r| r.0 < r.1)
        .map(|&(a, b)| (a & !(PAGE - 1), b.div_ceil(PAGE).saturating_mul(PAGE)))
        .collect();
    if pages.is_empty() {
        return false;
    }
    pages.sort_unstable();
    let mut joined: Vec<(usize, usize)> = Vec::with_capacity(pages.len());
    for (a, b) in pages {
        match joined.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => joined.push((a, b)),
        }
    }
    let mut st = state();
    if !st.held.is_empty() {
        LAZY_STATS[1].fetch_add(1, Ordering::Relaxed);
        return false;
    }
    let mut made: Vec<(usize, usize, u32)> = Vec::new();
    let mut ok = true;
    'all: for (a, b) in joined {
        let mut at = a;
        while at < b {
            // SAFETY: plain data filled in by the query.
            let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
            // SAFETY: a valid out pointer of the right size.
            let n = unsafe { VirtualQuery(at as *const _, &mut info, size_of_val(&info)) };
            let end = (info.BaseAddress as usize).saturating_add(info.RegionSize).min(b);
            if n == 0 || end <= at {
                ok = false;
                break 'all;
            }
            if info.State != MEM_COMMIT {
                // Nothing there to read; left alone.
                at = end;
                continue;
            }
            if info.Protect == 0 || info.Protect & (PAGE_GUARD | PAGE_NOACCESS) != 0 {
                ok = false;
                break 'all;
            }
            let mut old = 0;
            // SAFETY: committed title memory; only its protection changes.
            if unsafe { VirtualProtect(at as *const _, end - at, PAGE_NOACCESS, &mut old) } == 0 {
                ok = false;
                break 'all;
            }
            made.push((at, end, old));
            at = end;
        }
    }
    if !ok || made.is_empty() {
        for &(a, e, old) in &made {
            restore(a, e, old);
        }
        if !ok {
            LAZY_STATS[1].fetch_add(1, Ordering::Relaxed);
        }
        return false;
    }
    for (a, e, old) in made {
        let mut at = a;
        while at < e {
            let next = ((at / PIECE + 1) * PIECE).min(e);
            st.held.insert(at, Held { end: next, old, region: a });
            at = next;
        }
    }
    ACTIVE.store(true, Ordering::Release);
    LAZY_STATS[0].fetch_add(1, Ordering::Relaxed);
    true
}

/// Whether `at` lies in a held piece.
fn holds_at(st: &State, at: usize) -> bool {
    st.held.range(..=at).next_back().is_some_and(|(_, h)| h.end > at)
}

/// Whether any byte of `[at, at + len)` is held.
pub(crate) fn holds_any(at: usize, len: usize) -> bool {
    if !ACTIVE.load(Ordering::Acquire) {
        return false;
    }
    let end = at.saturating_add(len.max(1));
    state().held.range(..end).next_back().is_some_and(|(_, h)| h.end > at)
}

/// Opens the held pieces that overlap `[from, to)`, each with the
/// protection it had; true if there were any.
fn open_locked(st: &mut State, from: usize, to: usize) -> bool {
    let keys: Vec<usize> = st.held.range(..to).rev().take_while(|(_, h)| h.end > from).map(|(k, _)| *k).collect();
    for k in &keys {
        if let Some(h) = st.held.remove(k) {
            restore(*k, h.end, h.old);
        }
    }
    !keys.is_empty()
}

/// Our own access to title memory `[at, at + len)` through the kernel
/// failed: opens what of it is held (true if anything was, and the access
/// is worth another try).
pub(crate) fn release(at: usize, len: usize) -> bool {
    if !ACTIVE.load(Ordering::Acquire) {
        return false;
    }
    let opened = open_locked(&mut state(), at, at.saturating_add(len.max(1)));
    if opened {
        touched(at, len);
    }
    opened
}

/// The runtime maps or unmaps title memory `[at, at + len)`: what is held
/// there is no longer ours to open (the new pages have their own
/// protection).
pub(crate) fn forget(at: usize, len: usize) {
    if !ACTIVE.load(Ordering::Acquire) {
        return;
    }
    let mut st = state();
    let to = at.saturating_add(len);
    let keys: Vec<usize> = st.held.range(..to).rev().take_while(|(_, h)| h.end > at).map(|(k, _)| *k).collect();
    for k in keys {
        st.held.remove(&k);
    }
}

/// The batch is copied back: everything still held is opened (a region's
/// pieces with one call), and the title threads held go on.
pub(crate) fn done() {
    let mut st = state();
    let held = std::mem::take(&mut st.held);
    let mut run: Option<(usize, usize, u32, usize)> = None;
    let flush = |run: (usize, usize, u32, usize)| {
        if !restore(run.0, run.1, run.2) {
            // Piece by piece, as a fallback.
            let mut at = run.0;
            while at < run.1 {
                let next = ((at / PIECE + 1) * PIECE).min(run.1);
                restore(at, next, run.2);
                at = next;
            }
        }
    };
    for (start, h) in held {
        match &mut run {
            Some((_, end, old, region)) if *end == start && *old == h.old && *region == h.region => *end = h.end,
            _ => {
                if let Some(r) = run.take() {
                    flush(r);
                }
                run = Some((start, h.end, h.old, h.region));
            }
        }
    }
    if let Some(r) = run {
        flush(r);
    }
    ACTIVE.store(false, Ordering::Release);
    drop(st);
    DONE.notify_all();
}

/// An access violation at `at`: if it is held memory, the thread waits
/// until the batch is copied back (the thread that lands flights opens the
/// piece instead), and true says to run the access again.
pub(crate) fn fault(at: usize) -> bool {
    if !ACTIVE.load(Ordering::Acquire) {
        return false;
    }
    let mut st = state();
    if !holds_at(&st, at) {
        return false;
    }
    if LANDING.get() {
        open_locked(&mut st, at, at + 1);
        return true;
    }
    touched(at, 1);
    let began = Instant::now();
    let deadline = began + wait_most();
    LAZY_STATS[3].fetch_add(1, Ordering::Relaxed);
    while holds_at(&st, at) {
        let now = Instant::now();
        if now >= deadline {
            open_locked(&mut st, at, at + 1);
            LAZY_STATS[5].fetch_add(1, Ordering::Relaxed);
            break;
        }
        st = DONE.wait_timeout(st, deadline - now).unwrap_or_else(|e| e.into_inner()).0;
    }
    LAZY_STATS[4].fetch_add(began.elapsed().as_micros() as u64, Ordering::Relaxed);
    true
}

/// Registers the handler that holds title threads at held memory, once,
/// ahead of the crash reporter's.
fn install() {
    use windows_sys::Win32::Foundation::{EXCEPTION_ACCESS_VIOLATION, NTSTATUS};
    use windows_sys::Win32::System::Diagnostics::Debug::{
        AddVectoredExceptionHandler, EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH, EXCEPTION_POINTERS,
    };
    unsafe extern "system" fn on_fault(info: *mut EXCEPTION_POINTERS) -> i32 {
        // SAFETY: the system passes the exception being dispatched.
        let r = unsafe { &*(*info).ExceptionRecord };
        if r.ExceptionCode != EXCEPTION_ACCESS_VIOLATION as NTSTATUS || r.NumberParameters < 2 {
            return EXCEPTION_CONTINUE_SEARCH;
        }
        if fault(r.ExceptionInformation[1]) {
            EXCEPTION_CONTINUE_EXECUTION
        } else {
            EXCEPTION_CONTINUE_SEARCH
        }
    }
    static ONCE: std::sync::Once = std::sync::Once::new();
    // SAFETY: the handler only touches our own state and protections.
    ONCE.call_once(|| unsafe {
        AddVectoredExceptionHandler(1, Some(on_fault));
    });
}

/// For the frame line, since the last one.
pub(crate) fn line(frames: f64) -> String {
    let s: Vec<u64> = LAZY_STATS.iter().map(|s| s.swap(0, Ordering::Relaxed)).collect();
    let per = |v: u64| v as f64 / frames.max(1.0);
    format!(
        "lazy copy-back (VICTORY_LAZY_BACK=0: off) per frame: {:.1} batches held, {:.1} copied back first, {:.1} signals and writes sent before the copy-back, {:.1} title accesses held for {:.1} ms, {:.1} let through after {} ms",
        per(s[0]),
        per(s[1]),
        per(s[2]),
        per(s[3]),
        per(s[4]) / 1000.0,
        per(s[5]),
        wait_most().as_millis()
    )
}
