//! Pipeline recipes: what each title pipeline a draw or dispatch asked for
//! is made from (its shader containers and graphics state), kept in the
//! pipeline cache's folder so that `victory prepare` can make its host
//! pipeline with the game closed, on the cores the game would otherwise
//! fight for, into the same pipeline library a run loads from.
//!
//! In a world run on an RTX 4060 and i5-12400F, 1,675 pipelines were still
//! waiting after 9 minutes (4,600-6,000 draws a frame dropped for them: the
//! see-through world), because the title's own threads left the workers
//! a few pipelines a minute; each launch got further only by what the one
//! before had made. With the recipes, one launch that reaches a place is
//! enough: everything it asked for is made after it closes, and the next
//! launch loads it all from the library at its start.
//!
//! Two files, appended by one background thread so the title's threads
//! never wait on the disk: `recipe-shaders.bin` (every container once, by
//! its BLAKE3) and `recipes.bin` (one record per pipeline key). Each record
//! is a little-endian length and its bytes; a record cut short by a run
//! that ended while writing it is ignored. `VICTORY_RECIPES=0` keeps none.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::Shader;
use super::host::{GraphicsState, HostPipeline};

const SHADERS_FILE: &str = "recipe-shaders.bin";
const RECIPES_FILE: &str = "recipes.bin";
/// Recipes kept at most (the early list keeps 8,192 keys).
const MAX_RECIPES: usize = 16_384;
/// A record past this is not one of ours (a damaged file).
const MAX_RECORD: usize = 64 << 20;

enum Note {
    Shader([u8; 32], Vec<u8>),
    Recipe(String, Vec<u8>),
}

fn on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_RECIPES").is_none_or(|v| v != "0"))
}

/// The writer thread's queue, started on first use; `None` without a cache
/// folder or with recipes off.
fn sender() -> Option<&'static Mutex<Sender<Note>>> {
    static SENDER: OnceLock<Option<Mutex<Sender<Note>>>> = OnceLock::new();
    SENDER
        .get_or_init(|| {
            if !on() {
                return None;
            }
            let dir = super::library::dir()?.to_path_buf();
            let (tx, rx) = channel::<Note>();
            let spawned = std::thread::Builder::new()
                .name("victory-recipes".into())
                .spawn(move || {
                    let mut shaders: HashSet<[u8; 32]> = HashSet::new();
                    for_each_record(&dir.join(SHADERS_FILE), |r| {
                        if let Some(hash) = r.get(..32).and_then(|h| <[u8; 32]>::try_from(h).ok()) {
                            shaders.insert(hash);
                        }
                    });
                    let mut recipes: HashSet<String> = HashSet::new();
                    for_each_record(&dir.join(RECIPES_FILE), |r| {
                        if let Some(recipe) = Recipe::parse(r) {
                            recipes.insert(recipe.key);
                        }
                    });
                    let _ = std::fs::create_dir_all(&dir);
                    let open = |name: &str| {
                        std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(dir.join(name))
                            .ok()
                    };
                    let (mut shader_file, mut recipe_file) = (None, None);
                    for note in rx {
                        let (file, name, bytes) = match note {
                            Note::Shader(hash, container) => {
                                if !shaders.insert(hash) {
                                    continue;
                                }
                                let mut bytes = hash.to_vec();
                                bytes.extend_from_slice(&container);
                                (&mut shader_file, SHADERS_FILE, bytes)
                            }
                            Note::Recipe(key, bytes) => {
                                if recipes.len() >= MAX_RECIPES || !recipes.insert(key) {
                                    continue;
                                }
                                (&mut recipe_file, RECIPES_FILE, bytes)
                            }
                        };
                        if file.is_none() {
                            *file = open(name);
                        }
                        if let Some(f) = file.as_mut() {
                            // One write per record: a run that ends mid-way
                            // leaves at most the last one cut short.
                            let mut record = (bytes.len() as u32).to_le_bytes().to_vec();
                            record.extend_from_slice(&bytes);
                            if let Err(e) = f.write_all(&record) {
                                tracing::warn!(error = %e, file = name, "pipeline recipe not kept");
                                *file = None;
                            }
                        }
                    }
                });
            if let Err(e) = spawned {
                tracing::warn!(error = %e, "no pipeline recipes");
                return None;
            }
            Some(Mutex::new(tx))
        })
        .as_ref()
}

