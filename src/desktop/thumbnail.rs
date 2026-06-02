use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::Path;

use freedesktop_icons::lookup;
use image::{DynamicImage, GenericImageView, ImageFormat};
use resvg::usvg::{self, Tree};
use thiserror::Error;
use tiny_skia::{IntSize, Pixmap, Transform};

const DEFAULT_FALLBACK_ICON: &str = "application-x-generic";
const FALLBACK_THEME: &str = "Adwaita";
/// Largest icon file we are willing to read into memory.
const MAX_ICON_BYTES: u64 = 64 * 1024 * 1024;
/// Strict per-axis pixel limit applied when decoding raster icons.
const MAX_DECODE_DIM: u32 = 8192;
/// Upper bound on decoder allocations for a single raster icon.
const MAX_DECODE_ALLOC: u64 = 256 * 1024 * 1024;

fn decode_limits() -> image::Limits {
    let mut limits = image::Limits::no_limits();
    limits.max_image_width = Some(MAX_DECODE_DIM);
    limits.max_image_height = Some(MAX_DECODE_DIM);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    limits
}

/// Open an icon file, confirming it is a regular file within the size limit.
///
/// Stat-ing the open handle (rather than the path) shrinks the window between
/// validation and use, and the regular-file check rejects swaps to a
/// directory/FIFO between lookup and read.
fn open_capped(path: &Path) -> Result<File, ThumbnailError> {
    let file = File::open(path).map_err(|source| ThumbnailError::OpenIcon {
        path: path.display().to_string(),
        source,
    })?;
    let metadata = file.metadata().map_err(|source| ThumbnailError::OpenIcon {
        path: path.display().to_string(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(ThumbnailError::NotRegularFile(path.display().to_string()));
    }
    if metadata.len() > MAX_ICON_BYTES {
        return Err(ThumbnailError::FileTooLarge {
            path: path.display().to_string(),
            size: metadata.len(),
        });
    }
    Ok(file)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconFormat {
    Svg,
    Raster,
    Unsupported,
}

#[derive(Debug, Error)]
pub enum ThumbnailError {
    #[error("Failed to open icon '{path}': {source}")]
    OpenIcon {
        path: String,
        source: std::io::Error,
    },
    #[error("Icon is not a regular file: {0}")]
    NotRegularFile(String),
    #[error("Icon '{path}' exceeds size limit ({size} bytes)")]
    FileTooLarge { path: String, size: u64 },
    #[error("Failed to read SVG '{path}': {source}")]
    ReadSvg {
        path: String,
        source: std::io::Error,
    },
    #[error("Failed to parse SVG '{path}': {source}")]
    ParseSvg { path: String, source: usvg::Error },
    #[error("Invalid output size: {0}")]
    InvalidSize(u32),
    #[error("Failed to create pixmap")]
    PixmapCreate,
    #[error("Failed to encode PNG: {0}")]
    EncodePng(String),
    #[error("Failed to decode image '{path}': {source}")]
    DecodeImage {
        path: String,
        source: image::ImageError,
    },
    #[error("Failed to create directory '{path}': {source}")]
    CreateDirectory {
        path: String,
        source: std::io::Error,
    },
    #[error("Failed to create output file '{path}': {source}")]
    CreateOutput {
        path: String,
        source: std::io::Error,
    },
    #[error("Failed to write PNG '{path}': {source}")]
    WritePng {
        path: String,
        source: image::ImageError,
    },
    #[error("Failed to write output '{path}': {source}")]
    WriteBytes {
        path: String,
        source: std::io::Error,
    },
}

#[must_use]
pub fn detect_icon_format(path: &Path) -> IconFormat {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("svg") => IconFormat::Svg,
        Some("png" | "jpg" | "jpeg") => IconFormat::Raster,
        _ => IconFormat::Unsupported,
    }
}

/// Render an SVG icon to a PNG thumbnail file.
pub fn process_svg(path: &Path, out: &Path, size: u32) -> Result<(), ThumbnailError> {
    let mut file = open_capped(path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|source| ThumbnailError::ReadSvg {
            path: path.display().to_string(),
            source,
        })?;
    let options = usvg::Options::default();
    let tree = Tree::from_data(&data, &options).map_err(|source| ThumbnailError::ParseSvg {
        path: path.display().to_string(),
        source,
    })?;

    let pixmap_size = IntSize::from_wh(size, size).ok_or(ThumbnailError::InvalidSize(size))?;
    let mut pixmap = Pixmap::new(pixmap_size.width(), pixmap_size.height())
        .ok_or(ThumbnailError::PixmapCreate)?;

    let svg_size = tree.size();
    if svg_size.width() <= 0.0 || svg_size.height() <= 0.0 {
        return Err(ThumbnailError::InvalidSize(size));
    }
    let scale = (size as f32 / svg_size.width()).min(size as f32 / svg_size.height());
    let tx = Transform::from_row(
        scale,
        0.0,
        0.0,
        scale,
        ((size as f32) - (svg_size.width() * scale)) / 2.0,
        ((size as f32) - (svg_size.height() * scale)) / 2.0,
    );

    resvg::render(&tree, tx, &mut pixmap.as_mut());
    let png = pixmap
        .encode_png()
        .map_err(|source| ThumbnailError::EncodePng(source.to_string()))?;

    write_bytes(out, &png)
}

/// Read and resize raster icon data into a centered PNG thumbnail.
pub fn process_raster(path: &Path, size: u32, out_png: &Path) -> Result<(), ThumbnailError> {
    let file = open_capped(path)?;
    let mut reader = image::ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .map_err(|source| ThumbnailError::DecodeImage {
            path: path.display().to_string(),
            source: image::ImageError::IoError(source),
        })?;
    reader.limits(decode_limits());
    let img = reader.decode().map_err(|source| ThumbnailError::DecodeImage {
        path: path.display().to_string(),
        source,
    })?;
    let thumb = resize_image(&img, size);
    write_image(out_png, &thumb)
}

fn resize_image(img: &DynamicImage, size: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 || size == 0 {
        return DynamicImage::new_rgba8(size.max(1), size.max(1));
    }

    let ratio = size as f32 / w.max(h) as f32;
    let nw = (w as f32 * ratio).round().max(1.0) as u32;
    let nh = (h as f32 * ratio).round().max(1.0) as u32;
    let mut out = DynamicImage::new_rgba8(size, size);
    let small = img.resize_exact(nw, nh, image::imageops::FilterType::Triangle);
    let x = (size - nw) / 2;
    let y = (size - nh) / 2;
    image::imageops::overlay(&mut out, &small, x.into(), y.into());
    out
}

/// Create a fallback thumbnail when icon-specific processing fails.
pub fn create_fallback_thumbnail(out: &Path, size: u32) {
    let lookup_size = u16::try_from(size).unwrap_or(256);

    let fallback_icon = lookup(DEFAULT_FALLBACK_ICON)
        .with_size(lookup_size)
        .with_theme(FALLBACK_THEME)
        .find();

    let Some(path) = fallback_icon else {
        eprintln!("Failed to find fallback icon: {DEFAULT_FALLBACK_ICON}");
        return;
    };

    let result = match detect_icon_format(&path) {
        IconFormat::Svg => process_svg(&path, out, size),
        IconFormat::Raster => process_raster(&path, size, out),
        IconFormat::Unsupported => {
            eprintln!("Unsupported fallback extension: {}", path.display());
            return;
        }
    };

    if let Err(err) = result {
        eprintln!(
            "Failed to create fallback thumbnail from {}: {err}",
            path.display()
        );
    }
}

fn write_image(path: &Path, image: &DynamicImage) -> Result<(), ThumbnailError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|source| ThumbnailError::CreateDirectory {
            path: dir.display().to_string(),
            source,
        })?;
    }

    let mut file = File::create(path).map_err(|source| ThumbnailError::CreateOutput {
        path: path.display().to_string(),
        source,
    })?;

    image
        .write_to(&mut file, ImageFormat::Png)
        .map_err(|source| ThumbnailError::WritePng {
            path: path.display().to_string(),
            source,
        })
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), ThumbnailError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|source| ThumbnailError::CreateDirectory {
            path: dir.display().to_string(),
            source,
        })?;
    }

    fs::write(path, bytes).map_err(|source| ThumbnailError::WriteBytes {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        IconFormat, MAX_ICON_BYTES, ThumbnailError, detect_icon_format, open_capped, process_raster,
    };
    use image::{ImageBuffer, Rgba};
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn recognizes_supported_extensions_case_insensitively() {
        assert_eq!(detect_icon_format(Path::new("icon.SVG")), IconFormat::Svg);
        assert_eq!(
            detect_icon_format(Path::new("icon.JpEg")),
            IconFormat::Raster
        );
        assert_eq!(
            detect_icon_format(Path::new("icon.txt")),
            IconFormat::Unsupported
        );
    }

    fn write_png(path: &Path, width: u32, height: u32) {
        let image = ImageBuffer::from_pixel(width, height, Rgba([1_u8, 2, 3, 255]));
        let mut bytes = Vec::new();
        let encoded = image::DynamicImage::ImageRgba8(image).write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        );
        assert!(encoded.is_ok());
        assert!(std::fs::write(path, bytes).is_ok());
    }

    #[test]
    fn open_capped_rejects_oversized_file() {
        let Ok(tmp) = TempDir::new() else {
            panic!("tempdir should be created");
        };
        let path = tmp.path().join("huge.png");
        let Ok(file) = std::fs::File::create(&path) else {
            panic!("file should be created");
        };
        assert!(file.set_len(MAX_ICON_BYTES + 1).is_ok());
        assert!(matches!(
            open_capped(&path),
            Err(ThumbnailError::FileTooLarge { .. })
        ));
    }

    #[test]
    fn open_capped_rejects_non_regular_file() {
        let Ok(tmp) = TempDir::new() else {
            panic!("tempdir should be created");
        };
        assert!(matches!(
            open_capped(tmp.path()),
            Err(ThumbnailError::NotRegularFile(_))
        ));
    }

    #[test]
    fn process_raster_rejects_image_over_dimension_limit() {
        let Ok(tmp) = TempDir::new() else {
            panic!("tempdir should be created");
        };
        let input = tmp.path().join("wide.png");
        let output = tmp.path().join("thumb.png");
        // Width exceeds MAX_DECODE_DIM; data stays tiny so the test is cheap.
        write_png(&input, 9000, 1);
        assert!(process_raster(&input, 64, &output).is_err());
    }

    #[test]
    fn process_raster_decodes_icon_within_limits() {
        let Ok(tmp) = TempDir::new() else {
            panic!("tempdir should be created");
        };
        let input = tmp.path().join("ok.png");
        let output = tmp.path().join("thumb.png");
        write_png(&input, 16, 16);
        assert!(process_raster(&input, 64, &output).is_ok());
        assert!(output.is_file());
    }
}
