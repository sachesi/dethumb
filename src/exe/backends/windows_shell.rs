use std::path::Path;

use crate::exe::error::ExeThumbError;
use crate::exe::extractor::ExeIconExtractor;

pub struct WindowsShellIconExtractor;

impl ExeIconExtractor for WindowsShellIconExtractor {
    fn extract_best_icon(&self, path: &Path, out: &Path, size: u32) -> Result<(), ExeThumbError> {
        extract_via_windows_shell(path, out, size)
    }

    fn backend_name(&self) -> &'static str {
        "windows-shell"
    }
}

#[cfg(target_os = "windows")]
fn extract_via_windows_shell(path: &Path, out: &Path, size: u32) -> Result<(), ExeThumbError> {
    let escaped_input = path.display().to_string().replace('\'', "''");
    let escaped_output = out.display().to_string().replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         Add-Type -AssemblyName System.Drawing; \
         $icon=[System.Drawing.Icon]::ExtractAssociatedIcon('{escaped_input}'); \
         if ($null -eq $icon) {{ exit 3 }}; \
         $bitmap=$icon.ToBitmap(); \
         $thumb=New-Object System.Drawing.Bitmap($bitmap, {size}, {size}); \
         $thumb.Save('{escaped_output}', [System.Drawing.Imaging.ImageFormat]::Png);"
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|source| ExeThumbError::Io {
            path: path.to_path_buf(),
            source,
        })?;

    if output.status.success() {
        return Ok(());
    }

    if output.status.code() == Some(3) {
        return Err(ExeThumbError::NoIconResource {
            path: path.to_path_buf(),
        });
    }

    Err(ExeThumbError::DecodeFailed {
        path: path.to_path_buf(),
        reason: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

#[cfg(not(target_os = "windows"))]
fn extract_via_windows_shell(_path: &Path, _out: &Path, _size: u32) -> Result<(), ExeThumbError> {
    Err(ExeThumbError::UnsupportedPlatform)
}
