# dethumb

Generates PNG thumbnails for Linux `.desktop` files and Windows `.exe`
binaries. Integrates with file managers via the freedesktop thumbnailer
protocol.

## Usage

```
dethumb <input> <output.png> <size> [--debug]
```

Input type is detected by extension:
- `.desktop` — renders the icon from the `Icon=` entry using the system
  icon theme
- `.exe` — extracts the embedded icon from PE resources

## Build

```
cargo build --release
```

## License

GPL-3.0-or-later — see `LICENSE`.

## Release checks

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
cargo audit
```
