//! Host pipelines are made by workers, not by the title thread that first
//! records a draw with them. Translating a shader and the host driver's
//! compile take from milliseconds to seconds each (structurisation of the
//! biggest compute programs ~0.4 s, some driver compiles 3.4 s), and
//! the title's first world frame wants over 1,200 new pipelines: made
//! on its recording threads, that frame took more than 30 s and the
//! title's GPU watchdog ended the run (decision 0007, "Pipelines made off
//! the title's threads").
//!
//! A first use queues the pipeline and waits for it within a budget per
//! frame, so the cheap ones (menus, one-off passes such as a colour-grading
//! bake) are there at once as before; past the budget a draw or dispatch
//! whose pipeline is not ready is dropped, as an untranslated one is.
//!
//! The default budget is half a second per title frame. A pass the title
//! runs only once and whose pipeline takes longer is lost.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::Shader;
use super::host::{GraphicsState, HostPipeline};

/// What a host pipeline is made from, and the host pipeline once made.
pub(crate) struct Pending {
    shaders: Vec<Arc<Shader>>,
    pub(crate) graphics: Option<Box<GraphicsState>>,
    queue_state: AtomicU8,
    made: OnceLock<Option<HostPipeline>>,
    /// What identifies it across runs (shader containers and state), and
    /// whether a draw or dispatch has requested it yet.
    key: String,
    used: AtomicBool,
    /// Its shaders and [`GraphicsState::shape`], for a stand-in while it is
    /// being made ([`stand_in`]); `None` for compute.
    shape: Option<String>,
    /// Whether it is in [`SIBLINGS`] (or was looked at for it).
    listed: AtomicBool,
}

// SAFETY: D3D12 objects are free-threaded; the graphics state (whose input
// elements point into its own names) is only read after creation.
unsafe impl Send for Pending {}
// SAFETY: as above.
unsafe impl Sync for Pending {}

impl Pending {
    pub(crate) fn new(shaders: Vec<Arc<Shader>>, graphics: Option<Box<GraphicsState>>) -> Pending {
        let mut key = blake3::Hasher::new();
        for s in &shaders {
            key.update(s.stage.as_bytes());
            key.update(s.hash.as_bytes());
        }
        key.update(graphics.as_ref().map(|g| g.describe()).unwrap_or_default().as_bytes());
        let shape = graphics.as_ref().filter(|_| !shaders.is_empty()).map(|g| {
            let mut s = String::new();
            for shader in &shaders {
                s.push_str(shader.stage);
                s.push(' ');
                s.push_str(&shader.hash);
                s.push(' ');
            }
            s + &g.shape()
        });
        Pending {
            shaders,
            graphics,
            queue_state: AtomicU8::new(NOT_QUEUED),
            made: OnceLock::new(),
            key: key.finalize().to_hex()[..32].to_owned(),
            used: AtomicBool::new(false),
            shape,
            listed: AtomicBool::new(false),
        }
    }

    /// Whether a shader's container is past [`HEAVY_BYTES`].
    fn heavy(&self) -> bool {
        self.shaders.iter().any(|s| s.size() > HEAVY_BYTES)
    }

    fn make(&self) {
        let started = Instant::now();
        self.made
            .get_or_init(|| HostPipeline::new(&self.shaders, self.graphics.as_deref()));
        let took = started.elapsed();
        if took > Duration::from_secs(2) {
            tracing::info!(ms = took.as_millis() as u64, heavy = self.heavy(), shaders = ?self.shaders, "slow host pipeline");
        }
        let (lock, done) = &DONE;
        let _held = lock.lock().unwrap_or_else(|e| e.into_inner());
        done.notify_all();
    }
}

/// Why a pipeline has no host pipeline (diagnostics): its shaders that do
/// not translate, else "refused" or "being made"; `None` if it has one.
#[cfg(feature = "diagnostics")]
pub(crate) fn missing(p: &Pending) -> Option<String> {
    match p.made.get() {
        Some(Some(_)) => None,
        None => Some("being made".into()),
        Some(None) => {
            let failed: Vec<String> = p
                .shaders
                .iter()
                .filter(|s| matches!(s.translated.get(), Some(None)))
                .map(|s| format!("{} {}", s.stage, s.hash))
                .collect();
            Some(if failed.is_empty() {
                format!("refused {:?}", p.shaders)
            } else {
                failed.join(" + ")
            })
        }
    }
}

