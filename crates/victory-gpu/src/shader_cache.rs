//! Translated shaders kept on disk (decision 0003): under the configured
//! cache folder, keyed by the container's content and by the translator
//! that translated it (its source fingerprint and settings, see `build`), so
//! a changed translator never serves an old translation. Entries go through
//! `victory-pipeline`'s content-addressed cache, which writes them
//! atomically; an entry that does not read back is translated again.
//!
//! The entry is our own encoding of [`HostShader`]: a field added to it
//! must be added to `encode` and `decode` too (entries of older builds
//! are ignored anyway).
//!
//! Try 6.16, the disk: each build of the translator keys its translations
//! afresh (`build`), so every new version translated the title's 25,746
//! shaders again (the 6.15 prepare log: "2879 already cached, 25749
//! translated now") and filed them beside every older version's, which
//! nothing read again and nothing removed: about a gigabyte more of the
//! cache folder with each update. Now each build key has a folder of its
//! own (`b-` and 16 hex digits) with a stamp of when a run last used it,
//! and a while after the start the folders of all but the
//! `VICTORY_SHADER_CACHE_KEEP` (2 unless set; 0: none removed) most
//! recently used builds go, as do the entries of the flat layout before
//! 6.16 (folders of two hex digits). A folder used in the last hour stays
//! whatever its rank.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use victory_pipeline::{Cache, Converter};
use victory_shader::emit::{HostShader, StaticSampler, UserData};
use victory_shader::resource::RootIndexedView;
use victory_shader::{HwStage, RootType};

/// The cache folder ([`use_shader_cache`]), absolute.
static ROOT: OnceLock<PathBuf> = OnceLock::new();
/// The caches of the build keys met so far, each in its folder under
/// [`ROOT`] ([`cache_for`]): one or two a run.
static CACHES: Mutex<Vec<([u8; 32], Cache)>> = Mutex::new(Vec::new());
static HITS: AtomicU64 = AtomicU64::new(0);
static MISSES: AtomicU64 = AtomicU64::new(0);

/// The file in a build's folder that says when a run last used it
/// (seconds since 1970, as text).
const USED: &str = "used";

/// A folder used within this many seconds is never removed.
const RECENT_S: u64 = 3600;

/// Keeps translated shaders in `dir` from now on, each build's in a folder
/// of its own; the folders of older builds are removed a while after the
/// start ([`prune`]).
pub fn use_shader_cache(dir: PathBuf) {
    match Cache::open(&dir) {
        Ok(_) => {
            let root = std::path::absolute(&dir).unwrap_or(dir);
            if ROOT.set(root.clone()).is_ok() {
                prune_later(root);
            }
        }
        Err(e) => tracing::warn!(error = %e, "no shader cache"),
    }
}

/// How many of the most recently used builds' folders are kept
/// (`VICTORY_SHADER_CACHE_KEEP`; 2 unless set; 0: nothing is removed).
fn keep() -> usize {
    std::env::var("VICTORY_SHADER_CACHE_KEEP").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(2)
}

/// The folder of a build key's translations.
fn build_folder(build: &[u8; 32]) -> String {
    let mut name = String::from("b-");
    for b in &build[..8] {
        name.push_str(&format!("{b:02x}"));
    }
    name
}

fn now_s() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The cache of `build`'s translations, its folder made and stamped as
/// used at the first ask of this run; none without a cache folder.
fn cache_for(build: &[u8; 32]) -> Option<Cache> {
    let root = ROOT.get()?;
    let mut caches = CACHES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, cache)) = caches.iter().find(|(b, _)| b == build) {
        return Some(cache.clone());
    }
    let dir = root.join(build_folder(build));
    match Cache::open(&dir) {
        Ok(cache) => {
            let _ = std::fs::write(dir.join(USED), now_s().to_string());
            caches.push((*build, cache.clone()));
            Some(cache)
        }
        Err(e) => {
            static WARNED: std::sync::Once = std::sync::Once::new();
            WARNED.call_once(|| tracing::warn!(error = %e, dir = %dir.display(), "no shader cache folder for this build"));
            None
        }
    }
}

