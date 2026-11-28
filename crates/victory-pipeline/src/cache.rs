use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::Converter;

/// Identifies one conversion result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey(blake3::Hash);

impl CacheKey {
    pub fn new(converter: &dyn Converter, input: &[u8]) -> Self {
        let id = converter.id();
        let mut h = blake3::Hasher::new();
        // Length-prefix the id so a NUL inside it cannot be mistaken for the
        // old separator and collide with a different id/version pair.
        h.update(&(id.len() as u64).to_le_bytes());
        h.update(id.as_bytes());
        h.update(&converter.version().to_le_bytes());
        h.update(input);
        CacheKey(h.finalize())
    }

    pub fn to_hex(&self) -> String {
        self.0.to_hex().to_string()
    }
}

/// Content-addressed store under the user's cache directory.
#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    /// Opens (creating) a cache at `root`. Refuses locations inside the repo.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        // The title can change the process's working directory after startup.
        let root =
            std::path::absolute(&root).with_context(|| format!("resolving {}", root.display()))?;
        victory_core::ensure_outside_repo(&root)?;
        std::fs::create_dir_all(&root).with_context(|| format!("creating {}", root.display()))?;
        Ok(Cache { root })
    }

    pub fn path_for(&self, key: &CacheKey) -> PathBuf {
        let hex = key.to_hex();
        self.root.join(&hex[..2]).join(&hex[2..])
    }

    /// Returns the cached output for `input`, converting it first if needed.
    pub fn get_or_convert(&self, converter: &dyn Converter, input: &[u8]) -> Result<PathBuf> {
        let key = CacheKey::new(converter, input);
        let path = self.path_for(&key);
        if path.is_file() {
            tracing::trace!(converter = converter.id(), key = %key.to_hex(), "cache hit");
            return Ok(path);
        }
        let output = converter
            .convert(input)
            .with_context(|| format!("converter {} v{}", converter.id(), converter.version()))?;
        let dir = path.parent().expect("hashed path has a parent");
        std::fs::create_dir_all(dir)?;
        // Write to a temporary file on the same filesystem and then publish.
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        file.write_all(&output)?;
        file.persist(&path)
            .map_err(|err| err.error)
            .with_context(|| format!("publishing {}", path.display()))?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Upper {
        version: u32,
        runs: AtomicUsize,
    }

    impl Converter for Upper {
        fn id(&self) -> &'static str {
            "test.upper"
        }
        fn version(&self) -> u32 {
            self.version
        }
        fn convert(&self, input: &[u8]) -> Result<Vec<u8>> {
            self.runs.fetch_add(1, Ordering::Relaxed);
            Ok(input.to_ascii_uppercase())
        }
    }

    fn in_child(test: &str) -> bool {
        const CHILD: &str = "VICTORY_TEST_CACHE_CHILD";
        if std::env::var(CHILD).as_deref() == Ok(test) {
            return true;
        }
        let tmp = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &format!("cache::tests::{test}")])
            .env(CHILD, test)
            .current_dir(tmp.path())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        false
    }

    #[test]
    fn relative_cache_survives_a_working_directory_change() {
        // Changing the working directory must not affect other tests.
        if !in_child("relative_cache_survives_a_working_directory_change") {
            return;
        }

        let startup_dir = std::env::current_dir().unwrap();
        let cache = Cache::open("cache").unwrap();
        let title_dir = startup_dir.join("title");
        std::fs::create_dir(&title_dir).unwrap();
        std::env::set_current_dir(&title_dir).unwrap();
        let converter = Upper {
            version: 1,
            runs: AtomicUsize::new(0),
        };
        let path = cache.get_or_convert(&converter, b"abc").unwrap();
        assert!(path.starts_with(startup_dir.join("cache")));
        assert_eq!(std::fs::read(path).unwrap(), b"ABC");
        assert!(!title_dir.join("cache").exists());
    }

    #[test]
    fn recycled_process_id_can_stage_a_new_entry() {
        // A fresh process starts with the old staging counter at zero.
        if !in_child("recycled_process_id_can_stage_a_new_entry") {
            return;
        }

        let cache = Cache::open("cache").unwrap();
        let converter = Upper {
            version: 1,
            runs: AtomicUsize::new(0),
        };
        let path = cache.path_for(&CacheKey::new(&converter, b"abc"));
        let dir = path.parent().unwrap();
        std::fs::create_dir_all(dir).unwrap();
        let stale = dir.join(format!(".tmp-{}-0", std::process::id()));
        std::fs::write(&stale, b"abandoned").unwrap();

        assert_eq!(cache.get_or_convert(&converter, b"abc").unwrap(), path);
        assert_eq!(std::fs::read(path).unwrap(), b"ABC");
        assert_eq!(std::fs::read(stale).unwrap(), b"abandoned");
    }

    #[test]
    fn converts_once_then_hits() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = Cache::open(tmp.path().join("cache")).unwrap();
        let conv = Upper {
            version: 1,
            runs: AtomicUsize::new(0),
        };
        let a = cache.get_or_convert(&conv, b"abc").unwrap();
        let b = cache.get_or_convert(&conv, b"abc").unwrap();
        assert_eq!(a, b);
        assert_eq!(std::fs::read(&a).unwrap(), b"ABC");
        assert_eq!(conv.runs.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn failed_publication_removes_the_staging_file() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = Cache::open(tmp.path().join("cache")).unwrap();
        let conv = Upper {
            version: 1,
            runs: AtomicUsize::new(0),
        };
        let path = cache.path_for(&CacheKey::new(&conv, b"abc"));
        std::fs::create_dir_all(&path).unwrap();

        assert!(cache.get_or_convert(&conv, b"abc").is_err());
        assert!(path.is_dir());
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[test]
    fn version_bump_invalidates() {
        let v1 = Upper {
            version: 1,
            runs: AtomicUsize::new(0),
        };
        let v2 = Upper {
            version: 2,
            runs: AtomicUsize::new(0),
        };
        assert_ne!(CacheKey::new(&v1, b"x"), CacheKey::new(&v2, b"x"));
    }

    struct Keyed(&'static str, u32);

    impl Converter for Keyed {
        fn id(&self) -> &'static str {
            self.0
        }
        fn version(&self) -> u32 {
            self.1
        }
        fn convert(&self, _: &[u8]) -> Result<Vec<u8>> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn id_and_version_are_delimited() {
        assert_ne!(
            CacheKey::new(&Keyed("a\0", 1), b""),
            CacheKey::new(&Keyed("a", 256), b"")
        );
        assert_ne!(
            CacheKey::new(&Keyed("a", 1), b"b"),
            CacheKey::new(&Keyed("a", 1), b"")
        );
    }

    #[test]
    fn refuses_cache_inside_repo() {
        let inside = Path::new(env!("CARGO_MANIFEST_DIR")).join("cache");
        assert!(Cache::open(inside).is_err());
    }
}
