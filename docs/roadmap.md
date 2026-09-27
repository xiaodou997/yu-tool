# Roadmap

This roadmap intentionally prioritizes proving YuTool's runtime model before expanding into many tool categories.

Dates are not committed here. Milestones are capability-based.

## M0 — Documentation bootstrap

**Goal:** freeze the product language and architecture boundaries before implementation.

- [x] Define YuTool / `yu` naming
- [x] Define project vision
- [x] Define capability vs engine model
- [x] Define Built-in / Managed / System engine classes
- [x] Draft CLI contract
- [x] Draft capability matrix
- [x] Define agent usage rules
- [x] Record initial engine-strategy ADR
- [x] Rename repository from `img-cli` to `yu-tool`
- [ ] Set repository description/topics
- [ ] Choose license

Exit criteria:

- contributors and coding agents can explain what YuTool is and is not;
- v0.1 command shape is documented;
- optional engines are not accidentally treated as hard dependencies.

## M1 — Rust core and first built-in capability

**Goal:** prove the smallest useful YuTool runtime without external engines.

Progress:

- [x] Rust workspace bootstrap
- [x] `yu` CLI executable
- [x] shared core result/error types
- [x] capability registry
- [x] engine registry
- [x] engine resolver
- [x] `yu doctor`
- [x] `yu capabilities`
- [x] `yu engine list`
- [x] built-in raster engine (`raster-rs`)
- [x] initial `image.info`
- [x] initial `image.resize`
- [x] JSON output contract
- [x] integration tests

The first built-in raster implementation uses the Rust `image` crate with a deliberately small default format set: PNG, JPEG, and WebP.

Performance-specialized resizing (for example a future `fast_image_resize` integration) is intentionally deferred until the public resize semantics are proven.

Exit criteria:

```bash
yu doctor
yu capabilities --json
yu image info input.png --json
yu image resize input.png --width 1024 -o output.png --json
```

work on supported platforms without ImageMagick, Python, Node, or another optional engine.

### M1 Freeze

M1 is frozen once PR #3 lands and the freeze receipt in `docs/milestones/m1-freeze.md` records the accepted CLI/protocol baseline and green cross-platform CI.

## M2 — Engine Manager foundation

**Goal:** prove optional engine lifecycle management.

Progress:

- [x] engine manifest v1 model;
- [x] platform/architecture matching foundation;
- [x] managed-engine storage layout;
- [x] download staging;
- [x] checksum/integrity verification;
- [x] safe archive extraction;
- [x] atomic activation;
- [x] installed version discovery;
- [x] active/current version state;
- [x] managed version removal;
- [x] inter-process engine mutation lock;
- [x] public managed-engine CLI;
- [x] system executable discovery foundation;
- [x] system engine version probing (ImageMagick / FFmpeg / ExifTool);
- [x] unified Built-in / Managed / System inventory;
- [x] `yu engine info`;
- [x] `yu doctor` unified inventory integration;
- [x] managed engine state diagnostics.

CLI target:

```bash
yu engine list
yu engine install --manifest <file>
yu engine versions <engine>
yu engine activate <engine> <version>
yu engine deactivate <engine>
yu engine remove <engine> <version>
yu doctor
```

Important constraint:

YuTool should not silently invoke Homebrew, apt, winget, sudo, or administrator elevation as part of an unrelated operation.

Exit criteria:

- a managed engine can be installed, verified, activated, listed, used, and removed;
- a compatible system engine can be discovered without YuTool taking ownership of it.

### M2 Freeze

M2 functional behavior is frozen at `d95ccae8cdaefd4ea63a35d39d6ec803748036e8`. PR #9 records the accepted Engine Manager, lifecycle, unified inventory, CLI, security, and schema-v1 compatibility baseline in `docs/milestones/m2-freeze.md`.

M3 may extend YuTool with PSD capabilities and new engines, but should not casually break the M2 lifecycle or inventory contracts.

## M3 — PSD engine spike

**Goal:** select a practical PSD/PSB strategy based on fixtures rather than assumptions.

Progress:

- [x] engine-neutral PSD spike harness;
- [x] fixture corpus schema/provenance rules;
- [x] initial synthetic malformed fixture;
- [x] PSD/PSB fixture corpus v1 (pixel layers, groups, duplicate names, text, masks);
- [x] pinned third-party fixture provenance/license receipts;
- [x] Rust / psd-tools / TypeScript candidate adapter skeletons;
- [x] machine-readable per-candidate report model;
- [x] representative redistributable PSD/PSB benchmark corpus;
- [x] first Rust-native candidate integration (rawpsd 0.2.2);
- [x] psd-tools 1.20.0 reference candidate integration (Python 3.12);
- [x] TypeScript/Node candidate integration (ag-psd 31.0.2);
- [x] candidate comparison report v1 (conformance, distribution, maintenance evidence);
- [x] controlled performance/memory benchmark report v2;
- [ ] PSD engine strategy decision.

Build a representative fixture corpus covering, where legally distributable:

- simple pixel layers;
- nested groups;
- duplicate layer names;
- text layers;
- masks;
- blend modes;
- effects;
- Smart Object metadata;
- higher bit depth where applicable;
- PSD and PSB;
- malformed/edge-case inputs.

Evaluate candidate implementations across:

- parse success;
- layer-tree fidelity;
- metadata coverage;
- layer export;
- rendering fidelity;
- PSD vs PSB;
- round-trip behavior where writing is supported;
- performance;
- memory usage;
- platform/distribution cost;
- maintenance risk.

Candidate categories include:

- Rust-native PSD implementations;
- psd-tools;
- TypeScript/Node PSD implementations where useful;
- other mature implementations discovered during the spike.

Deliverables:

- benchmark/conformance report;
- recommended built-in PSD engine, if one is mature enough;
- recommended compatibility/managed engine, if useful;
- explicit unsupported/partial capability list.

Exit criteria:

```bash
yu psd inspect design.psd --json
yu psd tree design.psd --json
yu psd layer list design.psd --json
yu psd layer export design.psd --id <id> -o layer.png --json
```

have a tested engine strategy.

## M4 — Safe mutation

**Goal:** introduce modifications without compromising source-file safety.

Candidate capabilities:

- image crop/rotate/convert;
- PSD layer rename where supported;
- PSD show/hide where supported;
- opacity changes where supported;
- safe output replacement;
- `--dry-run`;
- structured change summary;
- post-operation validation.

Any PSD mutation capability must be gated by actual engine conformance results.

## M5 — YuTool Manager

**Goal:** provide a small GUI for runtime/engine management.

Initial GUI scope:

- installed engines;
- available managed engines;
- version;
- provider class;
- capability list;
- install/update/remove;
- health state;
- disk usage;
- license/source metadata.

The first Manager is **not** an image editor.

Preferred direction: Tauri sharing the Rust core with the CLI.

## After the runtime is proven

Only after the engine/runtime architecture is stable should YuTool expand into additional capability families.

Potential areas include:

```text
PDF
media / FFmpeg
metadata
SVG/vector
OCR
archives
documents
RAW
```

Each new family should begin with:

1. a capability contract;
2. engine candidates;
3. conformance fixtures;
4. an explicit scope decision.

## Non-goals for the first release

Do not make v0.1 responsible for:

- every image format;
- Photoshop-level PSD editing;
- full PDF tooling;
- video/audio transcoding;
- OCR;
- a general plugin marketplace;
- remote/cloud execution;
- an image-editing GUI.

The first release should prove that **one lightweight runtime can safely discover, resolve, execute, and report local capabilities through `yu`.**