/// Signalled whenever a worker has made a pipeline.
static DONE: (Mutex<()>, Condvar) = (Mutex::new(()), Condvar::new());

/// Waiting allowed per title frame, over all threads.
fn wait_budget() -> Duration {
    static BUDGET: OnceLock<Duration> = OnceLock::new();
    *BUDGET.get_or_init(|| {
        let millis = std::env::var("VICTORY_PIPELINE_WAIT_MS")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(500);
        tracing::info!(millis, "host pipeline wait budget per frame");
        Duration::from_millis(millis)
    })
}

/// Pipelines queued and not made yet.
static WAITING: AtomicU64 = AtomicU64::new(0);

/// Pipelines queued and not made yet (for the frame line).
pub(crate) fn waiting() -> u64 {
    WAITING.load(Ordering::Relaxed)
}

/// The host pipeline if it is made (or is made within the frame's budget);
/// queues it on first use.
pub(crate) fn ready(p: &Arc<Pending>) -> Option<&HostPipeline> {
    // A still-pending pipeline was requested by the title too. Prewarm it
    // on the next launch, even if this run never got to use it.
    if !p.used.load(Ordering::Relaxed) && !p.used.swap(true, Ordering::Relaxed) {
        super::library::note_used(&p.key);
        // And what it is made from, for `victory prepare` after the run.
        super::recipe::note(&p.key, &p.shaders, p.graphics.as_deref());
    }
    if p.made.get().is_none() {
        start(p, REQUESTED);
        wait(p, crate::frame::current());
        if p.made.get().is_none() {
            return stand_in(p);
        }
    }
    let made = p.made.get()?.as_ref();
    if made.is_some() && p.shape.is_some() && !p.listed.swap(true, Ordering::Relaxed) {
        let mut siblings = SIBLINGS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(shape) = &p.shape {
            siblings
                .get_or_insert_with(std::collections::HashMap::new)
                .entry(shape.clone())
                .or_insert_with(|| &*Box::leak(Box::new(p.clone())));
        }
    }
    made
}

/// The first made pipeline of each [`Pending::shape`] that a draw used:
/// kept for the rest of the run (one host pipeline per shape).
static SIBLINGS: Mutex<Option<std::collections::HashMap<String, &'static Arc<Pending>>>> = Mutex::new(None);

/// Draws that went out with a stand-in pipeline since the start.
pub(crate) static STAND_INS: AtomicU64 = AtomicU64::new(0);

/// `VICTORY_STAND_IN=0`: a draw whose pipeline is still being made is
/// dropped, as before.
fn stand_in_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_STAND_IN").is_none_or(|v| v != "0"))
}

/// For a graphics pipeline still being made (never one refused or that does
/// not translate): a made pipeline of the same shaders and
/// [`GraphicsState::shape`], which differs from it only in state that
/// leaves nothing behind for later passes (culling, the depth test's
/// function, blend factors). Its draws go out where they were dropped
/// until the pipeline is made: in the first frames of a scene, geometry
/// that was missing (or a frame late) is there.
/// shortcut: for those frames such a draw may cull or blend as its
/// sibling does.
fn stand_in(p: &Pending) -> Option<&HostPipeline> {
    let shape = p.shape.as_ref().filter(|_| stand_in_on())?;
    let sibling: &'static Arc<Pending> = *SIBLINGS.lock().unwrap_or_else(|e| e.into_inner()).as_ref()?.get(shape)?;
    let made = sibling.made.get()?.as_ref()?;
    STAND_INS.fetch_add(1, Ordering::Relaxed);
    Some(made)
}

/// Makes `p` on this thread if it is not made yet (waiting for a worker
/// that is making it); whether it has a host pipeline.
pub(crate) fn now(p: &Arc<Pending>) -> bool {
    p.make();
    p.made.get().is_some_and(|made| made.is_some())
}

