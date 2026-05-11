# EXE thumbnail extraction

dethumb generates thumbnails for `.exe` files by extracting embedded icons from PE resources. The file is never executed — only parsed as data.

## Extraction chain

Three backends are tried in order. The first to produce a valid icon wins.

1. **Windows Shell** — uses `powershell` to call `System.Drawing.Icon.ExtractAssociatedIcon`. Only available on Windows.
2. **PE resource parser** — pure Rust, reads `RT_GROUP_ICON` / `RT_ICON` resources directly. Works everywhere. Uses `pelite` as primary parser, with a manual fallback for edge cases.
3. **Freedesktop icon fallback** — looks up generic executable icons (`application-x-ms-dos-executable`, etc.) from the system icon theme.

On non-Windows platforms, backend 1 returns `UnsupportedPlatform` immediately and the chain continues.

If a backend hits a non-retryable error (bad PE format, permission denied, I/O error), the chain stops rather than trying remaining backends.

## PE validation

Before extraction, the PE header is validated:
- MZ signature at offset 0
- PE signature at the offset specified in the DOS header
- At least one section
- Optional header size ≥ 2 bytes
- All offsets checked against file length

Files larger than 512 MiB are rejected before parsing.

## Cache

Each thumbnail gets a `.cachekey` sidecar file. The key is a blake3 hash of:
- Canonical path
- File size and mtime
- Requested thumbnail size
- Backend chain version marker

On cache hit the output is skipped. The sidecar is written after a successful extraction.

## PNG scanning fallback

If `RT_GROUP_ICON` resources are missing, the PE parser scans the binary for embedded PNG signatures and decodes the best-matching one. This catches executables where icons are stored as raw PNGs in the resource section.

## ICO reconstruction

When icon groups are found, individual `RT_ICON` entries are assembled into a valid ICO file in memory, then decoded with the `image` crate. The best frame is chosen by size proximity to the requested thumbnail dimensions.

## Telemetry

The `src/exe/telemetry.rs` module tracks cache hits/misses, extraction attempts, and fallback reasons. This is in-process only — no network or file logging. Useful for debugging backend selection with `--debug`.
