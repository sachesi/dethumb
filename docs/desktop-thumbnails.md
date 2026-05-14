# Desktop entry thumbnail generation

dethumb reads `Icon=` from `.desktop` files and produces a PNG thumbnail
at the requested size.

## Lookup chain

1. Absolute path with supported extension — use directly. On Unix the
   file must be world-readable (mode `0o444`) and must not be a symlink.
2. Look up the icon name in the current GTK icon theme (from
   `gsettings get org.gnome.desktop.interface icon-theme`, falls back to
   `hicolor`).
3. Try fallback themes: Adwaita, then Papirus.

Candidate names are expanded with case variants and file-stem forms to
catch common naming patterns (e.g. `MyIcon.png` — tries `MyIcon.png`,
`myicon.png`, `MyIcon`, `myicon`).

## Rendering

- **SVG** — parsed with `resvg/usvg`, rasterised with `tiny-skia`.
  Scaled to fit the requested size while preserving aspect ratio,
  centred on a square canvas.
- **Raster (PNG/JPEG)** — decoded with the `image` crate, resized with a
  triangle filter, centred on a square canvas.

## Output path safety

Paths containing `..` components are rejected. The thumbnailer protocol
derives output filenames from the input URI, so this prevents writing
outside the thumbnail cache.

## Fallback

On failure a generic icon (`application-x-generic` from Adwaita) is
rendered. Non-thumbnailable errors (e.g. unsupported PE extension) exit
cleanly without a fallback so the file manager can try another
thumbnailer.