/// At the title's creation of a pipeline: starts it at once if an earlier
/// run drew with it (during loading, so the world's first frames find it).
pub(crate) fn early(p: &Arc<Pending>) {
    if !p.shaders.is_empty() && super::library::wanted_early(&p.key) {
        super::library::EARLY.fetch_add(1, Ordering::Relaxed);
        start(p, EARLY);
    }
}

/// Whether the frame's budget is one for all threads together: a thread
/// that waits takes what is left of it and gives back what it did not
/// use. Counted only when a wait had ended, the budget was one per thread:
/// each of the title's five recording threads began its first wait of a
/// frame with nothing counted yet and waited the whole half second, 2.5 s
/// of waiting in every frame in which pipelines were being made (runs
/// 34-49: "2504 ms waiting for pipelines" in each such frame).
/// On by default; `VICTORY_PIPELINE_WAIT_SHARED=0` counts as before.
fn shared_budget() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_PIPELINE_WAIT_SHARED").is_none_or(|v| v != "0"))
}

/// Waits for `p` while frame `frame` has budget left.
fn wait(p: &Pending, frame: u64) {
    let budget = wait_budget();
    if budget.is_zero() {
        return;
    }
    // The frame the budget belongs to, and what was waited of it (or is
    // taken by a thread waiting now).
    static SPENT: Mutex<(u64, Duration)> = Mutex::new((u64::MAX, Duration::ZERO));
    let shared = shared_budget();
    let left = {
        let mut spent = SPENT.lock().unwrap_or_else(|e| e.into_inner());
        if spent.0 != frame {
            *spent = (frame, Duration::ZERO);
        }
        let left = budget.saturating_sub(spent.1);
        if shared {
            spent.1 += left;
        }
        left
    };
    if left.is_zero() {
        return;
    }
    let started = Instant::now();
    let (lock, done) = &DONE;
    let mut held = lock.lock().unwrap_or_else(|e| e.into_inner());
    while p.made.get().is_none() {
        let rest = left.saturating_sub(started.elapsed());
        if rest.is_zero() {
            break;
        }
        held = done.wait_timeout(held, rest).unwrap_or_else(|e| e.into_inner()).0;
    }
    drop(held);
    let waited = started.elapsed().min(left);
    {
        let mut spent = SPENT.lock().unwrap_or_else(|e| e.into_inner());
        // A frame that began meanwhile has its own budget.
        if spent.0 == frame {
            if shared {
                spent.1 = spent.1.saturating_sub(left - waited);
            } else {
                spent.1 += waited;
            }
        }
    }
    #[cfg(feature = "diagnostics")]
    crate::copystat::add(
        crate::copystat::C::PipelineWaitUs,
        started.elapsed().as_micros() as u64,
    );
}

const NOT_QUEUED: u8 = 0;
const EARLY: u8 = 1;
const REQUESTED: u8 = 2;
const MAKING: u8 = 3;

/// A shader container past this size makes its pipeline heavy: the
/// world's biggest compute programs (275 of 60-78 KB in the first world
/// frames) each took an RTX 4060's driver ~18 s to compile. Queued with the
/// rest, they held every worker for minutes while ~1,600 small draw
/// pipelines waited behind them (4-9 made a minute), so the world's draws
/// were dropped and the picture stayed black.
pub(super) const HEAVY_BYTES: usize = 32 * 1024;

struct Queue {
    early: VecDeque<Arc<Pending>>,
    requested: VecDeque<Arc<Pending>>,
    /// Heavy pipelines, made by at most `heavy_limit` workers at once so
    /// the others keep making the light ones.
    heavy_early: VecDeque<Arc<Pending>>,
    heavy_requested: VecDeque<Arc<Pending>>,
    heavy_making: usize,
    heavy_limit: usize,
}

impl Queue {
    const fn new() -> Self {
        Self {
            early: VecDeque::new(),
            requested: VecDeque::new(),
            heavy_early: VecDeque::new(),
            heavy_requested: VecDeque::new(),
            heavy_making: 0,
            heavy_limit: 1,
        }
    }

