# `.exe` Thumbnail Generation: Implementation Plan

## Current Implementation Status (April 2, 2026)

- [x] Added file-kind detection for `.desktop` and `.exe` inputs.
- [x] Wired `.exe` branch into the shared orchestrator path in `src/lib.rs`.
- [x] Added a dedicated `.exe` error model with typed variants and conversions.
- [x] Introduced a modular extractor trait (`ExeIconExtractor`) and a concrete fallback backend implementation.
- [x] Added cache-key infrastructure for executable thumbnails using a sidecar key file.
- [x] Added unit/integration tests for dispatch and cache behavior.
- [x] Renamed `.desktop` pipeline modules from `src/core` to `src/desktop` for clearer separation from `.exe` logic.
- [x] Added executable-size resource-limit regression test coverage.
- [x] Hardened PE header validation with machine/section/optional-header bounds checks.
- [x] Implement native Windows shell/resource extraction backend.
- [x] Implement pure PE resource parser fallback backend (read-only parsing of `RT_GROUP_ICON`/`RT_ICON`).
- [x] Add backend observability counters and fallback reason telemetry.
- [x] Add malformed PE fuzzing and resource-threshold hardening tests (added deterministic malformed-corpus coverage for PE header validation and ICO blob scanning).

## 1) Platform Scope and High-Level Behavior

1. **Define supported behavior by OS at compile time**
   - **Windows**: fully support `.exe` thumbnail extraction from embedded icon resources.
   - **Linux/macOS**: parse `.exe` as PE files without executing them, extract icon resources in-process (portable path) if dependencies are available.
   - If extraction backend is unavailable on non-Windows platforms, return a typed `UnsupportedOnPlatform` error and continue existing `.desktop` flow.
2. **Preserve existing `.desktop` pipeline** and add `.exe` as a new file-kind branch through a common thumbnail orchestration API.
3. **Never execute `.exe` files** as part of thumbnail generation.

## 2) Architecture and Module Layout

Create a modular extractor pipeline with clear trait boundaries:

- `src/exe/mod.rs`
  - Public entrypoint for `.exe` thumbnail generation.
- `src/exe/detector.rs`
  - File-kind detection (`.desktop`, `.exe`, unsupported).
- `src/exe/extractor.rs`
  - `trait ExeIconExtractor { fn extract_best_icon(...) -> Result<RawIcon, ExeThumbError>; }`
  - Chooses backend based on OS/features.
- `src/exe/backends/windows_shell.rs`
  - Windows-specific extraction using Win32/Shell APIs.
- `src/exe/backends/pe_resource.rs`
  - Cross-platform PE resource parser fallback (read-only parsing).
- `src/exe/convert.rs`
  - Convert icon payloads (ICO/BMP/PNG chunks) into normalized RGBA buffers.
- `src/exe/render.rs`
  - Resize + encode PNG thumbnail output.
- `src/exe/cache.rs`
  - Cache key generation, lookup, and invalidation logic.
- `src/exe/error.rs`
  - Typed error enum and mapping to user-visible statuses.

Keep existing `src/desktop/thumbnail.rs` as the orchestrator and delegate `.exe` logic to `src/exe`.

## 3) Preferred Extraction Path for `.exe` Icons

### Primary path (Windows)
1. Use Windows API to query icon resources from the target executable:
   - `SHGetFileInfoW` / `IShellItemImageFactory` for shell-consistent icon retrieval (good UX parity with Explorer).
   - If needed for higher fidelity: `PrivateExtractIconsW` or resource APIs (`LoadLibraryExW` with `LOAD_LIBRARY_AS_DATAFILE`, `FindResource`, `LoadResource`).
2. Extract multiple icon sizes, pick the best candidate for requested thumbnail size.

### Secondary path (all platforms, including Windows fallback)
1. Parse PE resources directly using a pure Rust parser.
2. Locate `RT_GROUP_ICON` and matching `RT_ICON` entries.
3. Reconstruct ICO payload in memory and decode selected frame.

## 4) Fallback Strategy

1. **Tier 1**: OS-native extraction (Windows shell/resource APIs).
2. **Tier 2**: Pure Rust PE resource parsing backend.
3. **Tier 3**: Generic file icon placeholder (current default icon mechanism).
4. **Tier 4**: Return `NoThumbnailAvailable` gracefully without failing whole request.