/// A draw or dispatch asked for the title pipeline `key` (made from
/// `shaders` and `graphics`) for the first time in this run: keeps its
/// recipe for `victory prepare`.
pub(crate) fn note(key: &str, shaders: &[Arc<Shader>], graphics: Option<&GraphicsState>) {
    if shaders.is_empty() {
        return;
    }
    let Some(tx) = sender() else { return };
    // Containers already sent in this run (the writer also skips those an
    // earlier run kept).
    static SENT: Mutex<Option<HashSet<String>>> = Mutex::new(None);
    let mut notes = Vec::new();
    let mut record = Vec::new();
    record.push(key.len() as u8);
    record.extend_from_slice(key.as_bytes());
    record.push(shaders.len() as u8);
    {
        let mut sent = SENT.lock().unwrap_or_else(|e| e.into_inner());
        let sent = sent.get_or_insert_with(HashSet::new);
        for s in shaders {
            let full = *blake3::hash(&s.container).as_bytes();
            record.push(s.stage.len() as u8);
            record.extend_from_slice(s.stage.as_bytes());
            record.extend_from_slice(&full);
            if sent.insert(s.hash.clone()) {
                notes.push(Note::Shader(full, s.container.clone()));
            }
        }
    }
    if let Some(g) = graphics {
        record.extend_from_slice(&g.encode());
    }
    notes.push(Note::Recipe(key.to_owned(), record));
    let tx = tx.lock().unwrap_or_else(|e| e.into_inner());
    for n in notes {
        let _ = tx.send(n);
    }
}

/// Calls `f` with each whole record of `path`.
fn for_each_record(path: &Path, mut f: impl FnMut(&[u8])) {
    let Ok(bytes) = std::fs::read(path) else { return };
    let mut at = 0usize;
    while let Some(len) = bytes.get(at..at + 4).map(|l| u32::from_le_bytes([l[0], l[1], l[2], l[3]]) as usize) {
        let Some(record) = bytes.get(at + 4..at + 4 + len).filter(|_| len <= MAX_RECORD) else { break };
        f(record);
        at += 4 + len;
    }
}

/// One recipe as kept.
struct Recipe {
    key: String,
    /// Stage and the container's BLAKE3.
    shaders: Vec<(&'static str, [u8; 32])>,
    /// The graphics state's bytes; empty for compute.
    graphics: Vec<u8>,
}

impl Recipe {
    fn parse(mut r: &[u8]) -> Option<Recipe> {
        fn take<'a>(r: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
            if r.len() < n {
                return None;
            }
            let (head, rest) = r.split_at(n);
            *r = rest;
            Some(head)
        }
        let key_len = take(&mut r, 1)?[0] as usize;
        let key = String::from_utf8(take(&mut r, key_len)?.to_vec()).ok()?;
        let count = take(&mut r, 1)?[0] as usize;
        let mut shaders = Vec::with_capacity(count);
        for _ in 0..count {
            let len = take(&mut r, 1)?[0] as usize;
            let stage = match take(&mut r, len)? {
                b"vertex" => "vertex",
                b"pixel" => "pixel",
                b"mesh" => "mesh",
                b"compute" => "compute",
                b"amplification" => "amplification",
                b"hull" => "hull",
                b"domain" => "domain",
                b"geometry" => "geometry",
                _ => return None,
            };
            let hash = <[u8; 32]>::try_from(take(&mut r, 32)?).ok()?;
            shaders.push((stage, hash));
        }
        let graphics = r.to_vec();
        Some(Recipe { key, shaders, graphics })
    }
}

/// One host pipeline to make.
struct Job {
    shaders: Vec<Arc<Shader>>,
    graphics: Option<GraphicsState>,
    heavy: bool,
}

// SAFETY: as `make::Pending`: D3D12 objects are free-threaded and the
// graphics state (whose input elements point into its own names) is only
// read.
unsafe impl Send for Job {}
// SAFETY: as above.
unsafe impl Sync for Job {}

/// What [`prepare`] did.
#[derive(Clone, Copy, Debug, Default)]
pub struct Prepared {
    /// Recipes kept, and those whose containers were all kept.
    pub recipes: usize,
    pub usable: usize,
    /// Host pipelines that were in the library already, made now, and
    /// that could not be made (a shader that does not translate, a
    /// pipeline the host refused).
    pub loaded: u64,
    pub made: u64,
    pub failed: u64,
}

