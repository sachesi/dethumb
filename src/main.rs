use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use freedesktop_entry_parser::parse_entry;
use freedesktop_icons::lookup;
use image::{DynamicImage, GenericImageView, ImageFormat, RgbaImage};
use png;
use resvg::usvg::{self, Tree};
use tiny_skia::{IntSize, Pixmap, Transform};

fn main() {
    // Debug: Print current user
    println!(
        "Running as user: {:?}",
        env::var("USER").unwrap_or("unknown".to_string())
    );

    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: {} <.desktop> <out.png> <size>", args[0]);
        std::process::exit(1);
    }
    let desktop = &args[1];
    let out_png = &args[2];
    let size: u32 = args[3].parse().unwrap_or_else(|e| {
        eprintln!("Bad size '{}': {}", args[3], e);
        std::process::exit(1);
    });
    let size = if size == 0 { 256 } else { size };

    let path = fs::canonicalize(desktop).unwrap_or_else(|e| {
        eprintln!("Canon failed '{}': {}", desktop, e);
        create_fallback_thumbnail(out_png, size);
        std::process::exit(1);
    });
    let entry = parse_entry(&path).unwrap_or_else(|e| {
        eprintln!("Parse .desktop failed: {}", e);
        create_fallback_thumbnail(out_png, size);
        std::process::exit(1);
    });
    let icon = entry
        .section("Desktop Entry")
        .attr("Icon")
        .unwrap_or_else(|| {
            eprintln!("No Icon= in .desktop");
            create_fallback_thumbnail(out_png, size);
            std::process::exit(1);
        });
    println!("Icon value from .desktop: {}", icon);

    let theme = get_current_theme().unwrap_or_else(|| "hicolor".to_string());
    let icon_path = find_icon_path(&icon, &theme, size);

    match icon_path {
        Some(p) => {
            println!("Processing icon path: {:?}", p);
            match p
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase())
                .as_deref()
            {
                Some("svg") => {
                    if let Err(e) = process_svg(&p, out_png, size) {
                        eprintln!("SVG render err: {}", e);
                        create_fallback_thumbnail(out_png, size);
                    }
                }
                Some("png") | Some("jpg") | Some("jpeg") => {
                    if let Err(e) = process_png(&p, size, out_png) {
                        eprintln!("PNG/JPG process err: {}", e);
                        create_fallback_thumbnail(out_png, size);
                    }
                }
                _ => {
                    eprintln!("Unsupported extension on {}", p.display());
                    create_fallback_thumbnail(out_png, size);
                }
            }
        }
        None => {
            eprintln!("No valid icon path found for: {}", icon);
            create_fallback_thumbnail(out_png, size);
        }
    }
}

