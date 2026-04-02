use std::collections::HashSet;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use freedesktop_icons::lookup;

use crate::desktop::thumbnail::detect_icon_format;

const LOOKUP_FALLBACK_SIZE: u16 = 256;
const FALLBACK_THEMES: [&str; 2] = ["Adwaita", "Papirus"];

/// Read the current desktop icon theme via `gsettings`.
#[must_use]
pub fn get_current_theme() -> Option<String> {
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "icon-theme"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let theme = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_matches('\'')
        .trim()
        .to_owned();

    if theme.is_empty() { None } else { Some(theme) }
}

/// Resolve an icon to a concrete file path, handling absolute and themed icons.
#[must_use]
pub fn find_icon_path(icon: &str, theme: &str, size: u32) -> Option<PathBuf> {
    let icon = icon.trim();
    if icon.is_empty() || icon.contains('\0') {
        return None;
    }

    if let Some(path) = validate_absolute_icon(icon) {
        return Some(path);
    }

    let lookup_size = u16::try_from(size).unwrap_or(LOOKUP_FALLBACK_SIZE);
    let candidates = build_icon_candidates(icon);

    for name in &candidates {
        if let Some(path) = lookup(name).with_size(lookup_size).with_theme(theme).find() {
            return Some(path);
        }
    }

    for fallback_theme in FALLBACK_THEMES {
        for name in &candidates {
            if let Some(path) = lookup(name)
                .with_size(lookup_size)
                .with_theme(fallback_theme)
                .find()
            {
                return Some(path);
            }
        }
    }

    None
}

fn validate_absolute_icon(icon: &str) -> Option<PathBuf> {
    let path = Path::new(icon);
    if !path.is_absolute() {
        return None;
    }

    let metadata = fs::symlink_metadata(path).ok()?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return None;
    }

    #[cfg(unix)]
    {
        let mode = metadata.permissions().mode();
        if (mode & 0o444) == 0 {
            return None;
        }
    }

    if matches!(
        detect_icon_format(path),
        crate::desktop::thumbnail::IconFormat::Unsupported
    ) {
        return None;
    }

    Some(path.to_path_buf())
}

/// Build ordered, deduplicated icon lookup candidates.
#[must_use]
pub fn build_icon_candidates(icon: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();

    let mut push_candidate = |value: String| {
        if seen.insert(value.clone()) {
            candidates.push(value);
        }
    };

    push_candidate(icon.to_owned());
    push_candidate(icon.to_ascii_lowercase());

    if let Some(stem) = Path::new(icon).file_stem().and_then(|value| value.to_str()) {
        push_candidate(stem.to_owned());
        push_candidate(stem.to_ascii_lowercase());
    }

    candidates
}

#[cfg(test)]
mod tests {
    use super::build_icon_candidates;

    #[test]
    fn deduplicates_case_variants() {
        let candidates = build_icon_candidates("ICON");
        assert_eq!(candidates, vec!["ICON", "icon"]);
    }

    #[test]
    fn includes_stem_variants_once() {
        let candidates = build_icon_candidates("MyIcon.png");
        assert_eq!(
            candidates,
            vec!["MyIcon.png", "myicon.png", "MyIcon", "myicon"]
        );
    }
}
