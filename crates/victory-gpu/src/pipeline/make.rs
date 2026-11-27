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
use super::host::{GraphicsState, HostPipeline, TargetFormats};

/// `VICTORY_DENY_PIPELINES=<16-hex shader hash>,...` (off unless set): the
/// hash of a shader of `shaders` that is listed there. Such a pipeline is
/// never built (no driver compile) and counts as refused, so its draws and
/// dispatches are dropped: a test of whether building a pipeline, rather
/// than running it, is what loses the card (runs 281 and 318: the removal
/// 32 and 81 ms after the 3-second build of `CS_RaytracingResolve` of the
/// two-layer vehicle wheel material, 57a3fad03cac36af and d5b9c94a30d7cf28;
/// gfx41). Unlike `VICTORY_SKIP_PIPELINES`, in release builds too.
pub(super) fn denied(shaders: &[Arc<Shader>]) -> Option<&str> {
    static LIST: OnceLock<Vec<String>> = OnceLock::new();
    let list = LIST.get_or_init(|| {
        std::env::var("VICTORY_DENY_PIPELINES")
            .unwrap_or_default()
            .split(',')
            .map(|h| h.trim().to_ascii_lowercase())
            .filter(|h| h.len() == 16 && h.bytes().all(|b| b.is_ascii_hexdigit()))
            .collect()
    });
    if list.is_empty() {
        return None;
    }
    shaders.iter().find(|s| list.contains(&s.hash)).map(|s| s.hash.as_str())
}

/// Try 6.19, after trustno1-coder/VIctory-NVIDIA 35c15608 (readmet3xt-lab):
/// the light in the water's volume, in tiles
/// (`water_shading:CS_TransmissionVolumetrics_Tiled` and `_Tiled_LocalLights`,
/// GTA VI 1.0.2.1). Programs of one 64-lane wave a group, which a host of
/// 32-lane waves gets in the wave-wide form only (`victory_shader::emit`); in
/// that form they leave whole tiles of open water dark (around boats, along
/// shores, under bridges) and take about 15 ms of the card a frame (RTX 5070:
/// 25 fps with them, 30 without, the blocks gone; fxceless/victory-fork names
/// the same two programs and form). Left out there: the water has no light of
/// its own depth (darker, bluer) and no blocks. `VICTORY_WATER_LIGHT=1` runs
/// them as before.
const WATER_LIGHT: [&str; 2] = ["5dc3b25986622949", "6f225b641c5b0708"];

/// Whether `shaders` are the water's tiled light on a host of 32-lane waves,
/// left out ([`WATER_LIGHT`]).
pub(super) fn water_light_left_out(shaders: &[Arc<Shader>]) -> bool {
    static ASKED: OnceLock<bool> = OnceLock::new();
    !*ASKED.get_or_init(|| std::env::var_os("VICTORY_WATER_LIGHT").is_some_and(|v| v == "1"))
        && victory_shader::emit::host_uses_wave32()
        && shaders.iter().any(|s| WATER_LIGHT.contains(&s.hash.as_str()))
}

