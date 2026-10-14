use crate::model::Model;
use anyhow::{ensure, Result};
use bincode::Options;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Instant, SystemTime},
};

const MAGIC: &[u8; 8] = b"3DVSTEP1";
const MAX_ENTRY_BYTES: u64 = 128 * 1024 * 1024;
const CACHE_BUDGET: u64 = 512 * 1024 * 1024;

pub struct StepCache {
    path: PathBuf,
}

impl StepCache {
    pub fn for_source(source: &Path, tolerance: f64) -> Result<Option<Self>> {
        if std::env::var_os("MODEL_VIEWER_DISABLE_CACHE").is_some() {
            return Ok(None);
        }
        let Some(directory) = cache_directory() else {
            return Ok(None);
        };
        let key = cache_key(source, tolerance, &std::env::current_exe()?)?;
        Ok(Some(Self {
            path: directory.join(format!("{key}.bin")),
        }))
    }

    pub fn load(&self) -> Option<Model> {
        let started = Instant::now();
        let file = File::open(&self.path).ok()?;
        let length = file.metadata().ok()?.len();
        if !(40..=MAX_ENTRY_BYTES).contains(&length) {
            return None;
        }
        let mut bytes = Vec::with_capacity(length as usize);
        file.take(MAX_ENTRY_BYTES + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() as u64 > MAX_ENTRY_BYTES || bytes.get(..8)? != MAGIC {
            return None;
        }
        let checksum = bytes.get(8..40)?;
        let payload = bytes.get(40..)?;
        if self.checksum(payload).as_bytes() != checksum {
            return None;
        }
        let model = bincode::DefaultOptions::new()
            .with_limit(MAX_ENTRY_BYTES)
            .deserialize(payload)
            .ok()?;
        if let Ok(file) = OpenOptions::new().write(true).open(&self.path) {
            let _ = file.set_modified(SystemTime::now());
        }
        trace("verified and decoded cache", started);
        Some(model)
    }

    pub fn store(&self, model: &Model) -> Result<()> {
        if self.load().is_some() {
            return Ok(());
        }
        let payload = bincode::DefaultOptions::new()
            .with_limit(MAX_ENTRY_BYTES - 40)
            .serialize(model)?;
        ensure!(
            payload.len() as u64 + 40 <= MAX_ENTRY_BYTES,
            "Mesh exceeds cache entry limit"
        );
        let directory = self.path.parent().unwrap();
        fs::create_dir_all(directory)?;
        let mut temporary = tempfile::Builder::new()
            .prefix(".step-cache-")
            .tempfile_in(directory)?;
        temporary.write_all(MAGIC)?;
        temporary.write_all(self.checksum(&payload).as_bytes())?;
        temporary.write_all(&payload)?;
        temporary.flush()?;
        if let Err(error) = temporary.persist(&self.path) {
            if self.load().is_none() {
                return Err(error.into());
            }
        }
        prune(directory, CACHE_BUDGET)?;
        Ok(())
    }

    fn checksum(&self, payload: &[u8]) -> blake3::Hash {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.path.file_name().unwrap().to_string_lossy().as_bytes());
        hasher.update(payload);
        hasher.finalize()
    }
}

fn cache_directory() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("MODEL_VIEWER_CACHE_DIR") {
        return (!directory.is_empty()).then(|| PathBuf::from(directory));
    }
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(|path| PathBuf::from(path).join("3DViewer/step-cache"))
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|path| PathBuf::from(path).join(".cache")))
            .map(|path| path.join("3DViewer/step-cache"))
    }
}

