//! Switches that pick a translation per program, by the hash a run's log
//! names the shader by: the first 16 hex digits of the BLAKE3 of the
//! container as the title passes it (`hash=` in "shader translated").
//!
//! The one so far is for the wave64 form "without skips, scalars followed"
//! ([`crate::ir::without_skips`] with `deep`), the exact form tried for a
//! console wave64 compute program on a host of 32-lane waves before the
//! approximate per-half-wave ones. On an RTX 4080 it lost walls, roofs and
//! floors (about 5,000 draws a frame against 6,900 with it off), and the 14
//! cascade-shadow tiled renderers (`CS_ShadowRenderTiled*`) were the ones
//! wrong in it: with only them kept out, the prison cell had its ceiling,
//! lights, walls' colours and the uniform's print back; keeping all 22
//! programs that take the form out left the room much darker
//! (Xeltax/VIcky 05ee4e9 and VICKY.md, fxceless/victory-fork#62).
//! - `VICTORY_WAVE_DEEP_SKIPS=0`: off for every program;
//! - `VICTORY_WAVE_DEEP_SKIPS_NOT=<hash>,...`: off for the listed programs
//!   ([`SHADOW_PROGRAMS`] when unset; empty: on for every program);
//! - `VICTORY_WAVE_DEEP_SKIPS_ONLY=<hash>,...`: off for every program not
//!   listed.
//!
//! The switches are part of the translator's fingerprint (they are
//! `VICTORY_*` switches of this crate), so changing one translates again.

use std::cell::RefCell;
use std::sync::OnceLock;

thread_local! {
    // The hash of the container being translated on this thread
    // ([`for_program`]).
    static PROGRAM: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// The 14 cascade-shadow tiled renderers kept out of the deep-skips form
/// unless `VICTORY_WAVE_DEEP_SKIPS_NOT` says otherwise.
pub(super) const SHADOW_PROGRAMS: [&str; 14] = [
    // CS_ShadowRenderTiledCascades{AlphaCB,AlphaPPPC,Alpha,CB,}
    "922e1d0ee1e19a85",
    "d7db57731ada919b",
    "ab5455fc6abb804f",
    "be6ce5d849d51127",
    "191ecaed66eea3b1",
    // CS_ShadowRenderTiledMisc{Alpha,CBPPPC,CB,HairAlphaPPPC,HairAlpha,HairPPPC,Hair,PPPC,}
    "31d703207b51549a",
    "56f3bf2d9388e5a7",
    "804b342cb4b9d1e9",
    "abd7a22106898c66",
    "1d44a723bdebbf27",
    "b4b0427232707598",
    "240cb516c1af0bc3",
    "466ec41d3f9d24c0",
    "65de113e23fa4fdd",
];

/// Runs `f`, the translation of the container whose hash is `hash` (see the
/// module notes): the per-program switches see it.
pub fn for_program<T>(hash: &str, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<String>);
    impl Drop for Restore {
        fn drop(&mut self) {
            PROGRAM.set(self.0.take());
        }
    }
    let _restore = Restore(PROGRAM.replace(Some(hash.to_owned())));
    f()
}

/// The hash [`for_program`] set on this thread, if any.
pub(super) fn current() -> Option<String> {
    PROGRAM.with_borrow(Clone::clone)
}

/// Whether the deep-skips form may be tried for the program whose hash is
/// `hash` (`None`: not known, as for a translation outside
/// [`for_program`]), by the switches in the module notes.
pub fn deep_skips_allowed(hash: Option<&str>) -> bool {
    struct Switches {
        on: bool,
        not: Vec<String>,
        only: Option<Vec<String>>,
    }
    static SWITCHES: OnceLock<Switches> = OnceLock::new();
    let s = SWITCHES.get_or_init(|| Switches {
        on: std::env::var_os("VICTORY_WAVE_DEEP_SKIPS").is_none_or(|v| v != "0"),
        not: match std::env::var("VICTORY_WAVE_DEEP_SKIPS_NOT") {
            Ok(v) => parse_hash_list(&v),
            Err(_) => SHADOW_PROGRAMS.iter().map(|h| (*h).to_owned()).collect(),
        },
        only: std::env::var("VICTORY_WAVE_DEEP_SKIPS_ONLY").ok().map(|v| parse_hash_list(&v)),
    });
    decide(s.on, &s.not, s.only.as_deref(), hash)
}

/// The hashes of a list: separated by commas, semicolons or white space,
/// each with or without a `#` before it, in lower case and cut to the 16
/// digits a hash has. Pure, for testing.
fn parse_hash_list(value: &str) -> Vec<String> {
    value
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(|h| h.trim_start_matches('#').to_ascii_lowercase())
        .map(|h| h.chars().take(16).collect::<String>())
        .filter(|h| !h.is_empty())
        .collect()
}

/// Whether `list` names the program `hash`: an entry names every program
/// whose hash begins with it.
fn listed(list: &[String], hash: Option<&str>) -> bool {
    hash.is_some_and(|hash| {
        let hash = hash.to_ascii_lowercase();
        list.iter().any(|entry| hash.starts_with(entry.as_str()))
    })
}

/// [`deep_skips_allowed`] on the switches' values. Pure, for testing.
fn decide(on: bool, not: &[String], only: Option<&[String]>, hash: Option<&str>) -> bool {
    on && !listed(not, hash) && only.is_none_or(|l| listed(l, hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0123456789abcdef";
    const B: &str = "fedcba9876543210";

    fn list(v: &str) -> Vec<String> {
        parse_hash_list(v)
    }

    #[test]
    fn lists_split_on_commas_and_space() {
        assert_eq!(list(""), Vec::<String>::new());
        assert_eq!(list(" , ;"), Vec::<String>::new());
        assert_eq!(list(&format!(" #{A} ;\t{B} ,")), vec![A, B]);
        assert_eq!(list("#0123456789ABCDEF"), vec![A], "upper case");
        assert_eq!(list(&format!("{A}{B}")), vec![A], "cut to 16 digits");
    }

    #[test]
    fn not_keeps_the_listed_programs_out() {
        let not = list(&format!("#{B}, 01234567"));
        assert!(!decide(true, &not, None, Some(A)), "by its first digits");
        assert!(!decide(true, &not, None, Some(B)));
        assert!(decide(true, &not, None, Some("1111111111111111")));
        assert!(decide(true, &not, None, None), "an unknown program is not listed");
        assert!(decide(true, &[], None, Some(A)), "an empty list keeps everything");
        assert!(!decide(false, &[], None, Some(A)), "=0 still wins");
    }

    #[test]
    fn only_lets_just_the_listed_programs_in() {
        let only = list(A);
        assert!(decide(true, &[], Some(&only), Some(A)));
        assert!(!decide(true, &[], Some(&only), Some(B)));
        assert!(!decide(true, &[], Some(&only), None));
    }

    #[test]
    fn the_shadow_programs_are_hashes() {
        for h in SHADOW_PROGRAMS {
            assert_eq!(h.len(), 16);
            assert!(h.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
        }
    }

    #[test]
    fn the_program_hash_is_seen_inside_its_translation_only() {
        assert_eq!(current(), None);
        let inner = for_program(A, || {
            let nested = for_program(B, current);
            (current(), nested)
        });
        assert_eq!(inner, (Some(A.to_owned()), Some(B.to_owned())));
        assert_eq!(current(), None);
    }
}
