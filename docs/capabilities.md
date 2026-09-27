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
| `image.crop` | Planned | Rust built-in | ImageMagick/libvips |
| `image.rotate` | Planned | Rust built-in | ImageMagick |
| `image.convert` | Planned | Rust built-in where format support exists | ImageMagick/libvips |

Current resize semantics:

- width only: preserve aspect ratio;
- height only: preserve aspect ratio;
- width + height: exact target dimensions;
- output must be a new path;
- existing output files are rejected rather than overwritten;
- encoding is selected from the output extension.

The first implementation deliberately favors a small, predictable dependency footprint over maximum format breadth or peak resizing throughput.

## PSD / PSB

M3 selected the v0.1 engine strategy in ADR 0006.

Preferred production direction:

```text
engine:   ag-psd 31.0.2
runtime:  private Node.js 22
provider: managed
```

Node/ag-psd remain optional and must not become YuTool core dependencies. The production Managed package should be self-contained in YuTool-owned storage rather than depending on a user-managed system Node installation.

| Capability | v0.1 target | Selected strategy |
| --- | --- | --- |
| `psd.inspect` | Planned | Managed ag-psd |
| `psd.tree` | Planned | Managed ag-psd |
| `psd.layer.list` | Planned | Managed ag-psd |
| `psd.layer.info` | Planned | Managed ag-psd |
| `psd.layer.export` | Partial target | 8-bit materialized layer bitmap → RGBA8/PNG via ag-psd |
| `psd.render` | Deferred | no v0.1 fidelity promise |
| high-bit layer export | Unsupported in v0.1 | normalization contract unresolved |
| layer rename | Future | mutation safety not frozen |
| show/hide layer | Future | mutation safety not frozen |
| layer opacity | Future | mutation safety not frozen |
| layer move/delete | Future | mutation safety not frozen |
| text-layer editing | Future/Research | must be capability-tested |
| Smart Object editing | Future/Research | must be capability-tested |

Compatibility roles:

- `psd-tools 1.20.0 / Python 3.12` remains the independent reference and explicit compatibility path;
- `rawpsd 0.2.2` remains experimental evidence and is not a v0.1 production engine;
- YuTool does not silently fail over between PSD engines because output semantics can differ.

The 8-bit export target means bitmap materialization, not Photoshop-equivalent full-document rendering.

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