    /// Queues an unrequested pipeline or promotes one queued at creation.
    /// Returns whether it is new to the queue, or `None` if nothing changed.
    fn push(&mut self, p: &Arc<Pending>, priority: u8) -> Option<bool> {
        let prev = p.queue_state.load(Ordering::Relaxed);
        if prev >= priority {
            return None;
        }
        p.queue_state.store(priority, Ordering::Relaxed);
        let heavy = p.heavy();
        let lane = match (priority == REQUESTED, heavy) {
            (true, false) => &mut self.requested,
            (true, true) => &mut self.heavy_requested,
            (false, false) => &mut self.early,
            (false, true) => &mut self.heavy_early,
        };
        lane.push_back(p.clone());
        Some(prev == NOT_QUEUED)
    }

    /// The next pipeline to make: light requested ones first, then heavy
    /// requested ones while a heavy slot is free, then those queued early.
    fn pop(&mut self) -> Option<Arc<Pending>> {
        fn early(lane: &mut VecDeque<Arc<Pending>>) -> Option<Arc<Pending>> {
            // An early entry can have been promoted and claimed already.
            std::iter::from_fn(|| lane.pop_front())
                .find(|p| p.queue_state.load(Ordering::Relaxed) == EARLY)
        }
        let heavy_free = self.heavy_making < self.heavy_limit;
        let p = self
            .requested
            .pop_front()
            .or_else(|| heavy_free.then(|| self.heavy_requested.pop_front()).flatten())
            .or_else(|| early(&mut self.early))
            .or_else(|| heavy_free.then(|| early(&mut self.heavy_early)).flatten())?;
        if p.heavy() {
            self.heavy_making += 1;
        }
        p.queue_state.store(MAKING, Ordering::Relaxed);
        Some(p)
    }
}

static QUEUE: Mutex<Queue> = Mutex::new(Queue::new());
static AVAILABLE: Condvar = Condvar::new();

/// Start or promote work for a draw without waiting behind the entire early
/// list. Queue changes and worker claims happen under the same lock.
fn start(p: &Arc<Pending>, priority: u8) {
    if p.queue_state.load(Ordering::Relaxed) >= priority {
        return;
    }
    if !workers() {
        if p.queue_state.swap(MAKING, Ordering::Relaxed) == NOT_QUEUED {
            p.make();
        }
        return;
    }
    let mut queue = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(new) = queue.push(p, priority) {
        if new {
            PEAK.fetch_max(WAITING.fetch_add(1, Ordering::Relaxed) + 1, Ordering::Relaxed);
        }
        AVAILABLE.notify_one();
    }
}

fn next() -> Arc<Pending> {
    let mut queue = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if let Some(p) = queue.pop() {
            return p;
        }
        queue = AVAILABLE.wait(queue).unwrap_or_else(|e| e.into_inner());
    }
}

/// A worker is done with `p`: frees its heavy slot.
fn finished(p: &Pending) {
    if p.heavy() {
        let mut queue = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
        queue.heavy_making = queue.heavy_making.saturating_sub(1);
        AVAILABLE.notify_all();
    }
}

/// Pipelines waiting past which the workers run above the title's
/// ordinary threads.
const BACKLOG: u64 = 64;

/// Pipelines waiting past which half the workers run at the highest
/// priority short of time-critical.
const BIG_BACKLOG: u64 = 512;