/// The shaders of `p` (for logs).
pub(crate) fn shaders(p: &Pending) -> &[Arc<Shader>] {
    &p.shaders
}

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
    /// Its shaders and [`GraphicsState::loose_shape`], for the second tier
    /// of stand-ins ([`stand_in`]).
    loose: Option<String>,
    /// Whether it is in [`SIBLINGS`] (or was looked at for it).
    listed: AtomicBool,
    /// Draws and dispatches that asked for it before it was made: the
    /// queue makes the most asked for first ([`most_asked_first`]).
    asks: AtomicU64,
    /// Whether the made pipeline has gone to a draw or dispatch yet
    /// (`crate::host::first_use`: for a device removal's report).
    handed: AtomicBool,
    /// The formats its state declares for what a draw draws into (zeros
    /// without a graphics state).
    pub(crate) formats: TargetFormats,
    /// The same shaders and state for other formats, bound at a draw
    /// ([`for_formats`]): a chain from the title's pipeline, one link per
    /// set of formats met, never shortened.
    variant: OnceLock<Arc<Pending>>,
    /// The first title frame in which a draw or dispatch waited for it
    /// (`u64::MAX`: none yet; [`wait_once`]).
    waited_frame: AtomicU64,
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
        let names = || {
            let mut s = String::new();
            for shader in &shaders {
                s.push_str(shader.stage);
                s.push(' ');
                s.push_str(&shader.hash);
                s.push(' ');
            }
            s
        };
        let shape = graphics.as_ref().filter(|_| !shaders.is_empty()).map(|g| names() + &g.shape());
        let loose = graphics.as_ref().filter(|_| !shaders.is_empty()).map(|g| names() + &g.loose_shape());
        Pending {
            formats: graphics.as_ref().map_or([0; 9], |g| g.formats()),
            variant: OnceLock::new(),
            shaders,
            graphics,
            queue_state: AtomicU8::new(NOT_QUEUED),
            made: OnceLock::new(),
            key: key.finalize().to_hex()[..32].to_owned(),
            used: AtomicBool::new(false),
            shape,
            loose,
            listed: AtomicBool::new(false),
            asks: AtomicU64::new(0),
            handed: AtomicBool::new(false),
            waited_frame: AtomicU64::new(u64::MAX),
        }
    }

    /// Whether a shader's container is past [`HEAVY_BYTES`].
    fn heavy(&self) -> bool {
        self.shaders.iter().any(|s| s.size() > HEAVY_BYTES)
    }

    fn make(&self) {
        let started = Instant::now();
        self.made.get_or_init(|| {
            if let Some(hash) = denied(&self.shaders) {
                // (Try 6.12 denies a list of 1,034 by default: the first
                // few said, the rest counted.)
                static SAID: AtomicU64 = AtomicU64::new(0);
                let n = SAID.fetch_add(1, Ordering::Relaxed) + 1;
                if n <= 8 || n.is_power_of_two() {
                    tracing::info!(hash, shaders = ?self.shaders, so_far = n, "pipeline not built: its shader is in VICTORY_DENY_PIPELINES (on purpose: Try lists the ray-tracing and driver-crashing ones)");
                }
                return None;
            }
            // Try 6.19 (nv 35c15608): the water's tiled light, dark blocks
            // on a host of 32-lane waves.
            if water_light_left_out(&self.shaders) {
                static SAID: AtomicU64 = AtomicU64::new(0);
                if SAID.fetch_add(1, Ordering::Relaxed) == 0 {
                    tracing::info!(
                        shaders = ?self.shaders,
                        "pipeline not built: the water's tiled light on a host of 32-lane waves (dark blocks on open water; VICTORY_WATER_LIGHT=1 builds it)"
                    );
                }
                return None;
            }
            // Try 6.12 (edsuuu/VI-BR-PORT): passes that only serve rays,
            // whose dispatches are left out, are not compiled either.
            if super::never_built(&self.shaders) {
                static SAID: AtomicU64 = AtomicU64::new(0);
                let n = SAID.fetch_add(1, Ordering::Relaxed) + 1;
                if n == 1 || n.is_power_of_two() {
                    tracing::info!(so_far = n, "pipelines of passes that only serve rays not built (VICTORY_SKIP_RT=1; VICTORY_RT_SKIN, VICTORY_RT_RESOLVE, VICTORY_RT_TRACE, VICTORY_ACTIVATE_RAYS =1 build them)");
                }
                return None;
            }
            // Try 6.15, after cool_fox 744c09ba (Filip Michalski, #1353): a
            // build that panics (a translation bug) is a refused pipeline.
            // Before, the panic ended the worker thread and left the entry
            // unmade for good: its draws were dropped as "being made" for
            // the rest of the run, and one worker fewer made the others.
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                HostPipeline::new(&self.shaders, self.graphics.as_deref())
            })) {
                Ok(made) => made,
                Err(_) => {
                    tracing::warn!(shaders = ?self.shaders, "host pipeline build panicked: refused");
                    None
                }
            }
        });
        let took = started.elapsed();
        if took > Duration::from_secs(2) {
            tracing::info!(ms = took.as_millis() as u64, heavy = self.heavy(), shaders = ?self.shaders, "slow host pipeline");
        }
        let (lock, done) = &DONE;
        let _held = lock.lock().unwrap_or_else(|e| e.into_inner());
        done.notify_all();
    }
}

