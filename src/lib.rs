use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use freedesktop_entry_parser::parse_entry;
use thiserror::Error;

pub mod desktop;
pub mod exe;

use desktop::icon_lookup::{find_icon_path, get_current_theme};
use desktop::path_safety::has_parent_dir_component;
use desktop::thumbnail::{
    IconFormat, ThumbnailError, create_fallback_thumbnail, detect_icon_format, process_raster,
    process_svg,
};
use exe::detector::{InputKind, detect_input_kind};
use exe::error::ExeThumbError;
use exe::extractor::generate_exe_thumbnail;

const DEFAULT_SIZE: u32 = 256;

#[derive(Debug, Clone)]
pub struct CliArgs {
    input_path: PathBuf,
    output_path: PathBuf,
    size: u32,
}

impl CliArgs {
    #[must_use]
    pub fn new(input_path: PathBuf, output_path: PathBuf, size: u32) -> Self {
        Self {
            input_path,
            output_path,
            size: if size == 0 { DEFAULT_SIZE } else { size },
        }
    }

    pub fn parse_from_env() -> Result<Self, AppError> {
        let args: Vec<String> = env::args().collect();
        Self::parse_from_slice(&args)
    }

    pub fn parse_from_slice(args: &[String]) -> Result<Self, AppError> {
        if args.len() != 4 {
            return Err(AppError::Usage(format!(
                "Usage: {} <input.desktop|input.exe> <out.png> <size>",
                args[0]
            )));
        }

        let input_path = PathBuf::from(&args[1]);
        let output_path = PathBuf::from(&args[2]);

        if has_parent_dir_component(&output_path) {
            return Err(AppError::UnsafeOutputPath(output_path));
        }

        let parsed_size = args[3]
            .parse::<u32>()
            .map_err(|source| AppError::InvalidSize {
                value: args[3].clone(),
                source,
            })?;

        Ok(Self::new(input_path, output_path, parsed_size))
    }

    #[must_use]
    pub fn output_path(&self) -> &Path {
        &self.output_path
    }

    #[must_use]
    pub fn size(&self) -> u32 {
        self.size
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Usage(String),
    #[error("Refusing unsafe output path with parent traversal: {0}")]
    UnsafeOutputPath(PathBuf),
    #[error("Bad size '{value}': {source}")]
    InvalidSize {
        value: String,
        source: std::num::ParseIntError,
    },
    #[error("Canon failed '{path}': {source}")]
    Canonicalize {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Parse .desktop failed: {0}")]
    DesktopParse(String),
    #[error("Unsupported input type: {0}")]
    UnsupportedInputType(String),
    #[error("No Icon= in .desktop")]
    MissingIcon,
    #[error("No valid icon path found for: {0}")]
    IconNotFound(String),
    #[error("Unsupported extension on {0}")]
    UnsupportedExtension(String),
    #[error(transparent)]
    ExeThumbnail(#[from] ExeThumbError),
    #[error(transparent)]
    Thumbnail(#[from] ThumbnailError),
}

pub fn run() -> Result<(), AppError> {
    let args = CliArgs::parse_from_env()?;
    run_with_args(&args)
}

pub fn run_with_args(args: &CliArgs) -> Result<(), AppError> {
    let input_path =
        fs::canonicalize(&args.input_path).map_err(|source| AppError::Canonicalize {
            path: args.input_path.clone(),
            source,
        })?;

    match detect_input_kind(&input_path) {
        InputKind::DesktopEntry => process_desktop_entry(&input_path, &args.output_path, args.size),
        InputKind::Executable => generate_exe_thumbnail(&input_path, &args.output_path, args.size)
            .map_err(AppError::from),
        InputKind::Unsupported => Err(AppError::UnsupportedInputType(
            input_path.display().to_string(),
        )),
    }
}

fn process_desktop_entry(input_path: &Path, output_path: &Path, size: u32) -> Result<(), AppError> {
    let entry =
        parse_entry(input_path).map_err(|source| AppError::DesktopParse(source.to_string()))?;
    let icon = entry
        .section("Desktop Entry")
        .attr("Icon")
        .filter(|icon| !icon.trim().is_empty())
        .ok_or(AppError::MissingIcon)?;

    let theme = get_current_theme().unwrap_or_else(|| "hicolor".to_owned());

    let icon_path = find_icon_path(icon, &theme, size)
        .ok_or_else(|| AppError::IconNotFound(icon.to_owned()))?;

    render_icon(&icon_path, output_path, size)
}

fn render_icon(icon_path: &Path, output_path: &Path, size: u32) -> Result<(), AppError> {
    match detect_icon_format(icon_path) {
        IconFormat::Svg => process_svg(icon_path, output_path, size).map_err(AppError::from),
        IconFormat::Raster => process_raster(icon_path, size, output_path).map_err(AppError::from),
        IconFormat::Unsupported => Err(AppError::UnsupportedExtension(
            icon_path.display().to_string(),
        )),
    }
}

#[must_use]
pub fn run_with_fallback() -> i32 {
    match run() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("{err}");
            match CliArgs::parse_from_env() {
                Ok(args) => create_fallback_thumbnail(args.output_path(), args.size()),
                Err(parse_err) => eprintln!("{parse_err}"),
            }
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CliArgs;

    #[test]
    fn maps_zero_size_to_default() {
        let argv = vec![
            "dethumb".to_string(),
            "in.desktop".to_string(),
            "out.png".to_string(),
            "0".to_string(),
        ];

        let parsed = CliArgs::parse_from_slice(&argv);
        assert!(parsed.is_ok());
        if let Ok(parsed) = parsed {
            assert_eq!(parsed.size(), 256);
        }
    }
}
