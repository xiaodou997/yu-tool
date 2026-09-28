# CLI Specification

> Status: **M2 frozen baseline + M3 read-only PSD CLI / v0.1 evolving**

The public executable is:

```bash
yu
```

This document defines the public command model through the M2 frozen baseline and PR #22 read-only PSD runtime. Commands explicitly marked **planned** belong to later milestones.

## Design rules

1. Commands describe capabilities, not backend syntax.
2. Human-readable output is the default.
3. Automation-oriented commands should support `--json`.
4. Optional engine selection is explicit through `--engine` when needed.
5. Mutating operations should not overwrite source files by default.
6. Public command names and structured fields should remain stable once released.

## Global commands

### `yu doctor`

Diagnose YuTool and the unified engine inventory.

```bash
yu doctor
yu doctor --json
```

Doctor counts Built-in, Managed, and discovered System engines from the same inventory used by `yu engine list`.

A healthy but inactive Managed engine is reported as `disabled` and does not make Doctor degraded. Missing optional System tools are not included at all. Broken/incompatible discovered engines contribute warnings and degrade health.

### `yu capabilities`

List currently usable capabilities.

```bash
yu capabilities
yu capabilities --json
```

This reports **effective capabilities on the current machine**, not merely features known to the source code.

PSD capabilities are added only when the activated Managed ag-psd version has a valid entrypoint and declares them. This is a metadata-based readiness check, not a parsing health probe. Inactive versions' capability declarations cannot make an operation executable. `yu doctor` uses this same effective capability count; Engine Inventory retains its M2 union-of-installed-versions metadata semantics.

### `yu engine list`

List the unified engine inventory across Built-in, Managed, and discovered System providers.

```bash
yu engine list
yu engine list --json
```

The JSON result preserves the existing descriptor fields:

- `id`
- `display_name`
- `provider`
- `state`
- `version`
- `capabilities`

and may add optional discovery fields such as `executable`, `installed_versions`, `active_version`, and `warnings`.

System engines are listed only when their executable is actually discovered on `PATH`.

### `yu engine info`

Inspect every discovered provider for one logical engine ID.

```bash
yu engine info imagemagick
yu engine info imagemagick --json
```

If both a YuTool-managed ImageMagick and a system ImageMagick exist, both provider records are returned rather than silently replacing one with the other.

States include:

- `ready`
- `not_installed`
- `broken`
- `incompatible`
- `disabled`

Provider classes:

- `built_in`
- `managed`
- `system`

### `yu engine install`

Install a YuTool-managed engine from a local manifest.

```bash
yu engine install --manifest ./imagemagick.json
yu engine install --manifest ./imagemagick.json --json
```

Installation is explicit and does not activate the version automatically.

