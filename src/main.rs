use std::env;
use std::fs;

use freedesktop_entry_parser::parse_entry;

mod core {
    pub mod icon_lookup;
    pub mod path_safety;
    pub mod thumbnail;
}

use core::icon_lookup::{find_icon_path, get_current_theme};
use core::path_safety::has_parent_dir_component;
use core::thumbnail::{create_fallback_thumbnail, process_png, process_svg};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: {} <.desktop> <out.png> <size>", args[0]);
        std::process::exit(1);
    }
    let desktop = &args[1];
    let out_png = &args[2];
    if has_parent_dir_component(out_png) {
        eprintln!(
            "Refusing unsafe output path with parent traversal: {}",
            out_png
        );
        std::process::exit(1);
    }
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
