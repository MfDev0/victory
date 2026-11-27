//! Pipeline states. Both descriptions start as on PC: the root signature,
//! then the shader stages (five for graphics, one for compute); streams
//! carry the rest of the graphics state as subobjects of the standard
//! types.
//!
//! Host pipelines are made on a pipeline's first draw, not at creation:
//! The title creates some 60,000 pipelines in its first 20
//! seconds and draws with a few dozen (decision 0007, step 3). Shader
//! containers are shared by content hash and translated once, on first
//! use (decision 0003).
//!
//! shortcut: only vertex plus pixel pipelines from streams and compute
//! pipelines get host pipelines; mesh, tessellation and geometry stages,
//! and the plain graphics description, keep their containers only. The 32-byte
//! packet after the header stays zero: titles only copy it inline, which
//! our null command-list limits keep from running.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::collections::HashMap;
use std::mem::offset_of;
use std::sync::{Arc, Mutex, OnceLock};

use crate::object::{self, E_INVALIDARG, HRESULT, Iid, Object, PIPELINE, Table, This, guid};
use crate::resource::PAGEABLE_IIDS;

const PIPELINE_IIDS: [Iid; 4] = [
    guid(
        0x765a30f3,
        0xf624,
        0x4c6f,
        [0xa8, 0x28, 0xac, 0xe9, 0x48, 0x62, 0x24, 0x45],
    ),
    PAGEABLE_IIDS[0],
    PAGEABLE_IIDS[1],
    PAGEABLE_IIDS[2],
];

#[repr(C)]
pub(crate) struct Bytecode {
    code: *const u8,
    size: usize,
}

#[repr(C)]
pub(crate) struct GraphicsHead {
    root: This,
    /// Vertex, pixel, domain, hull, geometry.
    stages: [Bytecode; 5],
}

#[repr(C)]
pub(crate) struct ComputeHead {
    root: This,
    stage: Bytecode,
}

/// A shader container a title passed, shared by every pipeline with the
/// same bytes.
pub(crate) struct Shader {
    pub(crate) stage: &'static str,
    /// The first 8 bytes of its BLAKE3, as logged.
    pub(crate) hash: String,
    container: Vec<u8>,
    /// Its result gets the least indirect light ([`crate::ambient`]).
    #[cfg(windows)]
    pub(crate) resolves_light: bool,
    /// A pixel program of the title's decoration bake ([`bakes`]).
    #[cfg(windows)]
    pub(crate) bakes: bool,
    #[cfg(windows)]
    translated: OnceLock<Option<Translated>>,
    /// Whether its code walks the ray-tracing structures, once looked at
    /// ([`Shader::walks`]).
    #[cfg(windows)]
    walks: OnceLock<bool>,
}

impl std::fmt::Debug for Shader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.stage, self.hash)
    }
}

/// Every container seen, by full content hash.
static SHADERS: Mutex<Option<HashMap<[u8; 32], Arc<Shader>>>> = Mutex::new(None);

/// For the memory lines: containers kept and their bytes, translations
/// kept and the bytes of their host code. None of them is ever let go.
static KEPT: [std::sync::atomic::AtomicU64; 4] = [const { std::sync::atomic::AtomicU64::new(0) }; 4];

/// What the shader table and the pipeline library keep in memory:
/// containers and their bytes, translations and their bytes, the bytes of
/// the library's file (kept whole while the library lives) and what its
/// last save wrote.
#[cfg(windows)]
#[cfg_attr(not(feature = "diagnostics"), allow(dead_code))]
pub(crate) fn kept() -> [u64; 7] {
    use std::sync::atomic::Ordering::Relaxed;
    [
        KEPT[0].load(Relaxed),
        KEPT[1].load(Relaxed),
        KEPT[2].load(Relaxed),
        KEPT[3].load(Relaxed),
        library::BLOB.load(Relaxed),
        library::SAVED.load(Relaxed),
        library::TRIMMED_BYTES.load(Relaxed),
    ]
}

/// The shared container for `bytes`.
fn shader(stage: &'static str, bytes: &[u8]) -> Arc<Shader> {
    let full = blake3::hash(bytes);
    let mut shaders = SHADERS.lock().unwrap_or_else(|e| e.into_inner());
    shaders
        .get_or_insert_with(HashMap::new)
        .entry(*full.as_bytes())
        .or_insert_with(|| {
            KEPT[0].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            KEPT[1].fetch_add(bytes.len() as u64, std::sync::atomic::Ordering::Relaxed);
            Arc::new(Shader {
                stage,
                hash: full.to_hex()[..16].to_owned(),
                container: bytes.to_vec(),
                #[cfg(windows)]
                resolves_light: bytes
                    .windows(crate::ambient::PROGRAM.len())
                    .any(|w| w == crate::ambient::PROGRAM),
                #[cfg(windows)]
                bakes: stage == "pixel" && bakes(bytes),
                #[cfg(windows)]
                translated: OnceLock::new(),
                #[cfg(windows)]
                walks: OnceLock::new(),
            })
        })
        .clone()
}

/// A translated shader and the console resources its code reads.
#[cfg(windows)]
pub(crate) struct Translated {
    pub(crate) host: victory_shader::emit::HostShader,
    pub(crate) used: Vec<victory_shader::UsedResource>,
    /// Whether the shader's code can store to buffer memory ([`stores`]);
    /// true when its code is not known.
    pub(crate) stores: bool,
    /// A compute shader's group size (`numthreads`); `[0; 3]` for other
    /// stages. A `DispatchX` that counts threads is made groups with it.
    pub(crate) threads: [u32; 3],
}

#[cfg(windows)]
impl Shader {
    /// The container's size in bytes.
    pub(crate) fn size(&self) -> usize {
        self.container.len()
    }