/// Why a pipeline has no host pipeline: its shaders that do not
/// translate, else "refused" or "being made"; `None` if it has one.
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

/// Try 6.13, from edsuuu/VI-BR-PORT dc3fa30b and trustno1-coder/VIctory-NVIDIA
/// 77e13a97 (the problem of fxceless issue #1181): whether a pipeline still
/// being made is waited for in the first frame that asks for it only. A
/// pipeline whose make takes longer than the budget (a driver compile of
/// seconds) otherwise cost the whole budget in every frame it was in view:
/// the picture stood still, half a second a frame, until the camera turned
/// away. After that frame its draws get a stand-in or are dropped until it
/// is made, as past any frame's budget. On by default;
/// `VICTORY_PIPELINE_WAIT_ONCE=0` waits in every frame as before.
fn wait_once() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_PIPELINE_WAIT_ONCE").is_none_or(|v| v != "0"))
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
        p.asks.fetch_add(1, Ordering::Relaxed);
        start(p, REQUESTED);
        // fx a42b627 (Exopus, #1312): a dispatch waits for its compute
        // pipeline until it is made (`VICTORY_PIPELINE_COMPUTE_WAIT=0`: within
        // the frame's budget, as before): a dispatch dropped for frames leaves
        // what it computes (skinning, light) missing or stale on screen.
        if p.graphics.is_none() && compute_waits() {
            // Made here, or waited for while a worker makes it.
            p.make();
        }
        let frame = crate::frame::current();
        // Waited for in the first frame that asks for it only
        // ([`wait_once`]); later frames go on without it.
        let first = match p.waited_frame.compare_exchange(u64::MAX, frame, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => frame,
            Err(f) => f,
        };
        // Try 6.15, a new thing on screen at once: a stand-in of the same
        // shaders and shape (`stand_in`, its first tier) is taken before any
        // wait. The object is drawn in this very frame, and the frame does
        // not stand still for the driver's compile; the real pipeline
        // replaces it as soon as it is made. After Kelvaroq 0050
        // (stellarr-studio: "a hit binds immediately, a miss keeps the
        // budget"). `VICTORY_STAND_IN_FIRST=0`: wait first, as before.
        if p.made.get().is_none()
            && stand_in_first()
            && let Some(made) = stand_in_tier(p, false)
        {
            STAND_INS_FIRST.fetch_add(1, Ordering::Relaxed);
            return Some(made);
        }
        if p.made.get().is_none() && (first == frame || !wait_once()) {
            wait(p, frame);
        }
        if p.made.get().is_none() {
            return stand_in(p);
        }
    }
    let made = p.made.get()?.as_ref();
    if made.is_some() && p.shape.is_some() && !p.listed.swap(true, Ordering::Relaxed) {
        let mut siblings = SIBLINGS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(shape) = &p.shape {
            let map = siblings.get_or_insert_with(std::collections::HashMap::new);
            let new_shape = !map.contains_key(shape);
            let new_loose = p.loose.as_ref().filter(|l| !map.contains_key(*l));
            if new_shape || new_loose.is_some() {
                let leaked: &'static Arc<Pending> = &*Box::leak(Box::new(p.clone()));
                if new_shape {
                    map.insert(shape.clone(), leaked);
                }
                if let Some(loose) = new_loose {
                    map.insert(loose.clone(), leaked);
                }
            }
        }
    }
    // The first time the made pipeline goes to a draw or dispatch: noted
    // for a device removal's report (one load a call after that).
    if let Some(host) = made
        && !p.handed.load(Ordering::Relaxed)
        && !p.handed.swap(true, Ordering::Relaxed)
    {
        crate::host::first_use(&host.name);
    }
    made
}

