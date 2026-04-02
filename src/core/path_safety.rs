use std::path::{Component, Path};

/// Reject output paths that contain `..` traversal components.
#[must_use]
pub fn has_parent_dir_component(path: &Path) -> bool {
    path.components()
        .any(|component| component == Component::ParentDir)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::has_parent_dir_component;

    #[test]
    fn detects_parent_dir_in_output_path() {
        assert!(has_parent_dir_component(Path::new("../thumb.png")));
        assert!(has_parent_dir_component(Path::new("/tmp/../thumb.png")));
        assert!(!has_parent_dir_component(Path::new("/tmp/thumb.png")));
        assert!(!has_parent_dir_component(Path::new("thumb.png")));
    }
}
