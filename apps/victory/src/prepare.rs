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

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;

use crate::shader::sources;

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
    let done = AtomicUsize::new(0);
    // Reason -> containers stopped by it, and the first one's hash.
    let failed = Mutex::new(BTreeMap::<String, (usize, String)>::new());
    let not_programs = Mutex::new(BTreeMap::<String, (usize, String)>::new());
    let dump = std::env::var_os("VICTORY_DUMP_FAILED").map(PathBuf::from);
    let dumped = AtomicUsize::new(0);
    for path in crate::shader::shader_files(paths, false) {
        {
            let Some(data) = crate::shader::read_shader_carrier(&path) else {
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
                            let hash = blake3::hash(&bytes);
                            if !seen.lock().unwrap_or_else(|e| e.into_inner()).insert(hash) {
                                continue;
                            }
                            // One shader that panics the translator is left to the run.
                            let outcome = std::panic::catch_unwind(|| victory_hle::modules::d3d12x::prepare_shader(&bytes))
                                .unwrap_or_else(|p| Err(format!("the translator panicked: {}", panic_text(&*p))));
                            match outcome {
                                Ok(()) => {
                                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                                    // Try 6.11: a line every 2,000 shaders, so the
                                    // first pass over the game's files (minutes)
                                    // shows that it goes on.
                                    if n.is_multiple_of(2000) {
                                        println!("translating  {n} shaders of the game's files so far");
                                    }
                                }
                                Err(why) => {
                                    let hash16 = hash.to_hex()[..16].to_owned();
                                    let reason = reason_of(&why);
                                    // Try 6.23: containers a run never makes a
                                    // pipeline of (no console program in them,
                                    // or only stages the title's pipelines do
                                    // not use) are counted apart: they were
                                    // most of "do not translate".
                                    let not_program = is_not_program(&reason);
                                    let into = if not_program { &not_programs } else { &failed };
                                    into.lock()
                                        .unwrap_or_else(|e| e.into_inner())
                                        .entry(reason)
                                        .or_insert_with(|| (0, hash16.clone()))
                                        .0 += 1;
                                    // The real failures go where a run puts the
                                    // shaders it cannot translate (Try's
                                    // failed-shaders folder, sent with a report),
                                    // at most 400 of them.
                                    if !not_program
                                        && let Some(dir) = &dump
                                        && dumped.fetch_add(1, Ordering::Relaxed) < 400
                                    {
                                        let file = dir.join(format!("prepare-{hash16}.dxbc"));
                                        let _ = std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&file, &bytes));
                                    }
                                }
                            }
                        }
                    });
                }
            });
        }
    }
    let (hits, made) = victory_hle::modules::d3d12x::prepared_shader_counts();
    let failed = failed.into_inner().unwrap_or_else(|e| e.into_inner());
    let not_programs = not_programs.into_inner().unwrap_or_else(|e| e.into_inner());
    let count = |m: &BTreeMap<String, (usize, String)>| m.values().map(|v| v.0).sum::<usize>();
    println!(
        "translated   {} shaders from the files ({hits} already cached, {made} translated now), {} do not translate, {} are no program a run makes",
        done.load(Ordering::Relaxed),
        count(&failed),
        count(&not_programs)
    );
    // Each reason once, most first (the lines start with "translat" so
    // VIctory Try shows them).
    for (title, map) in [("translate failed", &failed), ("translate skipped", &not_programs)] {
        let mut list: Vec<_> = map.iter().collect();
        list.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
        for (reason, (n, example)) in list.iter().take(25) {
            println!("{title} {n:>5}  {reason}  (e.g. {example})");
        }
        if list.len() > 25 {
            println!("{title} ...    and {} more reasons (all in prepare.log)", list.len() - 25);
            for (reason, (n, example)) in list.iter().skip(25) {
                eprintln!("  {n:>5}  {reason}  (e.g. {example})");
            }
        }
    }
}

/// A failure's reason without what is particular to one shader (the
/// cache's converter prefix, code offsets), so equal reasons count together.
fn reason_of(why: &str) -> String {
    let mut s = why;
    if let Some(i) = s.find("converter shader.dxil v")
        && let Some(j) = s[i..].find(": ")
    {
        s = &s[i + j + 2..];
    }
    if let Some(rest) = s.strip_prefix("GFX10 code at ")
        && let Some(j) = rest.find(": ")
    {
        s = &rest[j + 2..];
    }
    // Remaining hexadecimal numbers (addresses, offsets) as `0x_`.
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '0' && chars.peek() == Some(&'x') {
            chars.next();
            out.push_str("0x_");
            while chars.peek().is_some_and(char::is_ascii_hexdigit) {
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Reasons that say the container holds no program a run makes a pipeline
/// of: not a failure of the translator.
fn is_not_program(reason: &str) -> bool {
    reason.contains("no DXIL part")
        || reason.contains("not a DXBC container")
        || reason.contains("a container without a vertex, pixel, compute or mesh program")
}

fn panic_text(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "?".into())
}

#[cfg(test)]
mod tests {
    use super::{is_not_program, reason_of};

    #[test]
    fn reasons_count_together() {
        assert_eq!(
            reason_of("converter shader.dxil v2: GFX10 code at 0x1c4: a descriptor read only as data is not supported"),
            "a descriptor read only as data is not supported"
        );
        assert_eq!(reason_of("GFX10 code at 0x8: a loop at 0x20 is not supported"), "a loop at 0x_ is not supported");
        assert!(is_not_program(&reason_of("converter shader.dxil v2: malformed shader: no DXIL part")));
        assert!(!is_not_program("a descriptor read only as data is not supported"));
    }
}
