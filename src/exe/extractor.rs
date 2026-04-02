use std::path::Path;

use crate::desktop::thumbnail::{IconFormat, detect_icon_format, process_raster, process_svg};

use super::backends::pe_resource::PeResourceIconExtractor;
use super::backends::windows_shell::WindowsShellIconExtractor;
use super::cache::{ExeCacheKey, is_cache_hit, write_cache_key};
use super::error::ExeThumbError;
use super::pe::validate_executable_header;
use super::telemetry::{
    FallbackReason, record_cache_hit, record_cache_miss, record_extraction_attempt,
    record_extraction_success, record_fallback_reason,
};

const EXE_FALLBACK_ICON_NAMES: &[&str] = &[
    "application-x-ms-dos-executable",
    "application-x-executable",
    "application-x-generic",
];
const BACKEND_CHAIN_MARKER: &str = "windows-shell|pe-resource|freedesktop-fallback";
const FALLBACK_BACKEND_NAME: &str = "freedesktop-fallback";
const MAX_EXE_BYTES: u64 = 512 * 1024 * 1024;

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
        FALLBACK_BACKEND_NAME
    }
}

pub fn generate_exe_thumbnail(path: &Path, out: &Path, size: u32) -> Result<(), ExeThumbError> {
    if size == 0 {
        return Err(ExeThumbError::ResourceLimitExceeded {
            path: path.to_path_buf(),
        });
    }

    validate_pe_header(path)?;

    let cache_key = ExeCacheKey::compute(path, size, BACKEND_CHAIN_MARKER).map_err(map_io(path))?;

    if is_cache_hit(out, &cache_key) {
        record_cache_hit();
        return Ok(());
    }

    record_cache_miss();
    record_extraction_attempt();

    let extractors: [&dyn ExeIconExtractor; 3] = [
        &WindowsShellIconExtractor,
        &PeResourceIconExtractor,
        &FallbackExeIconExtractor,
    ];

    let mut last_error = ExeThumbError::NoIconAvailable {
        path: path.to_path_buf(),
    };

    for extractor in extractors {
        match extractor.extract_best_icon(path, out, size) {
            Ok(()) => {
                record_extraction_success();
                return write_cache_key(out, &cache_key).map_err(map_io(path));
            }
            Err(err) => {
                record_fallback_reason(reason_for_error(&err));
                if !is_retryable_backend_error(&err) {
                    return Err(err);
                }
                last_error = err;
            }
        }
    }

    Err(last_error)
}

fn validate_pe_header(path: &Path) -> Result<(), ExeThumbError> {
    let metadata = std::fs::metadata(path).map_err(map_io(path))?;
    if metadata.len() > MAX_EXE_BYTES {
        return Err(ExeThumbError::ResourceLimitExceeded {
            path: path.to_path_buf(),
        });
    }

    let mut file = std::fs::File::open(path).map_err(map_io(path))?;
    let is_valid = validate_executable_header(&mut file, metadata.len()).map_err(map_io(path))?;
    if !is_valid {
        return Err(ExeThumbError::InvalidPeFormat {
            path: path.to_path_buf(),
        });
    }

    Ok(())
}

fn ensure_readable(path: &Path) -> Result<(), ExeThumbError> {
    std::fs::metadata(path).map_err(map_io(path))?;
    Ok(())
}

fn reason_for_error(error: &ExeThumbError) -> FallbackReason {
    match error {
        ExeThumbError::UnsupportedPlatform => FallbackReason::UnsupportedPlatform,
        ExeThumbError::NoIconAvailable { .. } => FallbackReason::NoIconAvailable,
        ExeThumbError::NoIconResource { .. } => FallbackReason::UnsupportedIconFormat,
        ExeThumbError::InvalidPeFormat { .. } => FallbackReason::InvalidPeFormat,
        ExeThumbError::PermissionDenied { .. } => FallbackReason::PermissionDenied,
        ExeThumbError::Io { .. } => FallbackReason::Io,
        _ => FallbackReason::Other,
    }
}