fn cache_key(source: &Path, tolerance: f64, importer: &Path) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MAGIC);
    hasher.update(&tolerance.to_bits().to_le_bytes());
    let mut buffer = [0; 64 * 1024];
    for (phase, path) in [
        ("fingerprinted importer", importer),
        ("fingerprinted STEP contents", source),
    ] {
        let started = Instant::now();
        let mut file = File::open(path)?;
        hasher.update(&file.metadata()?.len().to_le_bytes());
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        trace(phase, started);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn trace(phase: &str, started: Instant) {
    if std::env::var_os("MODEL_VIEWER_IMPORT_TRACE").is_some() {
        eprintln!("STEP: {phase} in {:.3}s", started.elapsed().as_secs_f64());
    }
}

fn prune(directory: &Path, budget: u64) -> Result<()> {
    let mut entries = Vec::new();
    let mut total = 0;
    for entry in fs::read_dir(directory)?.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let is_cache = name
            .strip_suffix(".bin")
            .is_some_and(|key| key.len() == 64 && key.bytes().all(|byte| byte.is_ascii_hexdigit()));
        if !is_cache && !name.starts_with(".step-cache-") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        total += metadata.len();
        entries.push((
            metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            metadata.len(),
            entry.path(),
        ));
    }
    entries.sort_by_key(|entry| entry.0);
    for (_, length, path) in entries {
        if total <= budget {
            break;
        }
        if fs::remove_file(path).is_ok() {
            total -= length;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn keys_track_contents_tolerance_and_importer_not_filename() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("part.step");
        let renamed = directory.path().join("renamed.STEP");
        let importer = directory.path().join("viewer.exe");
        fs::write(&source, b"CAD geometry A").unwrap();
        fs::write(&renamed, b"CAD geometry A").unwrap();
        fs::write(&importer, b"importer version A").unwrap();
        let key = cache_key(&source, 0.001, &importer).unwrap();
        assert_eq!(key, cache_key(&renamed, 0.001, &importer).unwrap());
        assert_ne!(key, cache_key(&source, 0.0001, &importer).unwrap());
        fs::write(&source, b"CAD geometry B").unwrap();
        assert_ne!(key, cache_key(&source, 0.001, &importer).unwrap());
        fs::write(&source, b"CAD geometry A").unwrap();
        fs::write(&importer, b"importer version B").unwrap();
        assert_ne!(key, cache_key(&source, 0.001, &importer).unwrap());
    }

    #[test]
    fn cache_preserves_exact_model_and_warnings_and_rejects_corruption() {
        let directory = tempfile::tempdir().unwrap();
        let cache = StepCache {
            path: directory.path().join(format!("{}.bin", "a".repeat(64))),
        };
        let mut model = crate::model::demo();
        model.warnings.push("Incomplete STEP model".into());
        assert!(cache.load().is_none());
        cache.store(&model).unwrap();
        let decoded = cache.load().unwrap();
        assert_eq!(
            bincode::serialize(&model).unwrap(),
            bincode::serialize(&decoded).unwrap()
        );
        let mut bytes = fs::read(&cache.path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&cache.path, &bytes).unwrap();
        assert!(cache.load().is_none());
        fs::write(&cache.path, &bytes[..20]).unwrap();
        assert!(cache.load().is_none());
        cache.store(&model).unwrap();
        assert!(cache.load().is_some());
        let wrong_key = StepCache {
            path: directory.path().join(format!("{}.bin", "b".repeat(64))),
        };
        fs::copy(&cache.path, &wrong_key.path).unwrap();
        assert!(wrong_key.load().is_none());
    }

    #[test]
    fn concurrent_reads_and_atomic_writes_preserve_models() {
        let directory = tempfile::tempdir().unwrap();
        let cache = StepCache {
            path: directory.path().join(format!("{}.bin", "a".repeat(64))),
        };
        let model = crate::model::demo();
        let barrier = std::sync::Barrier::new(4);
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let cache = &cache;
                let model = &model;
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    for _ in 0..8 {
                        cache.store(model).unwrap();
                        let decoded = cache.load().unwrap();
                        assert_eq!(
                            bincode::serialize(model).unwrap(),
                            bincode::serialize(&decoded).unwrap()
                        );
                    }
                });
            }
        });
    }

    #[test]
    fn pruning_removes_old_entries_but_not_unrelated_files() {
        let directory = tempfile::tempdir().unwrap();
        let old = directory.path().join(format!("{}.bin", "a".repeat(64)));
        let recent = directory.path().join(format!("{}.bin", "b".repeat(64)));
        fs::write(&old, [0; 100]).unwrap();
        fs::write(&recent, [0; 100]).unwrap();
        fs::write(directory.path().join("keep.txt"), [0; 100]).unwrap();
        for (offset, path) in [(1, &old), (2, &recent)] {
            File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(offset))
                .unwrap();
        }
        prune(directory.path(), 100).unwrap();
        assert!(!old.exists());
        assert!(recent.exists());
        assert!(directory.path().join("keep.txt").exists());
    }
}