/// Makes the host pipeline of every recipe kept (`victory prepare`), on
/// `threads` workers, into the pipeline library, which is saved at the
/// end; `progress` is called every 15 s and at the end.
pub fn prepare(threads: usize, progress: &(dyn Fn(&Prepared, bool) + Sync)) -> Prepared {
    let mut out = Prepared::default();
    let Some(dir) = super::library::dir().map(Path::to_path_buf) else {
        return out;
    };
    // The early list is written with the library: read it first, so the
    // save at the end keeps it.
    let _ = super::library::wanted_early("");
    let mut containers: HashMap<[u8; 32], Vec<u8>> = HashMap::new();
    for_each_record(&dir.join(SHADERS_FILE), |r| {
        if let Some(hash) = r.get(..32).and_then(|h| <[u8; 32]>::try_from(h).ok()) {
            containers.entry(hash).or_insert_with(|| r[32..].to_vec());
        }
    });
    let mut seen = HashSet::new();
    let mut recipes = Vec::new();
    for_each_record(&dir.join(RECIPES_FILE), |r| {
        if let Some(recipe) = Recipe::parse(r)
            && seen.insert(recipe.key.clone())
        {
            recipes.push(recipe);
        }
    });
    out.recipes = recipes.len();
    // The programs a run skips (`VICTORY_SKIP_PIPELINES`) are not made.
    let skipped: Vec<String> = std::env::var("VICTORY_SKIP_PIPELINES")
        .unwrap_or_default()
        .split(',')
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| !h.is_empty())
        .collect();
    let jobs: Vec<Job> = recipes
        .iter()
        .filter_map(|r| {
            let shaders: Option<Vec<Arc<Shader>>> = r
                .shaders
                .iter()
                .map(|&(stage, hash)| containers.get(&hash).map(|c| super::shader(stage, c)))
                .collect();
            let shaders = shaders?;
            if shaders.iter().any(|s| skipped.contains(&s.hash)) {
                return None;
            }
            // Try 6.12: nor those a run never builds (`VICTORY_DENY_PIPELINES`,
            // and the passes that only serve rays with every ray a miss).
            if super::make::denied(&shaders).is_some() || super::never_built(&shaders) {
                return None;
            }
            let graphics = if r.graphics.is_empty() {
                None
            } else {
                Some(GraphicsState::decode(&r.graphics)?)
            };
            let heavy = shaders.iter().any(|s| s.size() > super::make::HEAVY_BYTES);
            Some(Job { shaders, graphics, heavy })
        })
        .collect();
    drop(containers);
    out.usable = jobs.len();
    tracing::info!(recipes = out.recipes, usable = out.usable, threads, "preparing host pipelines");
    let (loaded0, made0) = (
        super::library::LOADED.load(Ordering::Relaxed),
        super::library::MADE.load(Ordering::Relaxed),
    );
    let failed = AtomicU64::new(0);
    let next = AtomicUsize::new(0);
    // As the run's workers: at most half of them (1 to 3) on the biggest
    // programs at once, which take the driver the most memory.
    let heavy_limit = (threads / 2).clamp(1, 3);
    let heavy_now = (Mutex::new(0usize), Condvar::new());
    let base = out;
    let snapshot = |failed: u64| Prepared {
        loaded: super::library::LOADED.load(Ordering::Relaxed).saturating_sub(loaded0),
        made: super::library::MADE.load(Ordering::Relaxed).saturating_sub(made0),
        failed,
        ..base
    };
    let last_said = Mutex::new(Instant::now());
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            // The run's workers' stack (NVIDIA's compiler recursed past 2 MB).
            let worker = std::thread::Builder::new().stack_size(64 << 20);
            let _ = worker.spawn_scoped(s, || {
                while let Some(job) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let Job { shaders, graphics, heavy } = job;
                    if *heavy {
                        let (count, freed) = &heavy_now;
                        let mut n = count.lock().unwrap_or_else(|e| e.into_inner());
                        while *n >= heavy_limit {
                            n = freed.wait(n).unwrap_or_else(|e| e.into_inner());
                        }
                        *n += 1;
                    }
                    let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        HostPipeline::new(shaders, graphics.as_ref()).is_some()
                    }))
                    .unwrap_or(false);
                    if *heavy {
                        let (count, freed) = &heavy_now;
                        *count.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
                        freed.notify_all();
                    }
                    if !made {
                        failed.fetch_add(1, Ordering::Relaxed);
                    }
                    // Every 20 s at most, so a prepare cut short keeps most of its work.
                    super::library::save(false);
                    let mut last = last_said.lock().unwrap_or_else(|e| e.into_inner());
                    if last.elapsed() >= Duration::from_secs(15) {
                        *last = Instant::now();
                        progress(&snapshot(failed.load(Ordering::Relaxed)), false);
                    }
                }
            });
        }
    });
    super::library::save(true);
    out = snapshot(failed.load(Ordering::Relaxed));
    progress(&out, true);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipes_read_back_and_a_cut_record_is_ignored() {
        let mut record = vec![4u8];
        record.extend_from_slice(b"abcd");
        record.push(2);
        for (stage, fill) in [("vertex", 1u8), ("pixel", 2u8)] {
            record.push(stage.len() as u8);
            record.extend_from_slice(stage.as_bytes());
            record.extend_from_slice(&[fill; 32]);
        }
        record.extend_from_slice(&[9, 9, 9]);
        let r = Recipe::parse(&record).expect("parses");
        assert_eq!(r.key, "abcd");
        assert_eq!(r.shaders, vec![("vertex", [1u8; 32]), ("pixel", [2u8; 32])]);
        assert_eq!(r.graphics, vec![9, 9, 9]);

        let dir = std::env::temp_dir().join(format!("victory-recipes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(RECIPES_FILE);
        let mut bytes = Vec::new();
        for _ in 0..2 {
            bytes.extend_from_slice(&(record.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&record);
        }
        // A third record cut short, as by a run that ended writing it.
        bytes.extend_from_slice(&(record.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&record[..5]);
        std::fs::write(&file, &bytes).unwrap();
        let mut n = 0;
        for_each_record(&file, |r| {
            assert!(Recipe::parse(r).is_some());
            n += 1;
        });
        assert_eq!(n, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
