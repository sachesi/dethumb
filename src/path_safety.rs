use std::path::{Component, Path};

/// Reject output paths that contain `..` traversal components.
pub fn has_parent_dir_component(path: &str) -> bool {
    Path::new(path)
        .components()
        .any(|component| component == Component::ParentDir)
}

#[cfg(test)]
mod tests {
    use super::has_parent_dir_component;

    #[test]
    fn detects_parent_dir_in_output_path() {
        assert!(has_parent_dir_component("../thumb.png"));
        assert!(has_parent_dir_component("/tmp/../thumb.png"));
        assert!(!has_parent_dir_component("/tmp/thumb.png"));
        assert!(!has_parent_dir_component("thumb.png"));
    }
}