    /// The translation, made on first use; a shader that does not
    /// translate is logged once.
    pub(crate) fn translated(&self) -> Option<&Translated> {
        self.translated
            .get_or_init(|| {
                let done = translate(&self.container);
                // `VICTORY_SHADER_SAVE=<dir>`: each shader as the title gave it (`.cdxil`) and as
                // translated (`.dxil`), by the hash crash reports name it with
                if let Some(dir) = std::env::var_os("VICTORY_SHADER_SAVE") {
                    let dir = std::path::PathBuf::from(dir);
                    let _ = std::fs::create_dir_all(&dir);
                    let stem = format!("{}.{}", self.hash, self.stage);
                    let _ = std::fs::write(dir.join(format!("{stem}.cdxil")), &self.container);
                    if let Ok(t) = &done {
                        let _ = std::fs::write(dir.join(format!("{stem}.dxil")), &t.host.dxil);
                    }
                }
                if let Ok(t) = &done {
                    KEPT[2].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    KEPT[3].fetch_add(t.host.dxil.len() as u64, std::sync::atomic::Ordering::Relaxed);
                }
                match &done {
                    Ok(t) => tracing::info!(
                        stage = self.stage,
                        hash = %self.hash,
                        user_data = t.host.user_data.len(),
                        samplers = t.host.samplers.len(),
                        "shader translated"
                    ),
                    Err(e) => {
                        tracing::warn!(
                            stage = self.stage,
                            hash = %self.hash,
                            error = %e,
                            "shader does not translate; its pipelines' draws are dropped"
                        );
                        // `VICTORY_DUMP_FAILED=<dir>`: only the containers of
                        // shaders that do not translate, to study offline
                        // (`victory translate`).
                        if let Some(dir) = std::env::var_os("VICTORY_DUMP_FAILED") {
                            let dir = std::path::PathBuf::from(dir);
                            let file = dir.join(format!("{}-{}.dxbc", self.stage, self.hash));
                            let written = std::fs::create_dir_all(&dir)
                                .and_then(|()| std::fs::write(&file, &self.container));
                            if let Err(e) = written {
                                tracing::warn!(error = %e, file = %file.display(), "shader not dumped");
                            }
                        }
                    }
                }
                done.ok()
            })
            .as_ref()
    }
}

#[cfg(windows)]
fn translate(container: &[u8]) -> Result<Translated, String> {
    use victory_shader::{Container, FourCC, HwStage};
    // The translation depends on it (a view index past a capped heap).
    crate::descriptor::host::probe_view_limit();
    let c = Container::parse(container).map_err(|e| e.to_string())?;
    let programs = match c.part(FourCC::DXIL) {
        Some(part) => victory_shader::native_programs(part.data).map_err(|e| e.to_string())?,
        None => Vec::new(),
    };
    let host = crate::shader_cache::host_shader(container)?;
    let stores = programs.is_empty() || programs.iter().any(stores);
    let main = programs.into_iter().find(|p| {
        matches!(
            p.stage,
            HwStage::Vs | HwStage::Ps | HwStage::Ms | HwStage::Cs
        )
    });
    let threads = match main.as_ref().map(|p| &p.io) {
        Some(victory_shader::native::StageIo::Cs(cs)) => cs.threads,
        _ => [0; 3],
    };
    let used = main.map(|p| p.used).unwrap_or_default();
    Ok(Translated { host, used, stores, threads })
}

/// The compute pipelines of the console's ray-tracing structure builder
/// (`XboxBvhBuilder.dll`; its programs are the ones with a spilled
/// user-data table) run **all or none**. Its stages feed each other
/// through memory: a stage that runs on what a dropped stage never wrote
/// computes its store offsets from garbage. The structures lie in plain
/// title memory, between the title's own objects, and a root view has no
/// size, so those stores land on the neighbours (run 98, with 14 of 35
/// stages translated: the title died within a second of the first builder
/// submission, on pointers that had become bounding-box bit patterns).
///
/// So a builder dispatch runs only while every builder pipeline the title
/// has created has a host pipeline; the first builder dispatch makes them
/// all on the spot instead of over the next frames (a stage "still being
/// made" would be a dropped stage). One that cannot be made turns the
/// builder off for the rest of the run.
///
/// shortcut: and only with `VICTORY_BVH_BUILDER=1`, until a run has shown
/// the whole builder and the host side of title memory by address
/// (`memory.rs`) working on a card; nothing reads the structures before
/// the programs that walk them translate.
#[cfg(windows)]
struct Builder {
    pipelines: Vec<Arc<make::Pending>>,
    /// Every one of `pipelines` has a host pipeline.
    whole: bool,
    /// One could not be made.
    off: bool,
}

#[cfg(windows)]
static BUILDER: Mutex<Builder> = Mutex::new(Builder {
    pipelines: Vec::new(),
    whole: false,
    off: false,
});

/// Whether `shaders` are a pipeline of the structure builder.
#[cfg(windows)]
fn of_builder(shaders: &[Arc<Shader>]) -> bool {
    use victory_shader::{Container, FourCC};
    let [shader] = shaders else { return false };
    if shader.stage != "compute" {
        return false;
    }
    let Ok(c) = Container::parse(&shader.container) else {
        return false;
    };
    c.part(FourCC::DXIL)
        .and_then(|part| victory_shader::native_programs(part.data).ok())
        .is_some_and(|programs| programs.iter().any(victory_shader::spill::is_spilled))
}

/// Whether a compute container is one of the title's passes that only
/// serve rays (Try 6.12, ported from edsuuu/VI-BR-PORT ab4571df, 1556de82,
/// f1e5e454 and 24f8d2ed, GPL-3.0-or-later; their last two after
/// codec-d/cool_fox `make.rs`):
///
/// - `bvh_skinning`: all 30 entry points named `CSComputeSkinning*` pose
///   every animated mesh once more, only for the ray-tracing structures,
///   and `bvh_skinning_smh`'s `CSGenerateHairVB` makes the hair's geometry
///   for them. They were the largest share of the card's time in a cutscene
///   there (Series S, RTX 4060 Ti 8 GB: 6.8 -> 10.2 fps). The picture's own
///   skinning is `CS_PedStandardSkinning*`, `CS_PosOnlyLodSkinning*` and the
///   cloth's `computeclothSkinning`: they run.
/// - `CS_RaytracingResolve*` shades what a ray hit (some 1,000 programs).
/// - `CS_ScreenProbes_ActivateRays*` picks the screen probes that cast rays.
/// - The tracing against the structures: the screen probes'
///   `CS_ScreenProbes_TraceRays*_BVH*` and the reflections'
///   `CS_RaytraceVBuffer*`. Their `Fallback` and `Voxels` forms march
///   through the title's voxels, not the structures, and light the
///   picture: they run.
///
/// The container holds its entry point's name, not the effect's.
#[cfg(windows)]
fn for_rays(bytes: &[u8]) -> RayOnly {
    let has = |name: &[u8]| bytes.windows(name.len()).any(|w| w == name);
    if has(b"CSComputeSkinning") || has(b"CSGenerateHairVB") {
        RayOnly::Skin
    } else if has(b"CS_RaytracingResolve") {
        RayOnly::Resolve
    } else if has(b"CS_ScreenProbes_ActivateRays") {
        RayOnly::Activate
    } else if (has(b"CS_ScreenProbes_TraceRays") && has(b"_BVH"))
        || (has(b"CS_RaytraceVBuffer") && !has(b"Voxels"))
    {
        RayOnly::Trace
    } else {
        RayOnly::No
    }
}