/// The priority level a worker runs at, 0 to 3 (below normal, normal,
/// above normal, highest), with `waiting` pipelines queued.
///
/// Normal was not enough: the title raises its own threads (it binds
/// `SetThreadPriority` to the host's), and in a world run on an
/// i5-12400F and RTX 4060 the workers made 71 pipelines in 9 minutes
/// (about 7 a minute) while 1,675 waited and 5,000 draws a frame were
/// dropped for them. With a backlog the workers now go above the
/// title's ordinary threads, and with a long one half of them go to the
/// highest priority, the other half staying above normal so the title
/// keeps running; once the backlog is gone they drop back below normal.
/// `VICTORY_PIPELINE_BOOST=0`: normal at most, as before.
fn priority_level(waiting: u64, worker: usize, workers: usize) -> u8 {
    static BOOST: OnceLock<bool> = OnceLock::new();
    let boost = *BOOST.get_or_init(|| std::env::var_os("VICTORY_PIPELINE_BOOST").is_none_or(|v| v != "0"));
    match waiting {
        w if w <= BACKLOG => 0,
        _ if !boost => 1,
        w if w > BIG_BACKLOG && worker < workers.div_ceil(2) => 3,
        _ => 2,
    }
}

/// Starts the workers on first use: two thirds of the logical processors,
/// 2 to 10, half of them (1 to 3: the driver ran out of memory with six
/// heavy compiles at once) allowed on heavy pipelines.
///
/// Each gets a 64 MB stack (Peppacodes, upstream #966): NVIDIA's compiler
/// recursed past the default 2 MB on a world compute pipeline, and the
/// driver reported the overflow as E_OUTOFMEMORY, refusing it for good.
/// Only the pages used are committed.
fn workers() -> bool {
    static RUNNING: OnceLock<bool> = OnceLock::new();
    *RUNNING.get_or_init(|| {
        let mut running = false;
        let workers = std::thread::available_parallelism().map_or(2, |n| (n.get() * 2 / 3).clamp(2, 10));
        let heavy = (workers / 2).clamp(1, 3);
        QUEUE.lock().unwrap_or_else(|e| e.into_inner()).heavy_limit = heavy;
        tracing::info!(workers, heavy, "pipeline workers");
        for i in 0..workers {
            let spawned = std::thread::Builder::new()
                .name(format!("victory-pipelines-{i}"))
                .stack_size(64 << 20)
                .spawn(move || {
                    let mut raised = None;
                    loop {
                        let p = next();
                        // Below the title's own threads (loading, recording)
                        // unless a backlog waits: the title then runs at
                        // full load and starved the workers (4-9 pipelines a
                        // minute in the world on an i5-12400F; see
                        // `priority_level`).
                        let level = priority_level(WAITING.load(Ordering::Relaxed), i, workers);
                        if raised != Some(level) {
                            raised = Some(level);
                            // SAFETY: a plain call on our own thread.
                            unsafe {
                                use windows::Win32::System::Threading::*;
                                let _ = SetThreadPriority(
                                    GetCurrentThread(),
                                    match level {
                                        0 => THREAD_PRIORITY_BELOW_NORMAL,
                                        1 => THREAD_PRIORITY_NORMAL,
                                        2 => THREAD_PRIORITY_ABOVE_NORMAL,
                                        _ => THREAD_PRIORITY_HIGHEST,
                                    },
                                );
                            }
                        }
                        p.make();
                        finished(&p);
                        if WAITING.fetch_sub(1, Ordering::Relaxed) == 1 {
                            drained();
                        } else {
                            super::library::save(false);
                        }
                    }
                });
            if let Err(e) = spawned {
                tracing::warn!(error = %e, "no pipeline worker");
            } else {
                running = true;
            }
        }
        running
    })
}

/// The most pipelines waiting at once since the workers last ran out.
static PEAK: AtomicU64 = AtomicU64::new(0);

