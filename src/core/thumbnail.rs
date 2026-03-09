use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use freedesktop_icons::lookup;
use image::{DynamicImage, GenericImageView, ImageFormat, RgbaImage};
use resvg::usvg::{self, Tree};
use tiny_skia::{IntSize, Pixmap, Transform};

/// Render an SVG icon to a PNG thumbnail file.
pub fn process_svg(path: &Path, out: &str, size: u32) -> Result<(), String> {
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

/// Read and resize raster icon data into a centered PNG thumbnail.
pub fn process_png(p: &Path, size: u32, out_png: &str) -> Result<(), String> {
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
    let small = img.resize(nw, nh, image::imageops::FilterType::Triangle);
    let x = (size - nw) / 2;
    let y = (size - nh) / 2;
    image::imageops::overlay(&mut out, &small, x.into(), y.into());
    out
}

/// Create a fallback thumbnail when icon-specific processing fails.
pub fn create_fallback_thumbnail(out: &str, size: u32) {
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