/// What a compute pipeline does only for rays ([`for_rays`]).
#[cfg(windows)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RayOnly {
    No,
    Skin,
    Resolve,
    Activate,
    Trace,
}

#[cfg(all(test, windows))]
#[test]
fn ray_programs_are_known_by_name() {
    assert_eq!(for_rays(b"..\0CSComputeSkinningRigidVelocity_1c34db0a_Wrapped\0.."), RayOnly::Skin);
    assert_eq!(for_rays(b"..\0CS_PedStandardSkinning_3_00aa11bb_Wrapped\0.."), RayOnly::No);
    assert_eq!(for_rays(b"computeclothSkinning"), RayOnly::No);
    assert_eq!(for_rays(b"..\0CS_RaytracingResolve_Opaque_12345678_Wrapped\0.."), RayOnly::Resolve);
    assert_eq!(for_rays(b"..\0CS_ScreenProbes_ActivateRays_0_Wrapped\0.."), RayOnly::Activate);
    assert_eq!(for_rays(b"..\0CSGenerateHairVB_c88a8f19_Wrapped\0.."), RayOnly::Skin);
    assert_eq!(for_rays(b"..\0CS_ScreenProbes_TraceRays_RayBinning_BVH_Mixed\0.."), RayOnly::Trace);
    assert_eq!(for_rays(b"..\0CS_ScreenProbes_TraceRays_Voxels\0.."), RayOnly::No);
    assert_eq!(for_rays(b"..\0CS_ScreenProbes_TraceRays_Fallback\0.."), RayOnly::No);
    assert_eq!(for_rays(b"..\0CS_RaytraceVBuffer_ShaderBinning_WithFar\0.."), RayOnly::Trace);
    assert_eq!(for_rays(b"..\0CS_RaytraceVBuffer_Voxels_RayBinning\0.."), RayOnly::No);
    // The ambient stand-in's program is not one of them.
    assert_eq!(for_rays(b"..\0CS_ScreenProbes_TemporalResolve_0\0.."), RayOnly::No);
}

/// Whether a dispatch of `this` is left out because it only serves rays
/// and nothing they could hit is there: every ray misses
/// (`VICTORY_SKIP_RT=1`) and the structure builder is off. Each kind runs
/// anyway with its switch: `VICTORY_RT_SKIN=1`, `VICTORY_RT_RESOLVE=1`,
/// `VICTORY_RT_TRACE=1`, `VICTORY_ACTIVATE_RAYS=1`.
#[cfg(windows)]
pub(crate) fn only_for_rays(this: This) -> bool {
    !this.is_null() && rays_only_off(pipeline(this).rt_only)
}

/// Whether passes of `kind` are left out ([`only_for_rays`]).
#[cfg(windows)]
fn rays_only_off(kind: RayOnly) -> bool {
    fn off(switch: &str) -> bool {
        std::env::var_os("VICTORY_SKIP_RT").is_some_and(|v| v == "1")
            && std::env::var_os("VICTORY_BVH_BUILDER").is_none_or(|v| v != "1")
            && std::env::var_os(switch).is_none_or(|v| v != "1")
    }
    static SKIN: OnceLock<bool> = OnceLock::new();
    static RESOLVE: OnceLock<bool> = OnceLock::new();
    static ACTIVATE: OnceLock<bool> = OnceLock::new();
    static TRACE: OnceLock<bool> = OnceLock::new();
    match kind {
        RayOnly::No => false,
        RayOnly::Skin => *SKIN.get_or_init(|| off("VICTORY_RT_SKIN")),
        RayOnly::Resolve => *RESOLVE.get_or_init(|| off("VICTORY_RT_RESOLVE")),
        RayOnly::Activate => *ACTIVATE.get_or_init(|| off("VICTORY_ACTIVATE_RAYS")),
        RayOnly::Trace => *TRACE.get_or_init(|| off("VICTORY_RT_TRACE")),
    }
}

/// The ray-only kind of a pipeline of `shaders` ([`for_rays`] of its
/// compute container).
#[cfg(windows)]
fn rt_only_of(shaders: &[Arc<Shader>]) -> RayOnly {
    shaders
        .iter()
        .filter(|s| s.stage == "compute")
        .map(|s| for_rays(&s.container))
        .find(|k| *k != RayOnly::No)
        .unwrap_or(RayOnly::No)
}

/// Whether a pipeline of `shaders` is not built at all: its dispatches
/// are left out anyway ([`only_for_rays`]), and its driver compile (some
/// 1,000 `CS_RaytracingResolve` programs among them) was time the
/// pipeline workers spent before and while the game ran.
#[cfg(windows)]
pub(crate) fn never_built(shaders: &[Arc<Shader>]) -> bool {
    rays_only_off(rt_only_of(shaders))
}

/// `VICTORY_BVH_QUARANTINE=1`: what the structure builder's stages store
/// stays in the host's copies and is not copied back into title memory. A
/// check for a first run of the whole builder: a stage that stores where
/// it should not can then not reach the title's own objects (run 98), and
/// the programs that walk the structures read the host's copies anyway.
/// shortcut: not a way to run: the title also reads results back (sizes,
/// counters) and gets none, and a copy's pages are overwritten from title
/// memory when the title writes them.
#[cfg(windows)]
pub(crate) fn builder_quarantined(this: This) -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    is_builder(this) && *ON.get_or_init(|| std::env::var_os("VICTORY_BVH_QUARANTINE").is_some_and(|v| v == "1"))
}