/// Whether a dispatch waits for its compute pipeline until it is made,
/// whatever the frame's budget ([`wait_budget`]); a draw still waits within
/// the budget and is dropped while its pipeline is not there.
///
/// A dropped draw is drawn again in the next frame. A dropped dispatch may
/// not be: the title runs some compute passes once, when a scene changes
/// (a buffer reset or cleared), and what they leave is read for as long as
/// the scene lasts. In the prison corridor a compute program met for the
/// first time took 4.4 s to make on a 16 GB machine; its one dispatch was
/// dropped after the half second of budget, and the windows of the room
/// before stayed blended over the whole picture for minutes (run 195).
/// A wait of seconds once (the pipeline library has it on the next run)
/// against a picture wrong until the scene ends.
/// On by default; `VICTORY_PIPELINE_COMPUTE_WAIT=0`: within the budget, as
/// before.
fn compute_waits() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_PIPELINE_COMPUTE_WAIT").is_none_or(|v| v != "0"))
}

/// Pipelines made again for the formats bound at a draw, since the start.
static VARIANTS: AtomicU64 = AtomicU64::new(0);

/// Whether a draw into views of other formats than its pipeline declares
/// gets a host pipeline for those formats ([`for_formats`]). On by
/// default; `VICTORY_TARGET_VARIANTS=0` sends such a draw to the host with
/// the title's pipeline, as before.
fn variants_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_TARGET_VARIANTS").is_none_or(|v| v != "0"))
}

/// The host pipeline for a draw with `p` into views of the formats
/// `bound`, which `p` does not declare (`host::formats_differ`): that of
/// the same shaders and state for those formats. It is a pipeline like the
/// title's own: queued at its first draw, made by a worker, waited for
/// within the frame's budget, and the draw is dropped while it is not
/// there ([`ready`]).
#[cold]
pub(crate) fn for_formats<'a>(p: &'a Arc<Pending>, bound: &TargetFormats) -> Option<&'a HostPipeline> {
    match p.graphics.as_deref() {
        Some(g) if variants_on() => ready(variant(p, g, &super::host::formats_for(&p.formats, bound))),
        _ => ready(p),
    }
}

/// The pipeline of `p`'s shaders and its state `g` for the formats
/// `bound`: found in the chain that starts at `p`, or added to it (logged
/// then: once per pipeline and set of formats).
fn variant<'a>(p: &'a Arc<Pending>, g: &GraphicsState, bound: &TargetFormats) -> &'a Arc<Pending> {
    let mut at = p;
    // (`bound` is what `host::formats_for` gives, and a state made for it
    // declares exactly it, so the walk ends at the link made for it; the
    // count is a bound all the same: past it the title's own pipeline.)
    for _ in 0..64 {
        let next = at.variant.get_or_init(|| {
            let so_far = VARIANTS.fetch_add(1, Ordering::Relaxed) + 1;
            tracing::info!(
                pipeline = ?p.shaders,
                declared = ?p.formats,
                bound = ?bound,
                so_far,
                "a pipeline is made again for the render target formats bound at its draw (eight render targets, then depth; VICTORY_TARGET_VARIANTS=0: the draw goes out with the title's pipeline)"
            );
            Arc::new(Pending::new(p.shaders.clone(), Some(Box::new(g.with_formats(bound)))))
        });
        // (A state made for `bound` declares exactly `bound`.)
        if next.formats == *bound {
            return next;
        }
        at = next;
    }
    p
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
///
/// Second tier, Dolphin's ubershader idea (a pipeline that draws anything
/// while the real one is made): with none of the same shape, one of the
/// same shaders, formats and blending whose depth and stencil writes differ
/// (`VICTORY_STAND_IN_LOOSE=0`: not taken). The object is drawn rather than
/// missing; for those frames it may write depth where it would not.
fn stand_in(p: &Pending) -> Option<&HostPipeline> {
    stand_in_tier(p, true)
}

