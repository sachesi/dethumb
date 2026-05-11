# dethumb

Generates PNG thumbnails for Linux `.desktop` files and Windows `.exe` binaries. Integrates with file managers via the freedesktop thumbnailer protocol (see `packaging/` for the `.thumbnailer` entry).

## Build and usage

```
cargo build --release
```

The `dethumb` binary expects four arguments:

```
dethumb <input> <output.png> <size>
```

An optional `--debug` flag enables verbose backend selection and cache diagnostics.

Inputs are auto-detected by extension: `.desktop` files are rendered from their `Icon=` entry using the current desktop icon theme; `.exe` files have icons extracted from PE resources (see `docs/` for extraction details).

## License

GPL-3.0-or-later — see `LICENSE`.

## Release checks

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
cargo audit
```

Run these before every release. Keep dependencies current (`cargo update`) and re-run `cargo audit` to catch new advisories.
