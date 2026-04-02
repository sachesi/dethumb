use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const CACHE_KEY_VERSION: &str = "exe-thumb-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExeCacheKey {
    digest: String,
}

impl ExeCacheKey {
    pub fn compute(input_path: &Path, requested_size: u32, backend: &str) -> io::Result<Self> {
        let metadata = fs::metadata(input_path)?;
        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
        let modified_duration = modified.duration_since(UNIX_EPOCH).unwrap_or_default();

        let mut hasher = blake3::Hasher::new();
        hasher.update(CACHE_KEY_VERSION.as_bytes());
        hasher.update(input_path.to_string_lossy().as_bytes());
        hasher.update(&requested_size.to_le_bytes());
        hasher.update(backend.as_bytes());
        hasher.update(&metadata.len().to_le_bytes());
        hasher.update(&modified_duration.as_secs().to_le_bytes());
        hasher.update(&modified_duration.subsec_nanos().to_le_bytes());

        Ok(Self {
            digest: hasher.finalize().to_hex().to_string(),
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.digest
    }
}

pub fn sidecar_path(output_path: &Path) -> PathBuf {
    let mut as_text = output_path.as_os_str().to_os_string();
    as_text.push(".cachekey");
    PathBuf::from(as_text)
}

pub fn is_cache_hit(output_path: &Path, cache_key: &ExeCacheKey) -> bool {
    if !output_path.is_file() {
        return false;
    }

    let sidecar = sidecar_path(output_path);
    let Ok(contents) = fs::read_to_string(sidecar) else {
        return false;
    };

    contents.trim() == cache_key.as_str()
}

pub fn write_cache_key(output_path: &Path, cache_key: &ExeCacheKey) -> io::Result<()> {
    let sidecar = sidecar_path(output_path);
    if let Some(parent) = sidecar.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(sidecar, cache_key.as_str())
}

#[cfg(test)]
mod tests {
    use super::{ExeCacheKey, is_cache_hit, sidecar_path, write_cache_key};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn cache_miss_without_sidecar() {
        let tmp = TempDir::new();
        assert!(tmp.is_ok());
        let Ok(tmp) = tmp else {
            panic!("tempdir should be created");
        };

        let input = tmp.path().join("app.exe");
        let output = tmp.path().join("thumb.png");
        assert!(fs::write(&input, b"mz").is_ok());
        assert!(fs::write(&output, b"png").is_ok());

        let key = ExeCacheKey::compute(&input, 128, "placeholder");
        assert!(key.is_ok());
        let Ok(key) = key else {
            panic!("cache key should compute");
        };

        assert!(!is_cache_hit(&output, &key));
    }

    #[test]
    fn cache_hit_with_matching_sidecar() {
        let tmp = TempDir::new();
        assert!(tmp.is_ok());
        let Ok(tmp) = tmp else {
            panic!("tempdir should be created");
        };

        let input = tmp.path().join("app.exe");
        let output = tmp.path().join("thumb.png");
        assert!(fs::write(&input, b"mz").is_ok());
        assert!(fs::write(&output, b"png").is_ok());

        let key = ExeCacheKey::compute(&input, 128, "placeholder");
        assert!(key.is_ok());
        let Ok(key) = key else {
            panic!("cache key should compute");
        };

        let write_result = write_cache_key(&output, &key);
        assert!(write_result.is_ok());

        assert!(is_cache_hit(&output, &key));
        assert!(sidecar_path(&output).is_file());
    }
}