/// [`prune`] on a thread of its own, 90 seconds after the start: by then
/// this run has stamped the folders it uses, and removing a gigabyte of
/// small files takes nothing from the loading.
fn prune_later(root: PathBuf) {
    let keep = keep();
    if keep == 0 {
        return;
    }
    let spawned = std::thread::Builder::new().name("victory-shader-cache-tidy".into()).spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(90));
        let (builds, flat) = prune(&root, keep, now_s());
        if builds + flat > 0 {
            tracing::info!(
                builds,
                flat,
                keep,
                "shader cache: translations of older builds removed (VICTORY_SHADER_CACHE_KEEP; each version translates the game's shaders again)"
            );
        }
    });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "no thread to remove older builds' translations");
    }
}

/// Removes from the cache folder `root` the folders of builds other than
/// the `keep` most recently used and those used in the last hour (`now`
/// in seconds since 1970), and the folders of the flat layout before Try
/// 6.16 (two hex digits). Other files and folders stay. Returns how many
/// build folders and flat folders went.
fn prune(root: &Path, keep: usize, now: u64) -> (usize, usize) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return (0, 0);
    };
    let mut builds = Vec::new();
    let mut flat = 0;
    for e in entries.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if name.len() == 2 && name.bytes().all(|b| b.is_ascii_hexdigit()) {
            if std::fs::remove_dir_all(&path).is_ok() {
                flat += 1;
            }
        } else if name.len() == 18 && name.starts_with("b-") && name[2..].bytes().all(|b| b.is_ascii_hexdigit()) {
            let used = std::fs::read_to_string(path.join(USED)).ok().and_then(|t| t.trim().parse::<u64>().ok()).unwrap_or(0);
            builds.push((used, path));
        }
    }
    builds.sort_by_key(|b| std::cmp::Reverse(b.0));
    let mut gone = 0;
    for (used, path) in builds.into_iter().skip(keep) {
        if now.saturating_sub(used) > RECENT_S && std::fs::remove_dir_all(&path).is_ok() {
            gone += 1;
        }
    }
    (gone, flat)
}

/// Shaders found in the cache and translated since the start.
pub(crate) fn counts() -> (u64, u64) {
    (HITS.load(Ordering::Relaxed), MISSES.load(Ordering::Relaxed))
}

/// What identifies the translation, besides the container: the shader
/// crate's fingerprint (its source, every switch it reads, the host facts the
/// device gave it, the loop limit, barrier waits and clean-up passes) and
/// this file's encoding of the result. Not the whole executable any more: a
/// rebuild that leaves the translator alone keeps its translations (each new
/// build used to translate the title's thousands of shaders again, which kept
/// the first minutes of every test run short of pipelines).
/// `VICTORY_SHADER_CACHE_KEY=exe` restores the old key (the executable's hash).
/// Worked out per lookup, so a translation made before the device said its
/// wave size is never filed under the later key.
fn build() -> [u8; 32] {
    static WARNED: std::sync::Once = std::sync::Once::new();
    let limit = victory_shader::emit::loop_limit();
    if limit != victory_shader::emit::DEFAULT_LOOP_LIMIT {
        WARNED.call_once(|| tracing::warn!(limit, "VICTORY_SHADER_LOOP_LIMIT: translated loops stop sooner"));
    }
    static EXE: OnceLock<Option<[u8; 32]>> = OnceLock::new();
    let exe = EXE.get_or_init(|| {
        if std::env::var_os("VICTORY_SHADER_CACHE_KEY").is_some_and(|v| v == "exe") {
            match std::env::current_exe().and_then(std::fs::read) {
                Ok(bytes) => Some(*blake3::hash(&bytes).as_bytes()),
                Err(e) => {
                    tracing::warn!(error = %e, "cannot read our executable; keying on the translator");
                    None
                }
            }
        } else {
            None
        }
    });
    let mut h = blake3::Hasher::new();
    h.update(b"victory shader cache key 2\0");
    if let Some(exe) = exe {
        h.update(exe);
    }
    h.update(&victory_shader::emit::translation_fingerprint());
    // The list this file reads for the translator (`translate_listed`): the
    // shader crate's fingerprint does not see it.
    h.update(b"VICTORY_WAVE_WIDE_ONLY=");
    if let Some(value) = std::env::var_os("VICTORY_WAVE_WIDE_ONLY") {
        h.update(value.as_encoded_bytes());
    }
    h.update(include_bytes!("shader_cache.rs"));
    *h.finalize().as_bytes()
}