All fallback transitions should be logged at debug/info level with reason codes.

## 5) Recommended Crates / APIs

- Windows interop:
  - `windows` crate for Win32/Shell bindings.
- PE/resource parsing:
  - `pelite` (or equivalent PE parser) for safe read-only resource traversal.
- Image decode/transform/encode:
  - `image` crate for PNG output and resizing.
  - `ico` crate for ICO parsing when reconstructing icon groups.
- Errors:
  - `thiserror` for typed error definitions.
- Caching/hash:
  - `blake3` (or `sha2`) for stable cache keys.

Prefer feature-gating:
- `cfg(target_os = "windows")` for Win32 backend.
- Cargo feature `exe-pe-fallback` for optional PE parser backend.

## 6) Icon Data -> PNG Thumbnail Conversion Pipeline

1. Extract icon payload + metadata (available sizes, bit depth, alpha presence).
2. Decode to RGBA8.
3. Select best source frame:
   - Choose nearest size >= requested size; otherwise largest available.
4. Resize with high-quality filter (`Lanczos3` or `CatmullRom`) to target dimensions.
5. Encode PNG with deterministic settings (stable output for cache hits).
6. Return bytes and dimensions to core thumbnail writer.

## 7) Error Handling Model

Define `ExeThumbError` with structured variants:
- `UnsupportedPlatform`
- `InvalidPeFormat`
- `NoIconResource`
- `DecodeFailed`
- `Io`
- `PermissionDenied`
- `ResourceLimitExceeded`

Guidelines:
1. Convert backend-specific errors into stable internal variants.
2. Distinguish retryable (`Io`, transient read) vs non-retryable (`InvalidPeFormat`).
3. Avoid noisy hard failures; degrade to fallback icon.

## 8) Caching Strategy

Cache key should include:
- Canonical path (or inode/file ID abstraction).
- File metadata: size + mtime (and optionally quick hash of PE header/resources).
- Requested thumbnail size.
- Backend/version marker (to invalidate across algorithm changes).

Behavior:
1. Read-through cache before extraction.
2. Write-through on successful PNG generation.
3. Use bounded cache size and LRU eviction.
4. Keep `.desktop` and `.exe` namespaces separate to avoid collisions.

## 9) Security Considerations for Untrusted `.exe`

1. **Never load for execution**; only map/read as data.
2. Apply strict resource limits:
   - Max file size for parsing.
   - Max icon dimensions / decoded pixel count.
   - Max allocation thresholds.
3. Validate lengths/offsets when reading PE structures.
4. Use timeout/abort guards for pathological inputs.
5. Treat parser failures as untrusted-data errors, not panics.
6. Sanitize logging (avoid dumping raw binary blobs).
7. Consider running extraction in a worker process/sandbox for defense-in-depth.

## 10) Testability and Milestones

### Milestone 1: Core abstraction and detection
- Add file-kind detector and extractor trait.
- Wire `.exe` branch into existing orchestrator behind feature flag.
- Unit tests for detection and dispatch.

### Milestone 2: PE fallback backend + conversion
- Implement PE resource parsing and ICO reconstruction.
- Add conversion pipeline to PNG bytes.
- Golden tests with fixture executables (small, curated test samples).

### Milestone 3: Windows-native backend
- Implement Win32 extraction backend and fallback ordering.
- Integration tests on Windows CI runner.

### Milestone 4: Caching + hardening
- Add cache keys/versioning and eviction behavior.
- Fuzz/robustness tests for malformed PE/icon inputs.
- Add observability counters for fallback reasons and cache hit rate.

## 11) Key Risks and Mitigations

1. **Risk**: Inconsistent icon selection across backends.
   - **Mitigation**: Centralized “best frame” selector and deterministic scoring rules.
2. **Risk**: Malformed PE files causing crashes or OOM.
   - **Mitigation**: Bounds checks, decode limits, fuzzing, and allocation caps.
3. **Risk**: Windows API complexity and handle leaks.
   - **Mitigation**: RAII wrappers and explicit cleanup tests.
4. **Risk**: Cache staleness after file updates.
   - **Mitigation**: include metadata + backend version in cache key.
5. **Risk**: Performance regressions for large batches.
   - **Mitigation**: benchmark extraction path, add concurrency limits, and cache aggressively.
