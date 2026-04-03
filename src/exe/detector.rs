use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    DesktopEntry,
    Executable,
    Unsupported,
}

const PE_CANDIDATE_EXTENSIONS: &[&str] = &["exe", "dll", "ocx", "cpl", "drv", "mui"];

#[must_use]
pub fn detect_input_kind(path: &Path) -> InputKind {
    match path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("desktop") => InputKind::DesktopEntry,
        Some(ext) if PE_CANDIDATE_EXTENSIONS.contains(&ext) => InputKind::Executable,
        _ => InputKind::Unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::{InputKind, detect_input_kind};
    use std::path::Path;

    #[test]
    fn detects_supported_input_types_case_insensitively() {
        assert_eq!(
            detect_input_kind(Path::new("app.desktop")),
            InputKind::DesktopEntry
        );
        assert_eq!(
            detect_input_kind(Path::new("APP.EXE")),
            InputKind::Executable
        );
        assert_eq!(
            detect_input_kind(Path::new("library.DLL")),
            InputKind::Executable
        );
        assert_eq!(
            detect_input_kind(Path::new("README.md")),
            InputKind::Unsupported
        );
    }
}
