# Capability Matrix

> This document distinguishes **product capabilities** from **candidate engine implementations**.

The matrix evolves as implementation spikes and conformance tests are completed.

## Status legend

| Status | Meaning |
| --- | --- |
| Planned | part of the intended milestone, not implemented yet |
| Experimental | implemented or spiked, but not stable |
| Supported | covered by the product contract and tests |
| Partial | usable with documented limitations |
| Unsupported | intentionally unavailable or blocked by known limitations |

## Bootstrap capabilities

| Capability | Status | Default implementation | Notes |
| --- | --- | --- | --- |
| `runtime.doctor` | Supported | Rust core | Diagnose runtime/engines |
| `runtime.capabilities` | Supported | Rust core | Effective capabilities |
| `engine.list` | Supported | Engine Manager + Rust core | Unified Built-in/Managed/System inventory |
| `engine.info` | Supported | Engine Manager + Rust core | All discovered providers for an engine ID |
| `engine.install` | Supported | Engine Manager | Local manifest; managed engines only |
| `engine.versions` | Supported | Engine Manager | Installed managed versions |
| `engine.activate` | Supported | Engine Manager | Explicit active-version switch |
| `engine.deactivate` | Supported | Engine Manager | Leaves versions installed |
| `engine.remove` | Supported | Engine Manager | Active/system/built-in removal refused |
| System version probe | Supported | Engine Manager | ImageMagick, FFmpeg, ExifTool when discovered on PATH |

## Raster image

The current built-in engine is:

```text
raster-rs
provider: built_in
formats: PNG, JPEG, WebP
```

| Capability | Status | Default implementation | Optional engines |
| --- | --- | --- | --- |
| `image.info` | Supported | `raster-rs` / Rust `image` | ImageMagick/libvips later |
| `image.resize` | Supported | `raster-rs` / Rust `image` | fast_image_resize/ImageMagick/libvips later |
| `image.crop` | Supported (M4a branch) | `raster-rs` | Bounds-checked rectangle; new PNG/JPEG/WebP output |
| `image.rotate` | Supported (M4a branch) | `raster-rs` | Clockwise 90/180/270 degrees; new output |
| `image.convert` | Supported (M4a branch) | `raster-rs` | PNG/JPEG/WebP output by file extension |

Current resize semantics:

- width only: preserve aspect ratio;
- height only: preserve aspect ratio;
- width + height: exact target dimensions;
- output must be a new path;
- existing output files are rejected rather than overwritten;
- encoding is selected from the output extension.

The first implementation deliberately favors a small, predictable dependency footprint over maximum format breadth or peak resizing throughput.

M4a adds crop, right-angle rotate and format convert using the existing built-in
engine. All three read the original image and publish only to a new destination.
Output extension controls encoding (PNG/JPEG/WebP); no automatic EXIF/ICC
preservation or lossless JPEG conversion is promised. Destination publication
uses a same-filesystem hard link from a completed temporary file, so an
existing destination is not overwritten, including on a concurrent race.
The output filesystem must support regular-file hard links.

M4b-1 adds `--dry-run` to all four built-in mutations (including resize):
read/decode the input, validate geometry and format, inspect output conflicts
and parent-directory existence, and return an additive `dry_run` boolean in
the JSON result. A successful plan performs no encoding or file writes and
does not guarantee that a later real execution can publish. The existing
no-clobber behavior is unchanged; overwrite/replacement is still deferred.

M4b-2 (`develop`) adds an **opt-in, content-hash-bound output replacement**
to the same four raster mutations: `--replace --expected-output-sha256 HASH`.
It refuses absent/non-regular/symlink destinations, wrong versions and
unverified writes. It stages encoded output before a final hash recheck and
OS replacement rename, holding a create-new sidecar lock against competing
YuTool operations. JSON adds `replaced` / `would_replace`; without explicit
replacement flags, no-clobber is unchanged. This is not an atomic compare-and-
swap against unrelated external writers, a promise of crash durability, or
preservation of the former destination's metadata/hardlink identity.

M4b-3 (`develop`) adds typed post-write verification receipts to all four
built-in raster mutations. YuTool decodes/checks the staged encoding before
publication, then reopens the **actual destination** to verify format,
dimensions and byte-level SHA-256 against staging. Success JSON includes
`output_receipt.status=verified`, an optional prior digest for replacement,
and `verified_output` with SHA-256, bytes, width, height and format.
Dry-run has `status=planned` without a fabricated new digest. Errors after
publication are `VERIFICATION_FAILED` and may leave a published output;
no automatic rollback or external-writer atomic CAS is promised.

