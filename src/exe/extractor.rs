use std::path::Path;

use crate::core::thumbnail::{IconFormat, detect_icon_format, process_raster, process_svg};

use super::error::ExeThumbError;

const EXE_FALLBACK_ICON_NAMES: &[&str] = &[
    "application-x-ms-dos-executable",
    "application-x-executable",
    "application-x-generic",
];

pub fn generate_exe_thumbnail(path: &Path, out: &Path, size: u32) -> Result<(), ExeThumbError> {
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
        IconFormat::Raster => process_raster(&icon_path, size, out).map_err(ExeThumbError::from),
        IconFormat::Unsupported => Err(ExeThumbError::NoIconAvailable {
            path: path.to_path_buf(),
        }),
    }
}

fn ensure_readable(path: &Path) -> Result<(), ExeThumbError> {
    std::fs::metadata(path).map_err(|source| ExeThumbError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}
