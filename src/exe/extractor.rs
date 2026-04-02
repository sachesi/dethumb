use std::path::Path;

use crate::core::thumbnail::{IconFormat, detect_icon_format, process_raster, process_svg};

use super::cache::{ExeCacheKey, is_cache_hit, write_cache_key};
use super::error::ExeThumbError;

const EXE_FALLBACK_ICON_NAMES: &[&str] = &[
    "application-x-ms-dos-executable",
    "application-x-executable",
    "application-x-generic",
];
const BACKEND_PLACEHOLDER: &str = "freedesktop-fallback";

pub trait ExeIconExtractor {
    fn extract_best_icon(&self, path: &Path, out: &Path, size: u32) -> Result<(), ExeThumbError>;
    fn backend_name(&self) -> &'static str;
}

pub struct FallbackExeIconExtractor;

impl ExeIconExtractor for FallbackExeIconExtractor {
    fn extract_best_icon(&self, path: &Path, out: &Path, size: u32) -> Result<(), ExeThumbError> {
        ensure_readable(path)?;

        let icon_path = EXE_FALLBACK_ICON_NAMES
            .iter()
            .find_map(|icon_name| {
                freedesktop_icons::lookup(icon_name)
                    .with_size(u16::try_from(size).unwrap_or(256))
                    .find()
            })
            .ok_or_else(|| ExeThumbError::NoIconAvailable {
                path: path.to_path_buf(),
            })?;

        match detect_icon_format(&icon_path) {
            IconFormat::Svg => process_svg(&icon_path, out, size).map_err(ExeThumbError::from),
            IconFormat::Raster => {
                process_raster(&icon_path, size, out).map_err(ExeThumbError::from)
            }
            IconFormat::Unsupported => Err(ExeThumbError::NoIconResource {
                path: path.to_path_buf(),
            }),
        }
    }

    fn backend_name(&self) -> &'static str {
        BACKEND_PLACEHOLDER
    }
}

pub fn generate_exe_thumbnail(path: &Path, out: &Path, size: u32) -> Result<(), ExeThumbError> {
    if size == 0 {
        return Err(ExeThumbError::ResourceLimitExceeded {
            path: path.to_path_buf(),
        });
    }

    let extractor = FallbackExeIconExtractor;
    let cache_key =
        ExeCacheKey::compute(path, size, extractor.backend_name()).map_err(map_io(path))?;

    if is_cache_hit(out, &cache_key) {
        return Ok(());
    }

    extractor.extract_best_icon(path, out, size)?;
    write_cache_key(out, &cache_key).map_err(map_io(path))
}

fn ensure_readable(path: &Path) -> Result<(), ExeThumbError> {
    std::fs::metadata(path).map_err(map_io(path))?;
    Ok(())
}

fn map_io(path: &Path) -> impl FnOnce(std::io::Error) -> ExeThumbError + '_ {
    move |source| {
        if source.kind() == std::io::ErrorKind::PermissionDenied {
            ExeThumbError::PermissionDenied {
                path: path.to_path_buf(),
                source,
            }
        } else {
            ExeThumbError::Io {
                path: path.to_path_buf(),
                source,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::generate_exe_thumbnail;
    use tempfile::TempDir;

    #[test]
    fn rejects_zero_thumbnail_size_for_executables() {
        let tmp = TempDir::new();
        assert!(tmp.is_ok());
        let Ok(tmp) = tmp else {
            panic!("tempdir should be created");
        };

        let input = tmp.path().join("app.exe");
        let output = tmp.path().join("thumb.png");
        assert!(std::fs::write(&input, b"MZ").is_ok());

        let result = generate_exe_thumbnail(&input, &output, 0);
        assert!(result.is_err());
    }
}