fn is_retryable_backend_error(error: &ExeThumbError) -> bool {
    matches!(
        error,
        ExeThumbError::UnsupportedPlatform
            | ExeThumbError::NoIconAvailable { .. }
            | ExeThumbError::NoIconResource { .. }
            | ExeThumbError::DecodeFailed { .. }
    )
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
    use crate::exe::error::ExeThumbError;
    use crate::exe::telemetry::{FallbackReason, reset, snapshot};
    use std::io::{Seek, SeekFrom, Write};
    use std::path::Path;
    use tempfile::TempDir;

    fn write_minimal_pe(path: &Path) {
        let mut bytes = vec![0_u8; 512];
        bytes[0] = b'M';
        bytes[1] = b'Z';

        let pe_offset: u32 = 0x80;
        bytes[0x3c..0x40].copy_from_slice(&pe_offset.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        bytes[0x84..0x86].copy_from_slice(&0x8664_u16.to_le_bytes());
        bytes[0x86..0x88].copy_from_slice(&4_u16.to_le_bytes());
        bytes[0x94..0x96].copy_from_slice(&0x00F0_u16.to_le_bytes());
        bytes[0x98..0x9a].copy_from_slice(&0x020b_u16.to_le_bytes());

        assert!(std::fs::write(path, bytes).is_ok());
    }

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

    #[test]
    fn rejects_non_pe_executable_and_tracks_reason() {
        reset();

        let tmp = TempDir::new();
        assert!(tmp.is_ok());
        let Ok(tmp) = tmp else {
            panic!("tempdir should be created");
        };

        let input = tmp.path().join("app.exe");
        let output = tmp.path().join("thumb.png");
        assert!(std::fs::write(&input, b"NOPE").is_ok());

        let result = generate_exe_thumbnail(&input, &output, 64);
        assert!(result.is_err());

        let snapshot = snapshot();
        assert_eq!(snapshot.extraction_attempts, 0);
        assert!(
            !snapshot
                .fallback_reasons
                .contains_key(&FallbackReason::NoIconAvailable)
        );
    }

    #[test]
    fn rejects_invalid_pe_offset() {
        let tmp = TempDir::new();
        assert!(tmp.is_ok());
        let Ok(tmp) = tmp else {
            panic!("tempdir should be created");
        };

        let input = tmp.path().join("broken_offset.exe");
        let output = tmp.path().join("thumb.png");

        let mut bytes = vec![0_u8; 64];
        bytes[0] = b'M';
        bytes[1] = b'Z';
        let invalid_offset: u32 = 0xFFFF_FFF0;
        bytes[0x3c..0x40].copy_from_slice(&invalid_offset.to_le_bytes());
        assert!(std::fs::write(&input, bytes).is_ok());

        let result = generate_exe_thumbnail(&input, &output, 64);
        assert!(matches!(result, Err(ExeThumbError::InvalidPeFormat { .. })));
    }

    #[test]
    fn rejects_bad_pe_signature() {
        let tmp = TempDir::new();
        assert!(tmp.is_ok());
        let Ok(tmp) = tmp else {
            panic!("tempdir should be created");
        };

        let input = tmp.path().join("broken_signature.exe");
        let output = tmp.path().join("thumb.png");
        write_minimal_pe(&input);

        let patch_result = std::fs::OpenOptions::new().write(true).open(&input);
        assert!(patch_result.is_ok());
        let Ok(mut file) = patch_result else {
            panic!("test executable should be writable");
        };
        assert!(file.seek(SeekFrom::Start(0x80)).is_ok());
        assert!(file.write_all(b"PX\0\0").is_ok());

        let result = generate_exe_thumbnail(&input, &output, 64);
        assert!(matches!(result, Err(ExeThumbError::InvalidPeFormat { .. })));
    }

    #[test]
    fn rejects_executables_over_resource_limit() {
        let tmp = TempDir::new();
        assert!(tmp.is_ok());
        let Ok(tmp) = tmp else {
            panic!("tempdir should be created");
        };

        let input = tmp.path().join("large.exe");
        let output = tmp.path().join("thumb.png");

        let open_result = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&input);
        assert!(open_result.is_ok());
        let Ok(file) = open_result else {
            panic!("test executable should be creatable");
        };

        let set_len_result = file.set_len(super::MAX_EXE_BYTES + 1);
        assert!(set_len_result.is_ok());

        let result = generate_exe_thumbnail(&input, &output, 64);
        assert!(matches!(
            result,
            Err(ExeThumbError::ResourceLimitExceeded { .. })
        ));
    }
}
