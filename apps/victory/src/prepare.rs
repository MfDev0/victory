//! `victory prepare [game files]`: with the game closed, makes the host
//! pipeline of every pipeline earlier runs asked for (the recipes runs keep
//! in the pipeline cache) into the pipeline library, translating their
//! shaders into the shader cache on the way, so the next run finds them all
//! at its start instead of dropping their draws (see-through objects) while
//! its workers fight the title's threads for the cores. Given files or
//! folders, it also translates every shader container found in them.
//!
//! Run it with the same settings as the run (`VICTORY_CACHE_DIR`,
//! `VICTORY_SHADER_LOOP_LIMIT`, the `VICTORY_WAVE_*` switches): they are
//! part of each translation's key. VIctory Try does that after each new
//! build and after each run that asked for new pipelines.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;

use crate::shader::{MAX_FILE, sources};

pub fn run(paths: Vec<PathBuf>, jobs: Option<usize>) -> Result<()> {
    let cfg = victory_core::Config::load()?;
    victory_hle::modules::d3d12x::use_shader_cache(cfg.cache_subdir("shaders")?);
    // All but two logical processors by default (the game is closed; the
    // PC stays usable), at least two.
    let threads = jobs.unwrap_or_else(|| std::thread::available_parallelism().map_or(2, |n| n.get().saturating_sub(2).max(2)));
    let started = std::time::Instant::now();
    let say = |line: String| println!("{line}   ({:.0} s)", started.elapsed().as_secs_f64());
    if !victory_hle::modules::d3d12x::prepare_pipelines(threads, &say) {
        println!("prepare      no pipeline recipes yet: a run keeps them as the game asks for pipelines");
    }
    if !paths.is_empty() {
        translate_files(&paths, threads);
    }
    Ok(())
}

/// Translates every shader container in `paths` into the shader cache.
fn translate_files(paths: &[PathBuf], threads: usize) {
    let seen = Mutex::new(HashSet::new());
    let (done, failed) = (AtomicUsize::new(0), AtomicUsize::new(0));
    for root in paths {
        for entry in walkdir::WalkDir::new(root).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() || entry.metadata().map_or(0, |m| m.len()) > MAX_FILE {
                continue;
            }
            let Ok(data) = std::fs::read(entry.path()) else {
                continue;
            };
            let jobs = sources(&data);
            let next = AtomicUsize::new(0);
            std::thread::scope(|s| {
                for _ in 0..threads.min(jobs.len()) {
                    // The run's pipeline workers' stack: deep programs overflowed 2 MB.
                    let worker = std::thread::Builder::new().stack_size(64 << 20);
                    let _ = worker.spawn_scoped(s, || {
                        while let Some(job) = jobs.get(next.fetch_add(1, Ordering::Relaxed)) {
                            let Some(bytes) = job.bytes() else { continue };
                            if !seen.lock().unwrap_or_else(|e| e.into_inner()).insert(blake3::hash(&bytes)) {
                                continue;
                            }
                            // One shader that panics the translator is left to the run.
                            let ok = std::panic::catch_unwind(|| victory_hle::modules::d3d12x::prepare_shader(&bytes))
                                .unwrap_or(false);
                            let counter = if ok { &done } else { &failed };
                            let n = counter.fetch_add(1, Ordering::Relaxed) + 1;
                            // Try 6.11: a line every 2,000 shaders, so the
                            // first pass over the game's files (minutes)
                            // shows that it goes on.
                            if ok && n.is_multiple_of(2000) {
                                println!("translating  {n} shaders of the game's files so far");
                            }
                        }
                    });
                }
            });
        }
    }
    let (hits, made) = victory_hle::modules::d3d12x::prepared_shader_counts();
    println!(
        "translated   {} shaders from the files ({hits} already cached, {made} translated now), {} do not translate",
        done.load(Ordering::Relaxed),
        failed.load(Ordering::Relaxed)
    );
}