/// A container translated with the wave-wide form allowed when its hash is
/// on `VICTORY_WAVE_WIDE_ONLY` (16-hex-digit hashes as the log names
/// shaders, comma separated; unset or empty: none). Off by default: two
/// programs that moved to that form were in a submission that lost the card
/// (runs 121-124, not proven the cause). After commit a416e392 of pull
/// request #823 (Felix2180), whose list is the four refused programs
/// 5dc3b25986622949 and 6f225b641c5b0708 (the water's transmission
/// volumetrics), 369cbe906ea72764 (particle visibility) and
/// 2cc4c197ee29e291 (the reflection pass's tiling). For runs and the offline
/// warm-up alike, which share the cache.
fn translate_listed(c: &[u8]) -> Result<HostShader, String> {
    static LIST: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let list = LIST.get_or_init(|| {
        std::env::var("VICTORY_WAVE_WIDE_ONLY")
            .unwrap_or_default()
            .split(',')
            .map(|h| h.trim().to_lowercase())
            .filter(|h| !h.is_empty() && h != "none")
            .collect()
    });
    let hash = blake3::hash(c).to_hex();
    let wide = list.iter().any(|h| hash.starts_with(h.as_str()));
    let program = program_hash(c);
    let done = victory_shader::emit::for_program(&program, || victory_shader::emit::with_wave_wide(wide, || victory_shader::emit::translate(c)))
        .map_err(|e| e.to_string());
    // Try 6.19: the retry form a console wave64 program was made in, said
    // where the translation is made (a run, `victory prepare` or the
    // warm-up; a translation from the cache says nothing). The approximate
    // forms give each half of a console wave its own cross-lane results: a
    // tiled program's light list or depth range per half-tile, light in
    // blocks on the screen.
    let form = victory_shader::emit::take_form();
    if done.is_ok()
        && let Some(form) = form
    {
        if form.contains("APPROXIMATE") {
            let n = APPROXIMATE.fetch_add(1, Ordering::Relaxed) + 1;
            tracing::warn!(
                hash = %program,
                form = %form,
                so_far = n,
                "shader translated APPROXIMATELY: each half of a console wave has its own cross-lane results (a tiled program's \
                 light list or depth range per half-tile: light in blocks); VICTORY_WAVE_WIDE_FIRST=1 tries the wave-wide form \
                 first for groups of one console wave"
            );
        } else {
            tracing::info!(hash = %program, form = %form, "shader translated in a retry form for 32-lane waves");
        }
    }
    done
}

/// Try 6.19: translations made in an approximate form ([`translate_listed`]),
/// since the start.
pub(crate) static APPROXIMATE: AtomicU64 = AtomicU64::new(0);

/// The offline warm-up (`victory translate --warm`): `container` is
/// translated into the cache unless it is there already. `Ok(true)`: it
/// was translated now. Needs [`use_shader_cache`] and the same host facts
/// the run will have (`victory_shader::emit::translation_fingerprint`).
pub fn warm(container: &[u8]) -> Result<bool, String> {
    let build = build();
    let cache = cache_for(&build).ok_or("no shader cache")?;
    let ran = AtomicBool::new(false);
    let translate = |c: &[u8]| {
        ran.store(true, Ordering::Relaxed);
        translate_listed(c)
    };
    cached(&cache, &build, victory_shader::emit::host_uses_wave32(), container, translate).map(|_| ran.load(Ordering::Relaxed))
}

/// Fills the cache ahead of a run (`victory prepare`): the container's
/// translation as this executable makes it in a run on this PC (the host's
/// wave width and view heap limit, found as a run finds them), taken from
/// the cache or made and stored now. Whether it translates.
///
/// A rebuilt VIctory keys its translations afresh, so the first world
/// visit after every update translated each shader again while the title
/// waited for it (draws dropped, objects see-through); prepared, a run
/// finds them all.
pub fn prepare(container: &[u8]) -> bool {
    crate::descriptor::host::probe_view_limit();
    host_shader(container).is_ok()
}

/// Translations found in the cache and made, since the start.
pub fn prepared_counts() -> (u64, u64) {
    counts()
}

/// The translation of a console shader container, from the cache when
/// there is one.
pub(crate) fn host_shader(container: &[u8]) -> Result<HostShader, String> {
    let translate = translate_listed;
    let build = build();
    match cache_for(&build) {
        Some(cache) => cached(&cache, &build, victory_shader::emit::host_uses_wave32(), container, translate),
        None => translate(container),
    }
}

/// The hash the log names a shader by (`pipeline.rs`: the first 16 hex
/// digits of the container's BLAKE3), which the translator's per-program
/// switches take (`victory_shader::emit::for_program`).
fn program_hash(container: &[u8]) -> String {
    blake3::hash(container).to_hex()[..16].to_owned()
}

