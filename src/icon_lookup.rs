use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use freedesktop_icons::lookup;

/// Read the current desktop icon theme via `gsettings`.
pub fn get_current_theme() -> Option<String> {
    Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "icon-theme"])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .trim_matches('\'')
                .to_string()
                .into()
        })
}

/// Resolve an icon to a concrete file path, handling absolute and themed icons.
pub fn find_icon_path(icon: &str, theme: &str, size: u32) -> Option<PathBuf> {
    println!("Searching for icon: {}", icon);
    if icon.starts_with('/') {
        let p = Path::new(icon);
        println!("Checking absolute path: {:?}", p);

        // Validate the provided absolute path before using it.
        match fs::symlink_metadata(p) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    println!("Refusing symlink icon path: {:?}", p);
                } else if metadata.is_file() {
                    #[cfg(unix)]
                    {
                        let mode = metadata.permissions().mode();
                        if (mode & 0o444) == 0 {
                            println!("Insufficient permissions for: {:?}", p);
                        } else if has_supported_icon_extension(p) {
                            println!("Found valid icon file: {:?}", p);
                            return Some(p.to_path_buf());
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        if has_supported_icon_extension(p) {
                            println!("Found valid icon file: {:?}", p);
                            return Some(p.to_path_buf());
                        }
                    }
                } else {
                    println!("Path is not a file: {:?}", p);
                }
            }
            Err(e) => {
                println!("Failed to access path {:?}: {}", p, e);
                if e.kind() == std::io::ErrorKind::NotFound {
                    println!("File not found: {:?}", p);
                } else {
                    println!("Error accessing file: {:?}", e);
                }
            }
        }
    }

    // Fall back to icon theme lookup when absolute-path lookup is unavailable.
    let candidates = build_icon_candidates(icon);
    for name in &candidates {
        if let Some(p) = lookup(name)
            .with_size(size.try_into().unwrap_or(256))
            .with_theme(theme)
            .find()
        {
            println!("Found icon in theme {}: {:?}", theme, p);
            return Some(p);
        }
    }
    for fb in &["Adwaita", "Papirus"] {
        for name in &candidates {
            if let Some(p) = lookup(name)
                .with_size(size.try_into().unwrap_or(256))
                .with_theme(fb)
                .find()
            {
                println!("Found icon in fallback theme {}: {:?}", fb, p);
                return Some(p);
            }
        }
    }
    println!("No icon found after searching themes");
    None
}

/// Build ordered, deduplicated icon lookup candidates.
pub fn build_icon_candidates(icon: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut push_unique = |candidate: String| {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    };

    push_unique(icon.to_string());
    push_unique(icon.to_lowercase());

    if let Some(stem) = Path::new(icon).file_stem().and_then(|s| s.to_str()) {
        push_unique(stem.to_string());
        push_unique(stem.to_lowercase());
    }

    candidates
}

fn has_supported_icon_extension(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        let ext = ext.to_lowercase();
        let supported = ["png", "jpg", "jpeg", "svg"].contains(&ext.as_str());
        if !supported {
            println!("Unsupported extension: {}", ext);
        }
        supported
    } else {
        println!("No extension found for: {:?}", path);
        false
    }
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
        assert_eq!(candidates, vec!["MyIcon.png", "myicon.png", "MyIcon", "myicon"]);
    }
}
