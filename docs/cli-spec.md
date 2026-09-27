# CLI Specification

> Status: **M2 frozen baseline / v0.1 evolving**

The public executable is:

```bash
yu
```

This document defines the public command model through the M2 frozen baseline. Commands explicitly marked **planned** belong to later milestones.

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

ADR 0006 selects `ag-psd 31.0.2` as the preferred v0.1 Managed PSD engine. The commands below remain planned until the Managed package and public capability schema are implemented.

The default production path must not require a system Node.js installation. If no compatible active Managed PSD engine exists, commands return a structured `ENGINE_UNAVAILABLE` result rather than silently installing or activating one.

PSD execution does not silently fail over to psd-tools. Use explicit `--engine` selection for an alternate implementation when such a provider is productized.

### `yu psd inspect` — planned

```bash
yu psd inspect design.psd
yu psd inspect design.psd --json
```

### `yu psd tree` — planned

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
  "document": {
    "width": 1920,
    "height": 1080
  },
  "layers": [
    {
      "id": "L0001",
      "name": "Background",
      "kind": "pixel",
      "visible": true,
      "children": []
    }
  ]
}
```

### `yu psd layer list` — planned

```bash
yu psd layer list design.psd
yu psd layer list design.psd --json
```

### `yu psd layer info` — planned

```bash
yu psd layer info design.psd --id L0007
```

Future selectors may include `--path` and `--name`, but ambiguous names must never silently select an arbitrary layer. The canonical `--id` remains the stable v1 selector for one parsed logical tree.

### `yu psd layer export` — planned / partial v0.1 contract

```bash
yu psd layer export design.psd --id L0007 -o layer.png
```

Initial semantics:

- source scope: 8-bit PSD/PSB documents;
- engine materializes the selected layer bitmap;
- YuTool normalizes pixels to RGBA8;
- v0.1 output container is PNG;
- selected engine/version/provider is included in structured output;
- this is not a Photoshop-equivalent full-document render promise;
- 16-bit/32-bit layer export returns `UNSUPPORTED_CAPABILITY` until a high-bit normalization contract is accepted.

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