#[cfg(windows)]
impl Shader {
    /// Whether the program walks the ray-tracing structures: its code has
    /// an `image_bvh_intersect_ray` or `image_bvh64_intersect_ray`. Looked
    /// at once, when a dispatch first asks.
    fn walks(&self) -> bool {
        *self.walks.get_or_init(|| container_walks(&self.container))
    }
}

/// Whether a console container's code tests rays against the nodes of a
/// ray-tracing structure.
#[cfg(windows)]
fn container_walks(container: &[u8]) -> bool {
    use victory_shader::{Container, FourCC};
    let Ok(c) = Container::parse(container) else {
        return false;
    };
    c.part(FourCC::DXIL)
        .and_then(|part| victory_shader::native_programs(part.data).ok())
        .is_some_and(|programs| programs.iter().any(|p| code_walks(&p.code)))
}

/// Whether native code has a ray-tracing node test.
#[cfg(windows)]
fn code_walks(code: &[u8]) -> bool {
    victory_shader::gfx10::decode(code).is_ok_and(|code| code.iter().any(|i| i.name().is_some_and(|n| n.starts_with("image_bvh"))))
}

/// Dispatches held back by [`walk_held`] since the last frame line.
#[cfg(windows)]
pub(crate) static WALKS_HELD: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Whether a dispatch with the title's pipeline `this` is held back
/// because its program walks ray-tracing structures nothing has built
/// (`VICTORY_BVH_HOLD=1`; off by default).
///
/// The structure builder does not run (see [`Builder`]: only with
/// `VICTORY_BVH_BUILDER=1`, and then only whole, which a host of 32-lane
/// waves does not manage yet). The title's ray-traced passes run all the
/// same: 64 programs of its shader pack test rays against structure nodes
/// (ray-traced shadows, screen probes, reflections, the irradiance cache;
/// one of them, `CS_IrradianceCache_ShadowTrace_WithFar` f607780aa8e502d0,
/// from the first world frame on in every run). Each walks, in a loop the
/// title bounds at 65,535 steps a wave, whatever bytes lie where the
/// structure would be: what it returns is not a picture of the scene, and
/// how long it runs depends on those bytes. Upstream's pull request 2311
/// measured a card hung in exactly that dispatch on an NVIDIA card, every
/// run, and both card losses of ours since the draw windows' own fault was
/// fixed (runs 208 and 224) came within a second of the first use of the
/// programs that shade those rays' hits (`CS_RaytracingResolve`).
///
/// With the switch such a dispatch is dropped until the builder has run
/// whole. What the passes would have written stays as the title left it
/// (their targets are not cleared by us): the picture has to be looked at.
#[cfg(windows)]
pub(crate) fn walk_held(this: This) -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    if this.is_null() || !*ON.get_or_init(|| std::env::var_os("VICTORY_BVH_HOLD").is_some_and(|v| v == "1")) {
        return false;
    }
    let p = pipeline(this);
    if !p.shaders.iter().any(|s| s.stage == "compute" && s.walks()) {
        return false;
    }
    // (Asked only for the few programs that walk: the lock is the builder's.)
    if BUILDER.lock().unwrap_or_else(|e| e.into_inner()).whole {
        return false;
    }
    if WALKS_HELD.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            tracing::info!(
                shaders = ?p.shaders,
                "a dispatch of a program that walks ray-tracing structures is held back: the structure builder has not                  built them (VICTORY_BVH_HOLD=1); the first one"
            );
        });
    }
    true
}

/// Whether the title's pipeline `this` is one of the structure builder's.
#[cfg(windows)]
pub(crate) fn is_builder(this: This) -> bool {
    !this.is_null() && pipeline(this).builder
}

/// A stage of the builder was dropped after all: none runs from here on.
#[cfg(windows)]
pub(crate) fn builder_off(why: &'static str) {
    let mut b = BUILDER.lock().unwrap_or_else(|e| e.into_inner());
    if !b.off {
        b.off = true;
        tracing::warn!(why, "the ray-tracing structure builder is turned off for the rest of the run: its stages run all or none");
    }
}

/// At a dispatch of a builder pipeline: whether the builder runs (see
/// [`Builder`]).
#[cfg(windows)]
pub(crate) fn builder_runs() -> bool {
    static ALLOWED: OnceLock<bool> = OnceLock::new();
    // On unless `VICTORY_BVH_BUILDER=0` (since 2026-11-27: the ray-tracing
    // programs are to be used).
    if !*ALLOWED.get_or_init(|| std::env::var_os("VICTORY_BVH_BUILDER").is_none_or(|v| v != "0")) {
        return false;
    }
    let mut b = BUILDER.lock().unwrap_or_else(|e| e.into_inner());
    if b.off {
        return false;
    }
    if b.whole {
        return true;
    }
    let missing = b.pipelines.iter().filter(|p| !make::now(p)).count();
    if missing > 0 {
        b.off = true;
        for p in b.pipelines.iter().filter(|p| !make::now(p)) {
            tracing::warn!(shaders = ?make::shaders(p), "a builder pipeline with no host pipeline");
        }
        tracing::warn!(
            missing,
            of = b.pipelines.len(),
            "the ray-tracing structure builder stays off: not all of its programs have a host pipeline, and its stages run all or none"
        );
        return false;
    }
    tracing::info!(
        pipelines = b.pipelines.len(),
        "the ray-tracing structure builder runs: all of its programs have host pipelines"
    );
    b.whole = true;
    true
}

/// Whether a console program can store to buffer memory: it has a buffer,
/// typed-buffer, scalar-memory, flat or global store or atomic, or code
/// that is not all known (an instruction of those kinds outside our table,
/// an indirect jump, code that does not decode). Stores to textures
/// (image instructions) and to the program's own scratch memory do not
/// count: neither reaches a buffer's host copy. After upstream #413,
/// which leaves scalar-memory stores out and counts image stores.
#[cfg(windows)]
fn stores(program: &victory_shader::NativeProgram) -> bool {
    use victory_shader::gfx10::{Encoding, decode};
    let Ok(code) = decode(&program.code) else {
        return true;
    };
    code.iter().any(|i| {
        let name = i.name();
        match i.encoding {
            Encoding::Mubuf
            | Encoding::Mtbuf
            | Encoding::Smem
            | Encoding::Flat
            | Encoding::Global => name.is_none_or(|n| {
                (n.contains("store") || n.contains("atomic")) && !n.contains("scratch")
            }),
            Encoding::Sop1 => name.is_none_or(|n| matches!(n, "s_setpc_b64" | "s_swappc_b64")),
            _ => false,
        }
    })
}

