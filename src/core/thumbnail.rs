use std::fs::{self, File};
use std::path::Path;

use freedesktop_icons::lookup;
use image::{DynamicImage, GenericImageView, ImageFormat};
use resvg::usvg::{self, Tree};
use thiserror::Error;
use tiny_skia::{IntSize, Pixmap, Transform};

const DEFAULT_FALLBACK_ICON: &str = "application-x-generic";
const FALLBACK_THEME: &str = "Adwaita";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconFormat {
    Svg,
    Raster,
    Unsupported,
}

#[derive(Debug, Error)]
pub enum ThumbnailError {
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
    let data = fs::read(path).map_err(|source| ThumbnailError::ReadSvg {
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
    let img = image::open(path).map_err(|source| ThumbnailError::DecodeImage {
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
    use super::IconFormat;
    use super::detect_icon_format;
    use std::path::Path;

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
}