/// Draws that took a same-shape stand-in before any wait
/// ([`stand_in_first`]), since the start.
pub(crate) static STAND_INS_FIRST: AtomicU64 = AtomicU64::new(0);

/// Whether a same-shape stand-in is taken before the frame waits for a
/// pipeline being made (`VICTORY_STAND_IN_FIRST=0`: after the wait, as
/// before). Only with stand-ins on.
fn stand_in_first() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        let on = stand_in_on() && std::env::var_os("VICTORY_STAND_IN_FIRST").is_none_or(|v| v != "0");
        if on {
            tracing::info!("a draw whose pipeline is being made takes a stand-in of the same shaders and shape at once, before any wait (VICTORY_STAND_IN_FIRST=0: after the wait)");
        }
        on
    })
}

/// [`stand_in`]; the second tier (other depth writes) only with `loose`.
fn stand_in_tier(p: &Pending, loose: bool) -> Option<&HostPipeline> {
    let shape = p.shape.as_ref().filter(|_| stand_in_on())?;
    let siblings = SIBLINGS.lock().unwrap_or_else(|e| e.into_inner());
    let siblings = siblings.as_ref()?;
    let made_of = |key: &String| -> Option<&'static HostPipeline> {
        let sibling: &'static Arc<Pending> = *siblings.get(key)?;
        sibling.made.get()?.as_ref()
    };
    if let Some(made) = made_of(shape) {
        STAND_INS.fetch_add(1, Ordering::Relaxed);
        return Some(made);
    }
    let loose = p.loose.as_ref().filter(|_| loose && loose_on())?;
    let made = made_of(loose)?;
    STAND_INS.fetch_add(1, Ordering::Relaxed);
    LOOSE_STAND_INS.fetch_add(1, Ordering::Relaxed);
    Some(made)
}

/// Draws that went out with a second-tier stand-in since the start.
pub(crate) static LOOSE_STAND_INS: AtomicU64 = AtomicU64::new(0);

/// See [`stand_in`].
fn loose_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_STAND_IN_LOOSE").is_none_or(|v| v != "0"))
}

/// Makes `p` on this thread if it is not made yet (waiting for a worker
/// that is making it); whether it has a host pipeline.
pub(crate) fn now(p: &Arc<Pending>) -> bool {
    p.make();
    p.made.get().is_some_and(|made| made.is_some())
}