#[repr(C)]
struct Pipeline {
    object: Object,
    packet: [u32; 8],
    /// Each used stage's container.
    shaders: Vec<Arc<Shader>>,
    /// Whether a list has bound it yet (its shaders are logged then).
    bound: std::sync::atomic::AtomicBool,
    /// What its host pipeline is made from, and the host pipeline once made
    /// (by a worker: the title may release the pipeline meanwhile).
    #[cfg(windows)]
    host: Arc<make::Pending>,
    /// One of the ray-tracing structure builder's ([`Builder`]).
    #[cfg(windows)]
    builder: bool,
    /// One of the title's passes that only serve rays ([`for_rays`]).
    #[cfg(windows)]
    rt_only: RayOnly,
}
const _: () = assert!(offset_of!(Pipeline, packet) == 0x10);

/// Stage names, in description order.
const GRAPHICS_STAGES: [&str; 5] = ["vertex", "pixel", "domain", "hull", "geometry"];

fn pipeline<'a>(this: This) -> &'a Pipeline {
    // SAFETY: the title passes one of our pipelines.
    unsafe { &*this.cast::<Pipeline>() }
}

/// List method: logs each pipeline's shaders the first time a list binds
/// it, so a run lists the shaders its frames need first.
pub(crate) fn bound(this: This) {
    if this.is_null() {
        return;
    }
    let p = pipeline(this);
    if p.bound.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let frame = crate::frame::current();
    for shader in &p.shaders {
        tracing::info!(pipeline = ?this, frame, stage = shader.stage, size = shader.container.len(), hash = %shader.hash, "first bind");
    }
}

/// The host pipeline of a title pipeline; `None` if one of its shaders does
/// not translate, the host refuses it, or it is still being made (its first
/// use starts that, see [`make`]).
#[cfg(windows)]
pub(crate) fn host_pipeline<'a>(this: This) -> Option<&'a host::HostPipeline> {
    if this.is_null() {
        return None;
    }
    make::ready(&pipeline(this).host)
}

/// As [`host_pipeline`], for a draw into render target and depth views of
/// the formats `bound`: if a bound view has another format than the title's
/// pipeline declares for its slot (`host::formats_differ`), the host
/// pipeline of the same shaders and state made for the bound formats
/// (`make::for_formats`). A draw whose views are as declared pays one
/// comparison of the nine words.
#[cfg(windows)]
pub(crate) fn host_pipeline_for<'a>(this: This, bound: &host::TargetFormats) -> Option<&'a host::HostPipeline> {
    if this.is_null() {
        return None;
    }
    let p = &pipeline(this).host;
    if p.formats != *bound && host::formats_differ(&p.formats, bound) {
        return make::for_formats(p, bound);
    }
    make::ready(p)
}

/// Host pipelines queued and not made yet.
#[cfg(windows)]
pub(crate) fn making() -> u64 {
    make::waiting()
}

/// Draws that went out with a stand-in while their pipeline was being made.
#[cfg(windows)]
pub(crate) fn stand_ins() -> u64 {
    make::STAND_INS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Of [`stand_ins`], those of the second tier (other depth writes).
#[cfg(windows)]
pub(crate) fn loose_stand_ins() -> u64 {
    make::LOOSE_STAND_INS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Draws that took a same-shape stand-in before any wait (Try 6.15,
/// `VICTORY_STAND_IN_FIRST`); counted apart from [`stand_ins`].
#[cfg(windows)]
pub(crate) fn stand_ins_first() -> u64 {
    make::STAND_INS_FIRST.load(std::sync::atomic::Ordering::Relaxed)
}

/// Draws and dispatches dropped since the last [`take_drops`], by reason.
/// Kept in every build (4.8): the frame lines said how many draws a frame
/// were dropped "without a host pipeline", 166 to 2,546 in the user's runs,
/// but only a diagnostics build said which programs they were.
#[cfg(windows)]
static DROPS: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);

/// Shader hashes as printed in the pipeline diagnostics.
#[cfg(all(windows, feature = "diagnostics"))]
fn parse_skip_hashes(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|hash| !hash.is_empty())
        .filter_map(|hash| {
            if hash.len() == 16 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                Some(hash.to_ascii_lowercase())
            } else {
                tracing::warn!(
                    hash,
                    "VICTORY_SKIP_PIPELINES expects 16-digit shader hashes"
                );
                None
            }
        })
        .collect()
}

#[cfg(all(windows, feature = "diagnostics"))]
fn skipped_shader(p: &Pipeline) -> Option<&str> {
    static HASHES: OnceLock<Vec<String>> = OnceLock::new();
    let hashes = HASHES.get_or_init(|| {
        let value = std::env::var("VICTORY_SKIP_PIPELINES").unwrap_or_default();
        parse_skip_hashes(&value)
    });
    if hashes.is_empty() {
        return None;
    }
    p.shaders
        .iter()
        .find(|shader| hashes.contains(&shader.hash))
        .map(|shader| shader.hash.as_str())
}

#[cfg(all(windows, feature = "diagnostics"))]
pub(crate) fn is_skipped(this: This) -> bool {
    !this.is_null() && skipped_shader(pipeline(this)).is_some()
}

/// Counts a draw or dispatch of `this` intentionally skipped or dropped
/// for want of a host pipeline, or with one that could not be bound.
#[cfg(windows)]
pub(crate) fn note_drop(this: This) {
    let why = if this.is_null() {
        "no pipeline set".to_owned()
    } else {
        let p = pipeline(this);
        #[cfg(feature = "diagnostics")]
        let skipped = skipped_shader(p).map(|hash| format!("skipped shader {hash}"));
        #[cfg(not(feature = "diagnostics"))]
        let skipped: Option<String> = None;
        skipped.or_else(|| make::missing(&p.host)).unwrap_or_else(|| {
            format!("its host pipeline is there but the draw could not be bound or run {:?}", p.shaders)
        })
    };
    note_why(why);
}