For an explicitly supplied local package (PR #25):

```bash
yu engine install --manifest ./manifest.json --archive ./engine.zip --json
```

This path performs no download and never falls back to HTTP. It still validates Manifest v1, exact target, the SHA-256 of bounded staged bytes and archive safety. Manifest HTTPS URLs remain required as identity metadata but are not dereferenced. The local archive must be a regular non-symlink file and is not modified. Source size is capped at 512 MiB. Installation and activation remain separate; no engine executes during installation. Unpack the outer Actions artifact first and supply the inner engine archive.


### `yu engine versions`

```bash
yu engine versions imagemagick
yu engine versions imagemagick --json
```

Lists installed managed versions and marks the active version.

### `yu engine activate`

```bash
yu engine activate imagemagick 7.1.2
```

### `yu engine deactivate`

```bash
yu engine deactivate imagemagick
```

### `yu engine remove`

```bash
yu engine remove imagemagick 7.1.1
```

The active version cannot be removed. Built-in and system engines are outside the managed lifecycle and are never uninstalled by this command.

On Windows, quarantine rename retries access/sharing/lock-denied errors for up to two seconds while retaining the engine mutation lock. A persistent failure remains an error and preserves the installed version; no copy/delete fallback or privilege change is attempted. Other errors and non-Windows renames are not retried. Successful JSON removal receipts additionally expose `quarantine.attempts` and `quarantine.elapsed_ms`. A failed quarantine reports raw OS error and bounded diagnostic observations of paths/attributes/current directory in the message; it keeps the existing error code and does not claim a lock owner. See [v0.1 preparation](releasing/v0.1-readiness.md). PSD timeout errors also include a transport snapshot (PID, spawn/elapsed time, child-exit and stream-completion flags); these are observations before cleanup, not a claimed cause or permission diagnosis. Public timeout values and error codes are unchanged. PR #26 adds the diagnostic `phase` (`process_exit`, `input_completion`, `pipe_eof`) and observed `stdout_bytes`/`stderr_bytes` to distinguish a partial response from EOF. These fields remain human-readable message observations, not a new stable machine error schema or a guarantee that the process tree has exited. See [Windows lifecycle investigation](reliability/windows-lifecycle.md).

### Optional Windows investigation trace

`YU_WINDOWS_LIFECYCLE_TRACE_DIR` enables best-effort separate JSON snapshot files for a PSD invocation's own Job and direct child. It must name an existing absolute directory; default execution does not create trace files. This developer diagnostic records PID/creation time, stage, version directory and basic Job counts without document contents. It is not a new stable result schema, a root-cause diagnosis, an unlocker or a completion guarantee. The existing process/timeout/removal policy remains unchanged. See [capture details](reliability/windows-occupancy-capture.md). Python occupancy collection is test-harness-only; Python is not added as a core or runtime-engine dependency.

## Image commands

The first built-in raster engine is `raster-rs`.

Its initial default build supports PNG, JPEG, and WebP.

### `yu image info`

Inspect a raster image.

```bash
yu image info photo.jpg
yu image info photo.jpg --json
yu image info photo.jpg --engine raster-rs --json
```

Structured result fields currently include:

- path;
- format;
- width;
- height;
- color type;
- bit depth per channel;
- channel count;
- alpha presence;
- selected engine.

### `yu image resize`

Resize an image to a new file.

```bash
yu image resize input.jpg --width 1024 -o output.jpg
yu image resize input.jpg --height 720 -o output.jpg
yu image resize input.jpg --width 1024 --engine raster-rs -o output.jpg
```

Semantics:

- `--width` only: preserve aspect ratio;
- `--height` only: preserve aspect ratio;
- both width and height: resize to those exact dimensions;
- at least one dimension is required;
- width/height must be greater than zero;
- output format is inferred from the output extension;
- the output path must not already exist;
- source files are never overwritten by this v0.1 operation.

### `yu image crop` — planned

```bash
yu image crop input.png   --x 100   --y 100   --width 500   --height 500   -o output.png
```

### `yu image rotate` — planned

```bash
yu image rotate input.png --degrees 90 -o output.png
```

### `yu image convert` — planned

```bash
yu image convert input.png -o output.webp
```

## PSD commands

ADR 0006 selects `ag-psd 31.0.2` as the preferred v0.1 Managed PSD engine. PR #22 wires the four read-only commands below to the explicitly activated Managed package. PR #23 adds partial 8-bit RGB layer bitmap export; rendering remains deferred.

All five implemented commands accept `--engine ag-psd`, `--json`, and `--timeout-secs <1..3600>` (default: 30). Input paths are resolved from the caller's working directory, must identify a regular file, and must be representable as UTF-8 by Protocol v1. Commands never modify the source file. Names are passed as JSON data, not shell commands.

The timeout covers engine process and protocol I/O, not filesystem discovery or package installation. Request/stdout/stderr limits are 64 KiB / 16 MiB / 64 KiB. Oversized output, timeouts, non-zero process exits, invalid transport, or invalid typed results return `EXECUTION_FAILED` (exit 1). Node runtime override variables `NODE_OPTIONS` and `NODE_PATH` are not inherited.

The default production path must not require a system Node.js installation. If no compatible active Managed PSD engine exists, commands return a structured `ENGINE_UNAVAILABLE` result rather than silently installing or activating one.

PSD execution does not silently fail over to psd-tools. Use explicit `--engine` selection for an alternate implementation when such a provider is productized.

### `yu psd inspect`

```bash
yu psd inspect design.psd
yu psd inspect design.psd --json
```

### `yu psd tree`

```bash
yu psd tree design.psd
yu psd tree design.psd --json
```

A machine-readable result uses backend-independent layer IDs from PSD Capability Contract v1.

IDs are assigned from the canonical logical layer tree using one-based depth-first pre-order traversal:

```text
L0001
L0002
...
L10000
```

Layer names are presentation data only. Duplicate names are valid and must never be used as an implicit selector.

Example shape:

```json
{
  "schema_version": "1",
  "operation": "psd.tree",
  "engine": {"id": "ag-psd", "provider": "managed", "version": "31.0.2+node22.23.3"},
  "result": {
    "contract_version": "1",
    "document": {
      "format": "psd", "width": 1920, "height": 1080,
      "channels": 3, "bits_per_channel": 8, "color_mode": "rgb",
      "layer_count": 1, "maximum_tree_depth": 1
    },
    "layers": [{
      "id": "L0001", "depth": 1, "name": "Background", "kind": "pixel",
      "visible": true, "has_pixel_mask": false, "has_vector_mask": false,
      "child_count": 0, "children": []
    }]
  },
  "warnings": []
}
```

### `yu psd layer list`

```bash
yu psd layer list design.psd
yu psd layer list design.psd --json
```

### `yu psd layer info`

```bash
yu psd layer info design.psd --id L0007
```

Future selectors may include `--path` and `--name`, but ambiguous names must never silently select an arbitrary layer. The canonical `--id` remains the stable v1 selector for one parsed logical tree.

Malformed/non-canonical IDs and IDs absent from the document return `INVALID_ARGUMENT` (exit 2). Invalid/corrupt files return `INVALID_INPUT` (exit 2). Missing/inactive/broken engine state returns `ENGINE_UNAVAILABLE` (exit 3); a valid active version that does not advertise the requested operation returns `ENGINE_INCOMPATIBLE` (exit 3). Alternate PSD engines are not wired in this build and are never used as fallback.

### `yu psd layer export` — implemented / partial

```bash
yu psd layer export design.psd --id L0007 -o layer.png --json
yu psd layer export design.psb --id L0007 -o layer.png --engine ag-psd --timeout-secs 30
```

Requires an explicitly activated export-capable package. The first such package is `31.0.2+node22.23.3.yu2`; the older PR #21 package keeps its four read-only capabilities and is not implicitly upgraded.

The implementation accepts **8-bit RGB PSD/PSB**, exporting the selected layer's stored bitmap as a static PNG with straight RGBA8 pixels. The bitmap uses the layer's own pixel dimensions, not full-canvas placement. Layer names are not selectors. A missing ID returns `INVALID_ARGUMENT`; groups, empty/non-materialized bitmaps, non-RGB color modes, and 16/32-bit documents return `UNSUPPORTED_CAPABILITY` rather than being silently composited or converted.

This is not a render operation: masks, opacity, blending, effects, group composition and ICC color conversion are not applied. Text, shape and Smart Object layers can only yield their stored bitmap, not freshly rendered content. Successful output includes a warning describing these limitations.

The destination must have a `.png` extension and an existing directory parent. Existing files, directories and even dangling symlinks are refused with `OUTPUT_CONFLICT`; there is no overwrite option. Source-file aliases are therefore not writable destinations. Non-UTF-8 paths are rejected rather than converted lossily.

The host passes a private staging path to the engine, validates the response identity and decodes the entire PNG to verify RGBA8, dimensions, CRCs and complete image data, then publishes through a same-filesystem hard link. This is an atomic no-clobber operation, including when another writer creates the destination after preflight. Filesystems without hard-link support fail with `EXECUTION_FAILED`; there is no non-atomic copy or overwrite fallback. Ordinary failed executions clean staging and do not publish the final output; crash/power-loss recovery is not promised.

Limits: export input 512 MiB, decoded RGBA8 bitmap 256 MiB, encoded PNG 320 MiB. These are individual bounds, not a whole-process memory guarantee. The process/I/O deadline is also checked after PNG validation and before publication; filesystem operations and PNG decoding are not interruptible mid-call.

The schema-v1 success envelope includes `engine` (ID/provider/version), `warnings`, and this result:

```json
{
  "contract_version": "1",
  "layer_id": "L0007",
  "output_path": "/absolute/output/layer.png",
  "width": 128,
  "height": 64,
  "pixel_format": "rgba8",
  "container": "png"
}
```

The public output path is the final canonical-parent destination, never the private staging path. PNG validation failures and mismatched engine responses return `EXECUTION_FAILED` without publishing an output.

### `yu psd render` — deferred

```bash
yu psd render design.psd -o preview.png
```

Rendering fidelity depends on the selected engine and document features. Structured output should report the actual engine used and relevant warnings.

## Common options

Current/planned common options:

```text
--json
--engine <engine-id>
--verbose      (planned)
--quiet        (planned)
```

Mutation-oriented commands may later support:

```text
--dry-run
--overwrite
```

`--overwrite` must be explicit when source or destination conflict would otherwise destroy data.

## Engine selection

Default engine resolution:

1. explicitly selected engine;
2. compatible built-in engine;
3. compatible managed engine;
4. compatible system engine;
5. structured unavailable/unsupported error.

An engine being installed does not guarantee compatibility with every input.

## JSON envelope

Successful capability execution uses a stable envelope direction:

```json
{
  "schema_version": "1",
  "operation": "image.resize",
  "engine": {
    "id": "raster-rs",
    "provider": "built_in",
    "version": "0.1.0"
  },
  "result": {},
  "warnings": []
}
```

Do not include terminal decoration or progress output in JSON mode.

## Structured errors

JSON-mode failures are emitted to stderr.

Once a PSD engine command has been selected, failures also include optional top-level `engine` metadata (`id`, `provider`, `version`). Pre-selection errors keep the original schema-v1 error shape. Argument parser failures are structured when `--json` is present before the `--` positional separator; help/version output remains human-readable and exits 0.

Example:

```json
{
  "schema_version": "1",
  "error": {
    "code": "OUTPUT_CONFLICT",
    "message": "output already exists: output.png"
  }
}
```

Initial error codes include:

- `INVALID_ARGUMENT`
- `INVALID_INPUT`
- `UNSUPPORTED_CAPABILITY`
- `ENGINE_UNAVAILABLE`
- `ENGINE_INCOMPATIBLE`
- `EXECUTION_FAILED`
- `OUTPUT_CONFLICT`
- `VERIFICATION_FAILED`

## Exit codes

Initial mapping:

| Code | Meaning |
| ---: | --- |
| 0 | success |
| 1 | execution/runtime failure |
| 2 | invalid arguments, invalid input, or output conflict |
| 3 | capability or engine unavailable/incompatible |

More granular error meaning belongs in structured output rather than an excessively large exit-code table.

## stdout / stderr

- successful human output: stdout;
- successful JSON output: stdout;
- warnings/diagnostics: stderr;
- JSON error envelope: stderr;
- progress UI: stderr or an interactive presentation layer, never mixed into JSON stdout.

## Compatibility

After the first stable release:

- adding optional fields is preferred over changing field meaning;
- removing/renaming fields requires a schema-version decision;
- public commands should not silently change semantics;
- experimental commands must be explicitly labeled.
