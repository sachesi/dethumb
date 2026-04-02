# dethumb

A small Rust utility that generates thumbnails for Linux `.desktop` files and Windows `.exe` binaries.

Quick start
- Build: `cargo build --release`
- Run: execute the produced `dethumb` binary (it generates PNG thumbnails for `.desktop` and `.exe` inputs and is intended for integration with a desktop thumbnailer service).

Notes
- Minimal, focused on producing clear thumbnails for application launchers and executable files.
- See `LICENSE` for license details (GPLv3).

Release hygiene
- Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `cargo audit` before every release.
- Regularly update dependencies (`cargo update`) and re-run `cargo audit` to keep security advisories current.