/// Counts a draw or dispatch dropped for `why`.
#[cfg(windows)]
pub(crate) fn note_why(why: String) {
    #[cfg(feature = "diagnostics")]
    crate::capture::op(0, || format!("DROPPED {why}"));
    let mut drops = DROPS.lock().unwrap_or_else(|e| e.into_inner());
    *drops.get_or_insert_with(HashMap::new).entry(why).or_default() += 1;
}

/// The drops counted since the last call, most first.
#[cfg(windows)]
pub(crate) fn take_drops() -> Vec<(String, u64)> {
    let drops = DROPS.lock().unwrap_or_else(|e| e.into_inner()).take();
    let mut drops: Vec<(String, u64)> = drops.into_iter().flatten().collect();
    // Frame reports only show the first 40 reasons. Keep ties ordered so
    // a HashMap iteration order change cannot change which reasons appear.
    drops.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    drops
}

/// The stage's container, if it has one.
fn copy(stage: &'static str, code: &Bytecode) -> Option<Arc<Shader>> {
    if code.code.is_null() || code.size == 0 {
        return None;
    }
    // SAFETY: the title passes `size` bytes of shader code.
    Some(shader(stage, unsafe {
        std::slice::from_raw_parts(code.code, code.size)
    }))
}

fn hand_out(
    shaders: Vec<Arc<Shader>>,
    #[cfg(windows)] graphics: Option<Box<host::GraphicsState>>,
    iid: *const Iid,
    out: *mut This,
) -> HRESULT {
    tracing::trace!(?shaders, "pipeline state");
    #[cfg(windows)]
    let host = Arc::new(make::Pending::new(shaders.clone(), graphics));
    #[cfg(windows)]
    make::early(&host);
    #[cfg(windows)]
    let builder = of_builder(&shaders);
    #[cfg(windows)]
    if builder {
        // Another stage: the group is looked at again at its next dispatch.
        let mut b = BUILDER.lock().unwrap_or_else(|e| e.into_inner());
        b.pipelines.push(host.clone());
        b.whole = false;
        // Named once each (gfx41): which programs are the builder's, to
        // match against the refused and untranslated ones.
        tracing::info!(n = b.pipelines.len(), ?shaders, "a pipeline of the ray-tracing structure builder");
    }
    let pipeline = Box::new(Pipeline {
        object: Object::new(&FUNCTIONS),
        packet: [0; 8],
        #[cfg(windows)]
        host,
        #[cfg(windows)]
        builder,
        #[cfg(windows)]
        rt_only: rt_only_of(&shaders),
        shaders,
        bound: std::sync::atomic::AtomicBool::new(false),
    });
    object::hand_out(PIPELINE, pipeline, iid, &PIPELINE_IIDS, out)
}

/// Device method.
pub(crate) extern "system" fn create_graphics(
    _: This,
    desc: *const GraphicsHead,
    iid: *const Iid,
    out: *mut This,
) -> HRESULT {
    // SAFETY: the title passes a description (null is tolerated).
    let Some(desc) = (unsafe { desc.as_ref() }) else {
        return E_INVALIDARG;
    };
    let stages = GRAPHICS_STAGES.iter().zip(&desc.stages);
    let shaders = stages.filter_map(|(s, c)| copy(s, c)).collect();
    hand_out(
        shaders,
        #[cfg(windows)]
        None,
        iid,
        out,
    )
}

/// Device method.
pub(crate) extern "system" fn create_compute(
    _: This,
    desc: *const ComputeHead,
    iid: *const Iid,
    out: *mut This,
) -> HRESULT {
    // SAFETY: the title passes a description (null is tolerated).
    let Some(desc) = (unsafe { desc.as_ref() }) else {
        return E_INVALIDARG;
    };
    hand_out(
        copy("compute", &desc.stage).into_iter().collect(),
        #[cfg(windows)]
        None,
        iid,
        out,
    )
}

/// Device method 0x1f0: the plain compute description plus console
/// extensions `(desc, extension count, extensions, iid, out)`.
/// shortcut: the extensions (console shader settings) are ignored, as is
/// the rest of the description.
pub(crate) extern "system" fn create_compute_x(
    this: This,
    desc: *const ComputeHead,
    _extensions: u32,
    _extended: *const u8,
    iid: *const Iid,
    out: *mut This,
) -> HRESULT {
    create_compute(this, desc, iid, out)
}

/// One part of a derived pipeline: which part, then its description, at
/// the size of the largest (the blend state).
pub(crate) const DERIVED_PART: usize = 0x14c;

/// Device method 0x1b8, `CreateDerivedGraphicsPipelineStateX(base, part
/// count, parts, iid, out)`: the base pipeline with some of its state
/// replaced (depth and stencil, rasterizer, blend, sample mask), each part
/// a `u32` kind and the description at +4, [`DERIVED_PART`] bytes apart.
/// The shaders are the base's. Parts only the console's GPU uses are
/// skipped.
pub(crate) extern "system" fn create_derived(
    _: This,
    base: This,
    count: u32,
    parts: *const u8,
    iid: *const Iid,
    out: *mut This,
) -> HRESULT {
    if base.is_null() || (count != 0 && parts.is_null()) {
        return E_INVALIDARG;
    }
    let base = pipeline(base);
    #[cfg(windows)]
    let graphics = base.host.graphics.as_deref().map(|g| {
        let mut g = g.derive();
        for i in 0..count as usize {
            // SAFETY: the title passes `count` parts.
            let (part, data) = unsafe {
                let at = parts.add(i * DERIVED_PART);
                (at.cast::<u32>().read_unaligned(), at.add(4))
            };
            // SAFETY: as above.
            if !unsafe { g.patch(part, data) } {
                static SKIPPED: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                if !SKIPPED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    tracing::info!(part, "derived pipeline part skipped (console only)");
                }
            }
        }
        Box::new(g)
    });
    #[cfg(not(windows))]
    let _ = (count, parts);
    hand_out(
        base.shaders.clone(),
        #[cfg(windows)]
        graphics,
        iid,
        out,
    )
}