/// At the title's creation of a pipeline: starts it at once if an earlier
/// run drew with it (during loading, so the world's first frames find it),
/// and (Try 6.13, edsuuu/VI-BR-PORT b9d4f96c) every compute pipeline: its
/// first dispatch waits for it, and a frame that first dispatches hundreds
/// of them (the world's) waited for each in turn. Most come out of the
/// pipeline library at once. `VICTORY_PIPELINE_EARLY_COMPUTE=0`: only the
/// ones an earlier run drew with, as before.
pub(crate) fn early(p: &Arc<Pending>) {
    static COMPUTE: OnceLock<bool> = OnceLock::new();
    // (Only with workers: without them a start is a make on the title's
    // own thread.)
    let compute = p.graphics.is_none()
        && workers()
        && *COMPUTE.get_or_init(|| std::env::var_os("VICTORY_PIPELINE_EARLY_COMPUTE").is_none_or(|v| v != "0"));
    if !p.shaders.is_empty() && (compute || super::library::wanted_early(&p.key)) {
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
    crate::framelog::add(crate::framelog::T::PipelineWaitUs, started.elapsed().as_micros() as u64);
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

    /// `pop_for(false)`.
    #[cfg(test)]
    fn pop(&mut self) -> Option<Arc<Pending>> {
        self.pop_for(false)
    }

    /// The next pipeline to make: light requested ones first, then heavy
    /// requested ones while a heavy slot is free, then those queued early
    /// (not with `asked_only`: the worker kept for requested ones,
    /// [`kept_worker`]). Of the requested ones, the one the most draws
    /// asked for so far ([`most_asked_first`]), the first queued of those.
    fn pop_for(&mut self, asked_only: bool) -> Option<Arc<Pending>> {
        fn requested(lane: &mut VecDeque<Arc<Pending>>) -> Option<Arc<Pending>> {
            if !most_asked_first() {
                return lane.pop_front();
            }
            let at = lane
                .iter()
                .enumerate()
                .max_by_key(|(i, p)| (p.asks.load(Ordering::Relaxed), std::cmp::Reverse(*i)))
                .map(|(i, _)| i)?;
            lane.remove(at)
        }
        fn early(lane: &mut VecDeque<Arc<Pending>>) -> Option<Arc<Pending>> {
            // An early entry can have been promoted and claimed already.
            std::iter::from_fn(|| lane.pop_front())
                .find(|p| p.queue_state.load(Ordering::Relaxed) == EARLY)
        }
        let heavy_free = self.heavy_making < self.heavy_limit;
        let p = requested(&mut self.requested)
            .or_else(|| heavy_free.then(|| requested(&mut self.heavy_requested)).flatten())
            .or_else(|| (!asked_only).then(|| early(&mut self.early)).flatten())
            .or_else(|| (heavy_free && !asked_only).then(|| early(&mut self.heavy_early)).flatten())?;
        ASKED_WAITING.store((self.requested.len() + self.heavy_requested.len()) as u64, Ordering::Relaxed);
        if p.heavy() {
            self.heavy_making += 1;
        }
        p.queue_state.store(MAKING, Ordering::Relaxed);
        Some(p)
    }
}

/// Whether requested pipelines are made most asked for first, not in the
/// order first asked for (`VICTORY_PIPELINE_MOST_ASKED=0`). A backlog of
/// 1,600 pipelines took minutes to work through; the pipeline of the
/// ground that 400 draws a frame wait for went behind a hundred that one
/// draw each asked for once.
fn most_asked_first() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_PIPELINE_MOST_ASKED").is_none_or(|v| v != "0"))
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
        ASKED_WAITING.store((queue.requested.len() + queue.heavy_requested.len()) as u64, Ordering::Relaxed);
        // All: the worker kept for requested pipelines ([`kept_worker`])
        // may be the one woken, and it leaves an early one where it is.
        AVAILABLE.notify_all();
    }
}

/// Requested pipelines (draws wait for them) left in the queue at the last
/// claim: what raises the workers above the title ([`worker_level`]).
static ASKED_WAITING: AtomicU64 = AtomicU64::new(0);

/// Whether worker `worker` of `workers` makes requested pipelines only:
/// the first, when there are three or more. At the intro every worker was
/// busy with pipelines started early from the last run's list (heavy ones
/// take a driver 10-40 s), so the one the intro's next draw asked for
/// waited for a worker while each frame waited up to
/// `VICTORY_PIPELINE_WAIT_MS` for it: the stutter at the first load.
/// `VICTORY_PIPELINE_KEEP_WORKER=0`: every worker takes any pipeline.
fn kept_worker(worker: usize, workers: usize) -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    worker == 0
        && workers >= 3
        && *ON.get_or_init(|| std::env::var_os("VICTORY_PIPELINE_KEEP_WORKER").is_none_or(|v| v != "0"))
}