/// The cache entry's converter: the build, host wave mode and container in, the
/// encoded translation out. `ran` tells a miss from a hit.
struct Translate<F> {
    translate: F,
    ran: AtomicBool,
}

impl<F: Fn(&[u8]) -> Result<HostShader, String> + Send + Sync> Converter for Translate<F> {
    fn id(&self) -> &'static str {
        "shader.dxil"
    }
    fn version(&self) -> u32 {
        // Version 1 did not distinguish translations made for 32-lane hosts.
        2
    }
    fn convert(&self, input: &[u8]) -> anyhow::Result<Vec<u8>> {
        self.ran.store(true, Ordering::Relaxed);
        let shader = (self.translate)(&input[33..]).map_err(anyhow::Error::msg)?;
        Ok(encode(&shader))
    }
}

fn cached(
    cache: &Cache,
    build: &[u8; 32],
    wave32: bool,
    container: &[u8],
    translate: impl Fn(&[u8]) -> Result<HostShader, String> + Send + Sync,
) -> Result<HostShader, String> {
    let input = [&build[..], &[u8::from(wave32)], container].concat();
    let converter = Translate {
        translate,
        ran: AtomicBool::new(false),
    };
    for _ in 0..2 {
        let path = cache
            .get_or_convert(&converter, &input)
            .map_err(|e| format!("{e:#}"))?;
        if let Some(shader) = std::fs::read(&path).ok().and_then(|b| decode(&b)) {
            let (counter, what) = if converter.ran.load(Ordering::Relaxed) {
                (&MISSES, "translated and cached")
            } else {
                (&HITS, "from the cache")
            };
            counter.fetch_add(1, Ordering::Relaxed);
            tracing::debug!(path = %path.display(), "shader {what}");
            return Ok(shader);
        }
        tracing::debug!(path = %path.display(), "unreadable shader cache entry; translating again");
        let _ = std::fs::remove_file(&path);
    }
    MISSES.fetch_add(1, Ordering::Relaxed);
    (converter.translate)(container)
}

const MAGIC: &[u8; 4] = b"VSH1";
const STAGES: [HwStage; 8] = [
    HwStage::Ls,
    HwStage::Hs,
    HwStage::Es,
    HwStage::Gs,
    HwStage::Vs,
    HwStage::Ps,
    HwStage::Ms,
    HwStage::Cs,
];

fn root_code(r: RootType) -> u32 {
    match r {
        RootType::Constants => 0,
        RootType::Descriptor => 1,
        RootType::Table => 2,
        RootType::ImmediateSampler => 3,
        RootType::ResourceHeap => 4,
        RootType::SamplerHeap => 5,
        RootType::Other(x) => 6 + x,
    }
}

fn root_type(code: u32) -> RootType {
    match code {
        0 => RootType::Constants,
        1 => RootType::Descriptor,
        2 => RootType::Table,
        3 => RootType::ImmediateSampler,
        4 => RootType::ResourceHeap,
        5 => RootType::SamplerHeap,
        x => RootType::Other(x - 6),
    }
}

/// Little-endian words and length-prefixed lists.
fn encode(s: &HostShader) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    let word = |out: &mut Vec<u8>, v: u32| out.extend(v.to_le_bytes());
    let stage = STAGES.iter().position(|&x| x == s.stage).unwrap_or(0) as u32;
    word(&mut out, stage);
    word(&mut out, s.dxil.len() as u32);
    out.extend(&s.dxil);
    word(&mut out, s.user_data.len() as u32);
    for u in &s.user_data {
        for v in [u.index, u32::from(u.slot), root_code(u.root), u.dword] {
            word(&mut out, v);
        }
    }
    word(&mut out, s.samplers.len() as u32);
    for sampler in &s.samplers {
        for &v in sampler {
            word(&mut out, v);
        }
    }
    word(&mut out, s.static_samplers.len() as u32);
    for t in &s.static_samplers {
        let [u, v, w] = t.address;
        let words = [
            t.filter,
            u,
            v,
            w,
            t.mip_lod_bias.to_bits(),
            t.max_anisotropy,
            t.comparison,
            t.border,
            t.min_lod.to_bits(),
            t.max_lod.to_bits(),
        ];
        for v in words {
            word(&mut out, v);
        }
    }
    for slots in [&s.root_cbvs, &s.root_srvs, &s.root_uavs] {
        word(&mut out, slots.len() as u32);
        out.extend(slots);
    }
    word(&mut out, s.root_indexed_views.len() as u32);
    for view in &s.root_indexed_views {
        for value in [
            u32::from(view.table_slot),
            u32::from(view.constant_slot),
            view.constant_dword,
            view.byte_offset,
            u32::from(view.shift),
        ] {
            word(&mut out, value);
        }
    }
    word(&mut out, s.half_views.len() as u32);
    for &(slot, dword) in &s.half_views {
        word(&mut out, u32::from(slot));
        word(&mut out, dword);
    }
    out
}