/// A pipeline described as a stream: its size and the subobjects, each a
/// type then its data, padded to 8 bytes.
#[repr(C)]
pub(crate) struct StreamDesc {
    size: usize,
    stream: *const u8,
}

/// A subobject type's size (type included), and the stage it is if it is a
/// shader. The console numbers amplification and mesh shaders 23 and 24
/// (PC: 24 and 25); its own types from 25 on are not sized yet.
fn subobject(kind: u32) -> Option<(usize, Option<&'static str>)> {
    let stage = |name| Some((24, Some(name)));
    match kind {
        1 => stage("vertex"),
        2 => stage("pixel"),
        3 => stage("domain"),
        4 => stage("hull"),
        5 => stage("geometry"),
        6 => stage("compute"),
        23 => stage("amplification"),
        24 => stage("mesh"),
        0 => Some((16, None)),
        12 | 19 => Some((24, None)),
        7 | 15 => Some((40, None)),
        8 => Some((336, None)),
        10 => Some((48, None)),
        11 => Some((56, None)),
        17 => Some((16, None)),
        21 => Some((64, None)),
        22 => Some((32, None)),
        9 | 13 | 14 | 16 | 18 | 20 => Some((8, None)),
        _ => None,
    }
}

/// Device method: a pipeline from a stream; the shaders are kept, and the
/// graphics state for the host pipeline.
pub(crate) extern "system" fn create_stream(
    _: This,
    desc: *const StreamDesc,
    iid: *const Iid,
    out: *mut This,
) -> HRESULT {
    // SAFETY: the title passes a description (null is tolerated).
    let Some(desc) = (unsafe { desc.as_ref() }) else {
        return E_INVALIDARG;
    };
    if desc.stream.is_null() {
        return E_INVALIDARG;
    }
    let mut shaders = Vec::new();
    #[cfg(windows)]
    let mut graphics = host::GraphicsState::default();
    let mut at = 0;
    while at + 8 <= desc.size {
        // SAFETY: inside the stream the title passes.
        let kind = unsafe { desc.stream.add(at).cast::<u32>().read_unaligned() };
        let Some((size, stage)) = subobject(kind) else {
            tracing::warn!(
                kind,
                at,
                "pipeline subobject of unknown size; the rest is ignored"
            );
            break;
        };
        if let Some(stage) = stage {
            // SAFETY: a shader subobject has its bytecode at +8.
            let code = unsafe { &*desc.stream.add(at + 8).cast::<Bytecode>() };
            shaders.extend(copy(stage, code));
        }
        #[cfg(windows)]
        // SAFETY: a whole subobject of this kind lies at `at`.
        unsafe {
            graphics.read(kind, desc.stream.add(at))
        };
        at += size;
    }
    // The whole state is read: integer targets do not blend on the host
    // (`GraphicsState::integer_targets_do_not_blend`).
    #[cfg(windows)]
    if graphics.integer_targets_do_not_blend() {
        static SAID: std::sync::Once = std::sync::Once::new();
        SAID.call_once(|| {
            tracing::info!(
                "a pipeline of the title's blends into a render target of integers: blending is off for that target on the host (said once)"
            );
        });
    }
    hand_out(
        shaders,
        #[cfg(windows)]
        Some(Box::new(graphics)),
        iid,
        out,
    )
}

extern "system" fn query(this: This, iid: *const Iid, out: *mut This) -> HRESULT {
    let found = object::is(iid, &PIPELINE_IIDS).then_some(this);
    object::answer(PIPELINE, iid, found, out)
}
extern "system" fn release(this: This) -> u32 {
    object::release_boxed::<Pipeline>(this)
}

/// IUnknown 3, object 4, device child 1, pipeline 3.
static FUNCTIONS: Table<24> = {
    let mut f = object::d3d12_object(object::missing_table::<PIPELINE, 24>());
    f[0] = query as *const ();
    f[2] = release as *const ();
    f[7] = object::parent_device as *const (); // 0x38
    Table(f)
};

#[cfg(windows)]
pub(crate) mod host;
#[cfg(windows)]
mod library;

/// Host pipelines created and loaded from the library since the start
/// (`crate::framelog`).
#[cfg(windows)]
pub(crate) fn frame_log_counts() -> (u64, u64) {
    use std::sync::atomic::Ordering::Relaxed;
    (library::MADE.load(Relaxed), library::LOADED.load(Relaxed))
}
#[cfg(windows)]
mod make;
#[cfg(windows)]
mod recipe;

#[cfg(windows)]
pub use recipe::Prepared;

/// Makes the host pipeline of every recipe the runs kept (`victory
/// prepare`, with the game closed) into the pipeline library, on `threads`
/// workers; `progress` is called every 15 s and at the end (`true`).
#[cfg(windows)]
pub fn prepare_recipes(threads: usize, progress: &(dyn Fn(&Prepared, bool) + Sync)) -> Prepared {
    recipe::prepare(threads, progress)
}

/// Lets the pipeline library's mapped file go from memory when no pipeline
/// has loaded for a while (`library::trim`); called by the host's sweep.
#[cfg(windows)]
pub(crate) fn trim_library() {
    library::trim();
}

/// The pipeline library's part of the report when the host device is
/// removed (`library::removal_note`).
#[cfg(windows)]
pub(crate) fn library_removal_note() -> String {
    library::removal_note()
}

/// Keeps compiled host pipelines, and the list of title pipelines to start
/// early, in `dir` (a folder of the cache) across runs.
#[cfg(windows)]
pub fn use_pipeline_cache(dir: std::path::PathBuf) {
    library::use_pipeline_cache(dir);
}

/// Saves the pipeline cache now if anything was added (at the end of a run).
#[cfg(windows)]
pub fn save_pipeline_cache() {
    library::save(true);
}

#[cfg(test)]
mod tests {
    #[cfg(all(windows, feature = "diagnostics"))]
    #[test]
    fn skip_hashes_accepts_full_hashes_only() {
        assert!(super::parse_skip_hashes(" , ").is_empty());
        assert_eq!(
            super::parse_skip_hashes(" 0123456789ABCDEF, 0123456789abcde, xyz, 0011223344556677,"),
            ["0123456789abcdef", "0011223344556677"]
        );
    }

