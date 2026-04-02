# desktop-thumbnailer

A small Rust utility that generates thumbnails for Linux `.desktop` files.

Quick start
- Build: `cargo build --release`
- Run: execute the produced `desktop-thumbnailer` binary (it generates PNG thumbnails for `.desktop` files and is intended for integration with a desktop thumbnailer service).

Notes
- Minimal, focused on producing clear thumbnails for application `.desktop` entries.
- See `LICENSE` for license details (GPLv3).

Release hygiene
- Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `cargo audit` before every release.
- Regularly update dependencies (`cargo update`) and re-run `cargo audit` to keep security advisories current.