/// The workers ran out of work: logged after a batch of 10 or more, and
/// the cache saved (at most every 20 s).
fn drained() {
    use super::library::{EARLY, LOADED, MADE};
    if PEAK.swap(0, Ordering::Relaxed) >= 10 {
        tracing::info!(
            made = MADE.load(Ordering::Relaxed),
            loaded = LOADED.load(Ordering::Relaxed),
            started_early = EARLY.load(Ordering::Relaxed),
            "host pipelines all made"
        );
    }
    super::library::save(false);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A first use queues the pipeline and, within the budget, gets the
    /// worker's answer (here: no host pipeline for no shaders).
    #[test]
    fn first_use_is_made_by_a_worker() {
        let p = Arc::new(Pending::new(Vec::new(), None));
        assert!(ready(&p).is_none());
        assert_ne!(p.queue_state.load(Ordering::Relaxed), NOT_QUEUED);
        let started = Instant::now();
        while p.made.get().is_none() && started.elapsed() < Duration::from_secs(10) {
            std::thread::yield_now();
        }
        assert!(matches!(p.made.get(), Some(None)));
    }

    #[test]
    fn pending_pipeline_is_recorded_for_the_next_launch() {
        let mut p = Pending::new(Vec::new(), None);
        p.key = "pending-pipeline-test".to_owned();
        // Simulate a worker that has taken the job but cannot finish it yet.
        p.queue_state.store(MAKING, Ordering::Relaxed);
        let p = Arc::new(p);

        assert!(!super::super::library::wanted_early(&p.key));
        assert!(ready(&p).is_none());
        assert!(p.made.get().is_none());
        assert!(super::super::library::wanted_early(&p.key));
    }

    #[test]
    fn draws_promote_early_pipelines_without_claiming_them_twice() {
        let earlier = Arc::new(Pending::new(Vec::new(), None));
        let promoted = Arc::new(Pending::new(Vec::new(), None));
        let requested = Arc::new(Pending::new(Vec::new(), None));
        let mut queue = Queue::new();

        assert_eq!(queue.push(&earlier, EARLY), Some(true));
        assert_eq!(queue.push(&promoted, EARLY), Some(true));
        assert_eq!(queue.push(&requested, REQUESTED), Some(true));
        assert_eq!(queue.push(&promoted, REQUESTED), Some(false));
        assert_eq!(queue.push(&promoted, REQUESTED), None);

        assert!(Arc::ptr_eq(&queue.pop().unwrap(), &requested));
        assert!(Arc::ptr_eq(&queue.pop().unwrap(), &promoted));
        assert_eq!(queue.push(&promoted, REQUESTED), None);
        assert!(Arc::ptr_eq(&queue.pop().unwrap(), &earlier));
        assert!(queue.pop().is_none());
    }

    /// Workers go above the title's threads only with a backlog, and half
    /// of them to the highest priority only with a long one.
    #[test]
    fn workers_rise_with_the_backlog() {
        assert_eq!(priority_level(0, 0, 8), 0);
        assert_eq!(priority_level(BACKLOG, 0, 8), 0);
        assert_eq!(priority_level(BACKLOG + 1, 0, 8), 2);
        assert_eq!(priority_level(BIG_BACKLOG + 1, 0, 8), 3);
        assert_eq!(priority_level(BIG_BACKLOG + 1, 3, 8), 3);
        assert_eq!(priority_level(BIG_BACKLOG + 1, 4, 8), 2);
        assert_eq!(priority_level(BIG_BACKLOG + 1, 7, 8), 2);
    }

    /// Heavy pipelines wait while their slots are taken; light ones and
    /// those queued early go on past them.
    #[test]
    fn heavy_pipelines_do_not_hold_up_light_ones() {
        let big = super::super::shader("compute", &vec![7u8; HEAVY_BYTES + 1]);
        let heavy = || Arc::new(Pending::new(vec![big.clone()], None));
        let (h1, h2) = (heavy(), heavy());
        let light = Arc::new(Pending::new(Vec::new(), None));
        let early = Arc::new(Pending::new(Vec::new(), None));
        let mut queue = Queue::new();
        assert_eq!(queue.heavy_limit, 1);

        queue.push(&h1, REQUESTED);
        queue.push(&h2, REQUESTED);
        queue.push(&early, EARLY);
        queue.push(&light, REQUESTED);
        assert!(Arc::ptr_eq(&queue.pop().unwrap(), &light));
        assert!(Arc::ptr_eq(&queue.pop().unwrap(), &h1));
        assert!(Arc::ptr_eq(&queue.pop().unwrap(), &early));
        assert!(queue.pop().is_none(), "the heavy slot is taken");
        queue.heavy_making -= 1;
        assert!(Arc::ptr_eq(&queue.pop().unwrap(), &h2));
    }
}