    use super::*;
    use crate::object::{S_OK, slot};

    fn shaders(p: This) -> Vec<(&'static str, Vec<u8>)> {
        pipeline(p)
            .shaders
            .iter()
            .map(|s| (s.stage, s.container.clone()))
            .collect()
    }

    #[test]
    fn derived_pipelines_keep_the_base_shaders() {
        let vs = [7u8, 8, 9];
        let none = || Bytecode {
            code: std::ptr::null(),
            size: 0,
        };
        let desc = GraphicsHead {
            root: std::ptr::null(),
            stages: [
                Bytecode {
                    code: vs.as_ptr(),
                    size: vs.len(),
                },
                none(),
                none(),
                none(),
                none(),
            ],
        };
        let mut base = std::ptr::null();
        create_graphics(std::ptr::null(), &desc, &PIPELINE_IIDS[0], &mut base);
        // A rasterizer part and one of the console's own.
        let mut parts = vec![0u8; 2 * DERIVED_PART];
        parts[0] = 1;
        parts[DERIVED_PART] = 5;
        let mut derived = std::ptr::null();
        let hr = create_derived(
            std::ptr::null(),
            base,
            2,
            parts.as_ptr(),
            &PIPELINE_IIDS[0],
            &mut derived,
        );
        assert_eq!(hr, S_OK);
        assert!(!derived.is_null() && derived != base);
        assert_eq!(shaders(derived), [("vertex", vs.to_vec())]);
        let hr = create_derived(
            std::ptr::null(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            &PIPELINE_IIDS[0],
            &mut derived,
        );
        assert_eq!(hr, E_INVALIDARG);
    }

    #[test]
    fn pipelines_keep_their_shaders() {
        let (vs, ps) = ([1u8, 2, 3], [4u8; 5]);
        let none = || Bytecode {
            code: std::ptr::null(),
            size: 0,
        };
        let desc = GraphicsHead {
            root: std::ptr::null(),
            stages: [
                Bytecode {
                    code: vs.as_ptr(),
                    size: vs.len(),
                },
                Bytecode {
                    code: ps.as_ptr(),
                    size: ps.len(),
                },
                none(),
                none(),
                none(),
            ],
        };
        let mut p = std::ptr::null();
        assert_eq!(
            create_graphics(std::ptr::null(), &desc, &PIPELINE_IIDS[0], &mut p),
            S_OK
        );
        assert_eq!(
            shaders(p),
            [("vertex", vs.to_vec()), ("pixel", ps.to_vec())]
        );
        // A second pipeline with the same pixel shader shares its container.
        let mut q = std::ptr::null();
        create_graphics(std::ptr::null(), &desc, &PIPELINE_IIDS[0], &mut q);
        assert!(Arc::ptr_eq(
            &pipeline(p).shaders[1],
            &pipeline(q).shaders[1]
        ));
        assert_eq!(slot::<extern "system" fn(This) -> u32>(p, 0x10)(p), 0);
    }

    /// The stream layout the title builds (title+0x86f090): root
    /// signature, five shader slots (the first a mesh shader), blend, sample
    /// mask, rasterizer, depth-stencil 1, sample description, input layout,
    /// topology, render target and depth formats: 0x2b0 bytes.
    #[test]
    fn streams_yield_their_shaders() {
        let (ms, ps) = ([9u8; 7], [4u8; 5]);
        let mut stream = vec![0u8; 0x2b0];
        let mut put = |at: usize, v: u64| stream[at..at + 8].copy_from_slice(&v.to_le_bytes());
        for (at, kind) in [
            (0x10, 24),
            (0x28, 2),
            (0x40, 3),
            (0x58, 4),
            (0x70, 5),
            (0x88, 8),
            (0x1d8, 9),
            (0x1e0, 10),
            (0x210, 21),
            (0x250, 17),
            (0x260, 12),
            (0x278, 14),
            (0x280, 15),
            (0x2a8, 16),
        ] {
            put(at, kind);
        }
        put(0x18, ms.as_ptr() as u64);
        put(0x20, ms.len() as u64);
        put(0x30, ps.as_ptr() as u64);
        put(0x38, ps.len() as u64);
        // Topology type 3 (triangles), one render target of format 28.
        put(0x278, 14 | 3 << 32);
        put(0x280, 15 | 28 << 32);
        put(0x2a0, 1 << 32);
        let desc = StreamDesc {
            size: stream.len(),
            stream: stream.as_ptr(),
        };
        let mut p = std::ptr::null();
        let hr = create_stream(std::ptr::null(), &desc, &PIPELINE_IIDS[0], &mut p);
        assert_eq!(hr, S_OK);
        assert_eq!(shaders(p), [("mesh", ms.to_vec()), ("pixel", ps.to_vec())]);
        #[cfg(windows)]
        {
            let g = pipeline(p).host.graphics.as_deref().unwrap();
            assert_eq!(g.topology.0, 3);
            assert_eq!(
                (g.targets.NumRenderTargets, g.targets.RTFormats[0].0),
                (1, 28)
            );
        }
    }
}

/// Whether a container is a pixel program of the title's decoration bake,
/// by its name inside (`PS_DecorationBlend_*`, `PS_BakeDecorations_*`: 51
/// of them). Try 6.13, ported from edsuuu/VI-BR-PORT 10ac68f9
/// (GPL-3.0-or-later): a list of four hashes (`crate::draw`'s) missed the
/// one the cutscene's faces are baked with (`PS_DecorationBlend_0011`,
/// fcee612943c008e0): its sources were not on the card and the face came
/// out black or flat.
#[cfg(windows)]
fn bakes(bytes: &[u8]) -> bool {
    [&b"PS_DecorationBlend_"[..], b"PS_BakeDecorations_"]
        .iter()
        .any(|name| bytes.windows(name.len()).any(|w| w == *name))
}

#[cfg(all(test, windows))]
#[test]
fn bake_programs_are_known_by_name() {
    assert!(bakes(b"..\0PS_DecorationBlend_0011_ff00f5de_Wrapped\0.."));
    assert!(bakes(b"PS_BakeDecorations_0100_583f45fe_Wrapped"));
    assert!(!bakes(b"PS_PedSkin_0001"));
}
