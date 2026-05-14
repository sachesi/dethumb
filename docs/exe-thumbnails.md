# EXE thumbnail extraction

dethumb generates thumbnails for `.exe` files by extracting embedded
icons from PE resources. The file is never executed — only parsed as
data.

## Extraction chain

Three backends are tried in order. The first to produce a valid icon
wins.

1. **Windows Shell** — calls `System.Drawing.Icon.ExtractAssociatedIcon`
   via PowerShell. Windows only.
2. **PE resource parser** — pure Rust, reads `RT_GROUP_ICON` /
   `RT_ICON` resources directly via `pelite`. Works everywhere.
3. **Freedesktop icon fallback** — looks up generic executable icons
   (`application-x-ms-dos-executable`, etc.) from the system icon theme.

Backend 1 returns `UnsupportedPlatform` on non-Windows platforms and the
chain continues. Non-retryable errors (bad PE, permission denied, I/O)
stop the chain immediately.

## PE validation

Before any extraction the PE header is checked:
- MZ signature at offset 0
- PE signature at the offset in the DOS header
- At least one section
- Optional header size >= 2 bytes
- All offsets validated against file length

Files larger than 512 MiB are rejected before parsing.

## Cache

Each thumbnail gets a `.cachekey` sidecar file keyed on a blake3 hash
of:
- Canonical path
- File size and mtime
- Requested size
- Backend chain version marker

On cache hit the output is skipped. The sidecar is written after a
successful extraction.

## PNG scanning

If `RT_GROUP_ICON` resources are missing, the parser scans the binary
for embedded PNG signatures and decodes the best-matching one. This
catches executables where icons are stored as raw PNGs.

## ICO reconstruction

`RT_ICON` entries are assembled into a valid ICO file in memory and
decoded with the `image` crate. The best frame is chosen by size
proximity to the requested thumbnail dimensions.

## Telemetry

The `src/exe/telemetry.rs` module tracks cache hits, extraction
attempts, and fallback reasons. In-process only — no network or file
logging. Useful with `--debug`.