fn get_current_theme() -> Option<String> {
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

fn find_icon_path(icon: &str, theme: &str, size: u32) -> Option<PathBuf> {
    println!("Searching for icon: {}", icon);
    if icon.starts_with('/') {
        let p = Path::new(icon);
        println!("Checking absolute path: {:?}", p);

        // Directly check metadata without canonicalization
        match fs::metadata(p) {
            Ok(metadata) => {
                if metadata.is_file() {
                    // Check read permissions on Unix
                    #[cfg(unix)]
                    {
                        let mode = metadata.permissions().mode();
                        if (mode & 0o444) == 0 {
                            println!("Insufficient permissions for: {:?}", p);
                        } else {
                            // Validate file extension
                            if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                                let ext = ext.to_lowercase();
                                if ["png", "jpg", "jpeg", "svg"].contains(&ext.as_str()) {
                                    println!("Found valid icon file: {:?}", p);
                                    return Some(p.to_path_buf());
                                } else {
                                    println!("Unsupported extension: {}", ext);
                                }
                            } else {
                                println!("No extension found for: {:?}", p);
                            }
                        }
                    }
                    // Non-Unix systems skip permission checks
                    #[cfg(not(unix))]
                    {
                        if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                            let ext = ext.to_lowercase();
                            if ["png", "jpg", "jpeg", "svg"].contains(&ext.as_str()) {
                                println!("Found valid icon file: {:?}", p);
                                return Some(p.to_path_buf());
                            } else {
                                println!("Unsupported extension: {}", ext);
                            }
                        } else {
                            println!("No extension found for: {:?}", p);
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

    // Proceed to look up icon in themes if absolute path check fails
    let mut candidates = Vec::new();
    candidates.push(icon.to_string());
    candidates.push(icon.to_lowercase());
    if let Some(stem) = Path::new(icon).file_stem().and_then(|s| s.to_str()) {
        candidates.push(stem.to_string());
        candidates.push(stem.to_lowercase());
    }
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

fn process_svg(path: &Path, out: &str, size: u32) -> Result<(), String> {
    let data = fs::read(path).map_err(|e| format!("Failed to read SVG: {}", e))?;
    let opt = usvg::Options::default();
    let tree = Tree::from_data(&data, &opt).map_err(|e| format!("Failed to parse SVG: {}", e))?;
    let sz = IntSize::from_wh(size, size).ok_or("Invalid size")?;
    let mut pix = Pixmap::new(sz.width(), sz.height()).ok_or("Failed to create pixmap")?;
    let svg_sz = tree.size();
    let scale = size as f32 / svg_sz.width().min(svg_sz.height());
    let tx = Transform::from_scale(scale, scale);
    resvg::render(&tree, tx, &mut pix.as_mut());
    let png = pix
        .encode_png()
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;
    if let Some(dir) = Path::new(out).parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    File::create(out)
        .and_then(|mut f| f.write_all(&png))
        .map_err(|e| format!("Failed to write output: {}", e))
}

fn process_png(p: &Path, size: u32, out_png: &str) -> Result<(), String> {
    let img = match image::open(p) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("image::open failed for {}: {}", p.display(), e);
            let f = File::open(p).map_err(|e| format!("File::open failed: {}", e))?;
            let mut reader = png::Decoder::new(f)
                .read_info()
                .map_err(|e| format!("png::Decoder::read_info failed: {}", e))?;
            let info = reader.info().clone();
            let mut buf = vec![0; reader.output_buffer_size()];
            reader
                .next_frame(&mut buf)
                .map_err(|e| format!("png::Decoder::next_frame failed: {}", e))?;
            match info.color_type {
                png::ColorType::Rgba => {
                    let rgba = RgbaImage::from_raw(info.width, info.height, buf)
                        .ok_or("Buffer size mismatch")?;
                    DynamicImage::ImageRgba8(rgba)
                }
                png::ColorType::Rgb => {
                    let mut rgba = RgbaImage::new(info.width, info.height);
                    for (i, chunk) in buf.chunks(3).enumerate() {
                        let x = (i as u32) % info.width;
                        let y = (i as u32) / info.width;
                        rgba.put_pixel(x, y, image::Rgba([chunk[0], chunk[1], chunk[2], 255]));
                    }
                    DynamicImage::ImageRgba8(rgba)
                }
                other => {
                    return Err(format!("Unsupported PNG color type: {:?}", other));
                }
            }
        }
    };
    let thumb = resize_image(img, size);
    if let Some(dir) = Path::new(out_png).parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    let mut fout =
        File::create(out_png).map_err(|e| format!("Failed to create output file: {}", e))?;
    thumb
        .write_to(&mut fout, ImageFormat::Png)
        .map_err(|e| format!("Failed to write PNG: {}", e))?;
    Ok(())
}

fn resize_image(img: DynamicImage, size: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    let ratio = size as f32 / w.max(h) as f32;
    let nw = (w as f32 * ratio).round() as u32;
    let nh = (h as f32 * ratio).round() as u32;
    let mut out = DynamicImage::new_rgba8(size, size);
    let small = img.resize(nw, nh, image::imageops::FilterType::Lanczos3);
    let x = (size - nw) / 2;
    let y = (size - nh) / 2;
    image::imageops::overlay(&mut out, &small, x.into(), y.into());
    out
}

fn create_fallback_thumbnail(out: &str, size: u32) {
    let def = "application-x-generic";
    let theme = "Adwaita";
    match lookup(def)
        .with_size(size.try_into().unwrap_or(256))
        .with_theme(theme)
        .find()
    {
        Some(p) => {
            println!("Creating fallback thumbnail from: {:?}", p);
            if p.extension().and_then(|s| s.to_str()) == Some("svg") {
                match process_svg(&p, out, size) {
                    Ok(_) => println!("Fallback SVG thumbnail created successfully"),
                    Err(e) => eprintln!("Failed to create SVG fallback thumbnail: {}", e),
                }
            } else {
                match image::open(&p) {
                    Ok(img) => {
                        let thumb = resize_image(img, size);
                        if let Some(dir) = Path::new(out).parent() {
                            if let Err(e) = fs::create_dir_all(dir) {
                                eprintln!("Failed to create directory for fallback: {}", e);
                                return;
                            }
                        }
                        match File::create(out) {
                            Ok(mut f) => {
                                if let Err(e) = thumb.write_to(&mut f, ImageFormat::Png) {
                                    eprintln!("Failed to write fallback thumbnail: {}", e);
                                } else {
                                    println!("Fallback thumbnail created successfully");
                                }
                            }
                            Err(e) => eprintln!("Failed to create fallback file: {}", e),
                        }
                    }
                    Err(e) => eprintln!("Failed to open fallback image: {}", e),
                }
            }
        }
        None => {
            eprintln!("Failed to find fallback icon: {}", def);
        }
    }
}
