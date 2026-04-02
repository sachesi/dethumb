use std::fs;
use std::path::Path;

use desktop_thumbnailer::{AppError, CliArgs, run_with_args};
use image::{ImageBuffer, Rgba};
use tempfile::TempDir;

fn write_png(path: &Path) {
    let image = ImageBuffer::from_pixel(8, 8, Rgba([255_u8, 0, 0, 255]));
    assert!(image.save(path).is_ok());
}

#[test]
fn rejects_parent_traversal_in_output_argument() {
    let args = vec![
        "desktop-thumbnailer".to_string(),
        "app.desktop".to_string(),
        "../thumb.png".to_string(),
        "256".to_string(),
    ];

    let parsed = CliArgs::parse_from_slice(&args);
    assert!(matches!(parsed, Err(AppError::UnsafeOutputPath(_))));
}

#[test]
fn generates_thumbnail_from_absolute_raster_icon() {
    let tmp = TempDir::new();
    assert!(tmp.is_ok());
    let Ok(tmp) = tmp else {
        panic!("tempdir should be created");
    };

    let icon_path = tmp.path().join("icon.png");
    let desktop_path = tmp.path().join("app.desktop");
    let out_path = tmp.path().join("thumb.png");

    write_png(&icon_path);

    let desktop_contents = format!(
        "[Desktop Entry]\nType=Application\nName=Demo\nIcon={}\n",
        icon_path.display()
    );
    assert!(fs::write(&desktop_path, desktop_contents).is_ok());

    let args = CliArgs::new(desktop_path, out_path.clone(), 64);
    let result = run_with_args(&args);

    assert!(result.is_ok(), "expected successful thumbnail generation");
    assert!(out_path.is_file(), "expected output thumbnail file");
}