fn decode(bytes: &[u8]) -> Option<HostShader> {
    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> Option<&'a [u8]> {
            let (head, rest) = self.0.split_at_checked(n)?;
            self.0 = rest;
            Some(head)
        }
        fn word(&mut self) -> Option<u32> {
            Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
        }
        fn list(&mut self) -> Option<Vec<u8>> {
            let n = self.word()? as usize;
            Some(self.take(n)?.to_vec())
        }
    }
    let mut r = Reader(bytes);
    if r.take(4)? != MAGIC {
        return None;
    }
    let stage = *STAGES.get(r.word()? as usize)?;
    let dxil = r.list()?;
    let n = r.word()? as usize;
    let mut user_data = Vec::with_capacity(n.min(256));
    for _ in 0..n {
        user_data.push(UserData {
            index: r.word()?,
            slot: u8::try_from(r.word()?).ok()?,
            root: root_type(r.word()?),
            dword: r.word()?,
        });
    }
    let n = r.word()? as usize;
    let mut samplers = Vec::with_capacity(n.min(256));
    for _ in 0..n {
        samplers.push([r.word()?, r.word()?, r.word()?, r.word()?]);
    }
    let n = r.word()? as usize;
    let mut static_samplers = Vec::with_capacity(n.min(256));
    for _ in 0..n {
        static_samplers.push(StaticSampler {
            filter: r.word()?,
            address: [r.word()?, r.word()?, r.word()?],
            mip_lod_bias: f32::from_bits(r.word()?),
            max_anisotropy: r.word()?,
            comparison: r.word()?,
            border: r.word()?,
            min_lod: f32::from_bits(r.word()?),
            max_lod: f32::from_bits(r.word()?),
        });
    }
    let root_cbvs = r.list()?;
    let root_srvs = r.list()?;
    let root_uavs = r.list()?;
    let n = r.word()? as usize;
    let mut root_indexed_views = Vec::with_capacity(n.min(256));
    for _ in 0..n {
        root_indexed_views.push(RootIndexedView {
            table_slot: u8::try_from(r.word()?).ok()?,
            constant_slot: u8::try_from(r.word()?).ok()?,
            constant_dword: r.word()?,
            byte_offset: r.word()?,
            shift: u8::try_from(r.word()?).ok().filter(|&s| s < 32)?,
        });
    }
    let n = r.word()? as usize;
    let mut half_views = Vec::with_capacity(n.min(256));
    for _ in 0..n {
        half_views.push((u8::try_from(r.word()?).ok()?, r.word()?));
    }
    r.0.is_empty().then_some(HostShader {
        stage,
        dxil,
        user_data,
        samplers,
        static_samplers,
        root_cbvs,
        root_srvs,
        root_uavs,
        root_indexed_views,
        half_views,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shader() -> HostShader {
        HostShader {
            stage: HwStage::Ps,
            dxil: b"DXBC and the rest".to_vec(),
            user_data: vec![UserData {
                index: 8,
                slot: 1,
                root: RootType::Table,
                dword: 0,
            }],
            samplers: vec![[0x400, 0xc0_0000, 0x18f0_0000, 0]],
            static_samplers: vec![StaticSampler::from_words([
                0x400,
                0xc0_0000,
                0x18f0_0000,
                0,
            ])],
            root_cbvs: vec![2],
            root_srvs: vec![0, 3],
            root_uavs: vec![1],
            root_indexed_views: vec![RootIndexedView {
                table_slot: 1,
                constant_slot: 2,
                constant_dword: 3,
                byte_offset: 32,
                shift: 5,
            }],
            half_views: vec![(0, 8), (4, 0)],
        }
    }

    #[test]
    fn translations_come_back_from_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::open(dir.path()).unwrap();
        let build = [7u8; 32];
        let container = b"a console container".as_slice();
        let runs = AtomicU64::new(0);
        let translate = |_: &[u8]| {
            runs.fetch_add(1, Ordering::Relaxed);
            Ok(shader())
        };
        assert_eq!(cached(&cache, &build, false, container, translate), Ok(shader()));
        assert_eq!(cached(&cache, &build, false, container, translate), Ok(shader()));
        assert_eq!(runs.load(Ordering::Relaxed), 1, "the second came from disk");

        // Another build translates again.
        assert_eq!(cached(&cache, &[8; 32], false, container, translate), Ok(shader()));
        assert_eq!(runs.load(Ordering::Relaxed), 2);

        // A damaged entry is a miss, translated and stored again.
        for e in walk(dir.path()) {
            std::fs::write(e, b"VSH1 broken").unwrap();
        }
        assert_eq!(cached(&cache, &build, false, container, translate), Ok(shader()));
        assert_eq!(runs.load(Ordering::Relaxed), 3);
        assert_eq!(cached(&cache, &build, false, container, translate), Ok(shader()));
        assert_eq!(runs.load(Ordering::Relaxed), 3);

        // Failures are not stored.
        let fail = |_: &[u8]| Err::<HostShader, _>("unsupported".to_owned());
        assert!(cached(&cache, &build, false, b"other", fail).is_err());
    }

    #[test]
    fn reopening_the_cache_preserves_each_host_wave_translation() {
        let dir = tempfile::tempdir().unwrap();
        let input = b"synthetic wave reduction";
        let runs = AtomicU64::new(0);
        for wave32 in [false, true, false, true] {
            let cache = Cache::open(dir.path()).unwrap();
            let mut expected = shader();
            expected.stage = HwStage::Cs;
            expected.dxil.push(if wave32 { 32 } else { 64 });
            let result = cached(&cache, &[7; 32], wave32, input, |container| {
                assert_eq!(container, input);
                runs.fetch_add(1, Ordering::Relaxed);
                Ok(expected.clone())
            });
            assert_eq!(result, Ok(expected));
        }
        assert_eq!(runs.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn older_builds_folders_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let now = 1_000_000;
        let make = |name: &str, used: Option<u64>| {
            let d = root.join(name);
            std::fs::create_dir_all(d.join("ab")).unwrap();
            std::fs::write(d.join("ab").join("cdef"), b"VSH1").unwrap();
            if let Some(used) = used {
                std::fs::write(d.join(USED), used.to_string()).unwrap();
            }
        };
        make("b-0000000000000001", Some(now));
        make("b-0000000000000002", Some(now - 10_000));
        make("b-0000000000000003", Some(now - 20_000));
        make("b-0000000000000004", Some(now - 100));
        make("b-0000000000000005", None);
        // The flat layout before 6.16, and things that are not the cache's.
        std::fs::create_dir_all(root.join("7f")).unwrap();
        std::fs::write(root.join("7f").join("0123"), b"VSH1").unwrap();
        std::fs::create_dir_all(root.join("pipelines")).unwrap();
        std::fs::write(root.join("notes.txt"), b"x").unwrap();

        assert_eq!(prune(root, 1, now), (3, 1));
        let left: std::collections::BTreeSet<String> =
            std::fs::read_dir(root).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        // The newest, and the one used within the hour, stay.
        let want: std::collections::BTreeSet<String> =
            ["b-0000000000000001", "b-0000000000000004", "pipelines", "notes.txt"].iter().map(|s| s.to_string()).collect();
        assert_eq!(left, want);
        // Nothing more to do.
        assert_eq!(prune(root, 1, now), (0, 0));
    }

    #[test]
    fn a_build_folder_is_named_by_its_key() {
        let mut key = [0u8; 32];
        key[0] = 0xab;
        key[7] = 0x01;
        key[8] = 0xff;
        assert_eq!(build_folder(&key), "b-ab00000000000001");
    }

    fn walk(dir: &std::path::Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            if e.path().is_dir() {
                files.extend(walk(&e.path()));
            } else {
                files.push(e.path());
            }
        }
        files
    }

    #[test]
    fn entries_round_trip_and_reject_damage() {
        let bytes = encode(&shader());
        assert_eq!(decode(&bytes), Some(shader()));
        assert_eq!(decode(&bytes[..bytes.len() - 1]), None);
        assert_eq!(decode(&[bytes.as_slice(), &[0]].concat()), None);
        assert_eq!(decode(b"VSH0"), None);
    }
}