## PSD / PSB

M3 selected the v0.1 engine strategy in ADR 0006. The implemented five-operation baseline is frozen in [M3 Freeze](milestones/m3-freeze.md), with supported/partial/unsupported boundaries and exact evidence. Capability availability still depends on the active package; implementation freeze is not public release.

Preferred production direction:

```text
engine:   ag-psd 31.0.2
runtime:  private Node.js 22
provider: managed
```

Node/ag-psd remain optional and must not become YuTool core dependencies. The production Managed package should be self-contained in YuTool-owned storage rather than depending on a user-managed system Node installation.

| Capability | v0.1 target | Selected strategy |
| --- | --- | --- |
| `psd.inspect` | Supported | Active Managed ag-psd; metadata only |
| `psd.tree` | Supported | Active Managed ag-psd; canonical logical tree |
| `psd.layer.list` | Supported | Active Managed ag-psd; preorder IDs |
| `psd.layer.info` | Supported | Active Managed ag-psd; explicit layer ID |
| `psd.layer.export` | Partial | Active Managed ag-psd `.yu2`; stored 8-bit RGB bitmap → RGBA8/PNG; new output only |
| `psd.render` | Deferred | no v0.1 fidelity promise |
| high-bit layer export | Unsupported in v0.1 | normalization contract unresolved |
| layer rename | Future | mutation safety not frozen |
| show/hide layer | Future | mutation safety not frozen |
| layer opacity | Future | mutation safety not frozen |
| layer move/delete | Future | mutation safety not frozen |
| text-layer editing | Future/Research | must be capability-tested |
| Smart Object editing | Future/Research | must be capability-tested |

PR #22 exposes the four read-only operations through the public CLI. PR #23 adds layer bitmap export to the separately versioned `31.0.2+node22.23.3.yu2` package. Effective capabilities use the exact active version, not the inventory union of all installed versions. This does not promise arbitrary PSD feature coverage or full rendering fidelity. The package is still a prototype with CI artifacts and placeholder distribution URLs; no public catalog is claimed.

Compatibility roles:

- `psd-tools 1.20.0 / Python 3.12` remains the independent reference and explicit compatibility path;
- `rawpsd 0.2.2` remains experimental evidence and is not a v0.1 production engine;
- YuTool does not silently fail over between PSD engines because output semantics can differ.

Export means stored bitmap materialization, not Photoshop-equivalent rendering. It does not apply masks, opacity, blending, effects, group composition or ICC conversion. Groups, absent/empty bitmaps, non-RGB modes and high-bit documents are rejected. PNG publication requires an existing output directory on a filesystem supporting hard links; existing destinations are never replaced. See the CLI specification and PR #23 test checklist for limits and verification.

## Engine-management capabilities

Planned engine metadata:

| Field | Purpose |
| --- | --- |
| engine ID | stable identifier |
| provider class | built-in / managed / system |
| version | actual active version |
| state | ready / missing / broken / incompatible / disabled |
| capabilities | operations implemented |
| platform support | OS/architecture compatibility |
| install size | useful for managed-engine UI |
| license metadata | installation transparency |

## Future capability families

These are **not v0.1 scope**.

| Family | Examples |
| --- | --- |
| PDF | inspect, extract, merge, render |
| Media | probe, transcode, extract audio/frame |
| Metadata | read/write EXIF/XMP |
| SVG/vector | inspect/render/convert |
| OCR | local text recognition |
| Archive | inspect/extract/create |
| Document | conversion and structural extraction |
| RAW | inspect/decode/convert |

## Capability discovery

`yu capabilities --json` reports what can be executed **now**, given:

- current platform;
- built-in features;
- managed engines installed;
- system engines discovered;
- input-independent compatibility checks.

A later operation can still fail if a particular input uses unsupported features.

## Conformance requirement

Two engines claiming the same capability should be testable against the same behavioral contract.

For example, implementations of `image.resize` should agree on:

- argument validation;
- output-file safety;
- structured result shape;
- cancellation/error classification;
- basic dimension semantics.

Pixel-perfect equality is not necessarily required when algorithms differ, but behavioral differences must be explicit.