/// The priority level of a worker: [`priority_level`] by the requested
/// backlog; pipelines only started early (no draw waits for them) raise the
/// workers to normal at most, so a long early list no longer runs above
/// the title's own threads while it plays its intro video (it stuttered
/// at 10-21 fps there with 84-230 early pipelines queued, 2026-11-25).
/// `VICTORY_PIPELINE_EARLY_BOOST=1`: every queued pipeline counts, as before.
fn worker_level(worker: usize, workers: usize) -> u8 {
    static ALL: OnceLock<bool> = OnceLock::new();
    let waiting = WAITING.load(Ordering::Relaxed);
    if *ALL.get_or_init(|| std::env::var_os("VICTORY_PIPELINE_EARLY_BOOST").is_some_and(|v| v == "1")) {
        return priority_level(waiting, worker, workers);
    }
    priority_level(ASKED_WAITING.load(Ordering::Relaxed), worker, workers).max(priority_level(waiting, worker, workers).min(1))
}

fn next(asked_only: bool) -> Arc<Pending> {
    let mut queue = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if let Some(p) = queue.pop_for(asked_only) {
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
        // Try 6.12 (all-out mode, as lepotane/VIctory-leonida's launcher):
        // `VICTORY_PIPELINE_WORKERS` sets how many (1 to 32) and
        // `VICTORY_PIPELINE_HEAVY` how many of them take heavy pipelines at
        // once (1 to 8; more than 3 ran the driver out of memory before).
        let env = |name: &str| std::env::var(name).ok().and_then(|v| v.trim().parse::<usize>().ok());
        let workers = env("VICTORY_PIPELINE_WORKERS").map_or_else(
            || std::thread::available_parallelism().map_or(2, |n| (n.get() * 2 / 3).clamp(2, 10)),
            |n| n.clamp(1, 32),
        );
        let heavy =
            env("VICTORY_PIPELINE_HEAVY").map_or((workers / 2).clamp(1, 3), |n| n.clamp(1, 8).min(workers));
        QUEUE.lock().unwrap_or_else(|e| e.into_inner()).heavy_limit = heavy;
        tracing::info!(workers, heavy, "pipeline workers");
        for i in 0..workers {
            let spawned = std::thread::Builder::new()
                .name(format!("victory-pipelines-{i}"))
                .stack_size(64 << 20)
                .spawn(move || {
                    let mut raised = None;
                    loop {
                        let p = next(kept_worker(i, workers));
                        // Below the title's own threads (loading, recording)
                        // unless a backlog waits: the title then runs at
                        // full load and starved the workers (4-9 pipelines a
                        // minute in the world on an i5-12400F; see
                        // `priority_level`).
                        let level = worker_level(i, workers);
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
                            super::library::save_soon();
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
    super::library::save_soon();
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

    /// A draw into other formats than its pipeline declares finds the
    /// pipeline for them, which exists once per set of formats and is
    /// queued by its first draw like any other.
    #[test]
    fn a_pipeline_for_bound_formats_is_made_once() {
        // The title's pipeline declares one render target, format 24.
        let p = Arc::new(Pending::new(Vec::new(), Some(Box::new(GraphicsState::declaring(24)))));
        assert_eq!(p.formats, [24, 0, 0, 0, 0, 0, 0, 0, 0]);
        let g = p.graphics.as_deref().unwrap();
        let (a, b) = ([26, 0, 0, 0, 0, 0, 0, 0, 0], [28, 0, 0, 0, 0, 0, 0, 0, 0]);
        let first = variant(&p, g, &a);
        assert_eq!(first.formats, a);
        assert_ne!(first.key, p.key);
        assert!(Arc::ptr_eq(first, variant(&p, g, &a)));
        let second = variant(&p, g, &b);
        assert_eq!(second.formats, b);
        assert!(!Arc::ptr_eq(first, second));
        assert_ne!(second.key, first.key);
        assert!(Arc::ptr_eq(first, variant(&p, g, &a)));
        assert!(Arc::ptr_eq(second, variant(&p, g, &b)));
        assert_eq!(first.queue_state.load(Ordering::Relaxed), NOT_QUEUED);
        assert_eq!(p.queue_state.load(Ordering::Relaxed), NOT_QUEUED);
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
