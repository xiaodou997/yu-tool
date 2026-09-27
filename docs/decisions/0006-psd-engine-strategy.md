# ADR 0006: PSD Engine Strategy for v0.1

- **Status:** Accepted
- **Date:** 2026-09-27
- **Milestone:** M3

## Context

M3 evaluated three PSD/PSB candidates against one engine-neutral conformance corpus and a separate representative benchmark suite.

The frozen evidence is:

| Candidate | Runtime | Corpus v1 |
| --- | --- | ---: |
| psd-tools 1.20.0 | Python 3.12 | 7/7 |
| rawpsd 0.2.2 | Rust native | 4/7 |
| ag-psd 31.0.2 | Node.js 22 | 7/7 |

The controlled benchmark additionally established:

- psd-tools and ag-psd produced identical materialized layer bytes on the tested 8-bit workloads;
- ag-psd had lower observed warm parse and layer-export medians on the canonical Ubuntu run;
- psd-tools had lower observed external-worker peak RSS on that run;
- 16-bit PSD and 32-bit PSB layer materialization were not cross-engine equivalent;
- rawpsd still lacks the required PSD/PSB feature coverage and normalized layer-export contract.

The project now needs a concrete v0.1 production strategy rather than another candidate comparison.

Machine-readable decision:

```text
docs/data/psd-engine-strategy-v1.json
```

## Decision

### 1. v0.1 will not ship a built-in PSD engine

No current Rust-native candidate satisfies the M3 correctness baseline strongly enough to become a Built-in PSD engine.

`rawpsd 0.2.2` remains useful research evidence, but it is not part of the v0.1 production PSD execution path.

This preserves YuTool's rule that "Rust-first core" does not mean "force every capability into Rust before it is mature enough."

### 2. ag-psd is the preferred v0.1 PSD engine

The preferred implementation is:

```text
engine:   ag-psd
version:  31.0.2
runtime:  Node.js 22
provider: managed
```

It is the selected implementation target for:

- `psd.inspect`;
- `psd.tree`;
- `psd.layer.list`;
- `psd.layer.info`;
- `psd.layer.export` within the v0.1 export contract.

Reasons:

- 7/7 corpus-v1 conformance on Ubuntu, macOS, and Windows;
- PSD and PSB coverage;
- text, mask, hierarchy, and duplicate-name coverage;
- matching 8-bit layer-materialization fingerprints against psd-tools on representative workloads;
- smaller direct package dependency surface than the Python reference path;
- favorable observed cold/warm/export timing on the canonical benchmark run;
- usable raw-data APIs without requiring node-canvas for the tested structural/export path.

This is a product strategy decision, not a claim that ag-psd is universally faster or more correct than every alternative.

### 3. ag-psd remains optional and Managed

Node.js and ag-psd must not become YuTool core dependencies.

The production direction is a **self-contained Managed engine package** under YuTool-owned storage.

The package should expose one YuTool-compatible executable entrypoint and privately carry the runtime assets it needs. The default production path must not require users to install or modify a system Node.js installation.

Installation and activation remain governed by the M2 lifecycle:

- no silent install;
- no automatic activation;
- no Homebrew/apt/winget mutation;
- verified YuTool-owned artifacts only.

Until that package exists, M3 has selected the engine strategy but has not declared the PSD capabilities generally available.

### 4. psd-tools remains the reference and explicit compatibility engine

`psd-tools 1.20.0 / Python 3.12` remains valuable because it:

- passes the entire corpus;
- provides an independent implementation for differential testing;
- has a mature PSD ecosystem and export/compositing APIs;
- showed lower external-worker peak RSS than ag-psd in the canonical benchmark.

It is **not** the v0.1 default engine.

YuTool must not silently fail over from ag-psd to psd-tools. The two engines already show different high-bit-depth layer materialization semantics, so implicit fallback could silently change output meaning.

A future productized psd-tools provider may be used only through explicit engine selection or explicit policy.

### 5. PSD engine fallback is not automatic in v0.1

For PSD capabilities:

1. explicit `--engine` selection wins;
2. otherwise use the active compatible Managed ag-psd engine;
3. if it is unavailable or incompatible, return a structured error;
4. do not silently retry another PSD engine.

The selected engine ID/version/provider must remain visible in structured results.

This keeps output behavior reproducible and follows M2's explicit-state philosophy.

### 6. v0.1 layer export is an 8-bit normalized bitmap capability

The v0.1 `psd.layer.export` target is:

```text
source scope:       8-bit documents
normalized pixels:  RGBA8
output container:   PNG
semantic:           materialized layer bitmap
```

It is **not** a Photoshop-equivalent full-document render contract.

The command must not imply that layer effects, adjustment composition, blending context, or Smart Object rendering are reproduced exactly unless separate fidelity tests prove that behavior.

### 7. High-bit-depth layer export is explicitly unsupported in v0.1

The benchmark found:

```text
16-bit PSD:
psd-tools bytes : ag-psd bytes = 1 : 2

32-bit PSB:
psd-tools bytes : ag-psd bytes = 1 : 4
```

Therefore YuTool will not silently normalize or truncate high-bit-depth layer export in v0.1.

For 16-bit/32-bit source documents:

- inspection/tree/list may be implemented only after capability-specific conformance tests;
- `psd.layer.export` returns a structured unsupported-capability result until a bit-depth contract is accepted.

A later ADR should decide whether YuTool:

- always normalizes to RGBA8;
- preserves source sample width;
- or exposes both normalized and native/raw export modes.

### 8. Full render and mutation are deferred

The following are outside the accepted v0.1 PSD contract:

- `psd.render` fidelity claims;
- PSD round-trip writing;
- layer rename/show/hide/opacity mutation;
- effects-accurate rendering;
- Smart Object content extraction/editing;
- high-bit-depth layer export.

These require separate conformance and source-file safety evidence.

## v0.1 PSD capability posture

| Capability | Strategy |
| --- | --- |
| `psd.inspect` | ag-psd Managed engine target |
| `psd.tree` | ag-psd Managed engine target |
| `psd.layer.list` | ag-psd Managed engine target |
| `psd.layer.info` | ag-psd Managed engine target |
| `psd.layer.export` | Partial: 8-bit materialized bitmap → RGBA8/PNG |
| `psd.render` | Deferred |
| PSD mutation/write | Deferred |
| High-bit layer export | Unsupported in v0.1 |

## Consequences

### Positive

- the lightweight Rust core remains free of Python/Node hard dependencies;
- v0.1 gets a concrete PSD/PSB implementation path backed by cross-platform evidence;
- 8-bit layer export has independent cross-engine consistency evidence;
- the default path has a smaller direct dependency surface than the Python reference stack;
- psd-tools remains available as an independent oracle instead of being discarded;
- high-bit ambiguity is surfaced instead of hidden.

### Costs

- the first production PSD engine is not Rust-native;
- YuTool must package/update a private Managed Node runtime or equivalent self-contained entrypoint;
- no silent compatibility fallback is available;
- high-bit layer export is intentionally unavailable;
- render and mutation remain future work.

## Rejected alternatives

### Make rawpsd the Built-in default

Rejected for v0.1 because the pinned candidate passes only 4/7 corpus-v1 cases, rejects PSB, and does not yet provide the required normalized text/mask/export behavior.

### Make psd-tools the default

Viable technically, but not selected for v0.1 because the Python dependency/runtime surface is larger and the controlled run showed materially higher observed operation latency than ag-psd on the tested workloads.

psd-tools remains the reference/compatibility engine because those tradeoffs do not reduce its value as an independent implementation.

### Bundle both external engines and fail over automatically

Rejected because silent fallback would make execution/output semantics depend on failure mode, and the high-bit benchmark already proves the engines do not always materialize equivalent pixel representations.

### Delay PSD until a Rust implementation reaches parity

Rejected because YuTool's architecture explicitly permits mature external Managed engines. Waiting for a Rust-only implementation would block useful PSD/PSB capabilities without improving the core architecture.

## Follow-up

M3 strategy work is complete after this ADR lands.

Implementation should proceed in this order:

1. ~~define the stable PSD capability/result schema and engine protocol~~ — completed by PR #20;
2. package ag-psd 31.0.2 + private Node 22 runtime as a Managed engine prototype;
3. wire `psd.inspect`, `psd.tree`, and `psd.layer.list`;
4. add stable backend-independent layer IDs;
5. add `psd.layer.info`;
6. add 8-bit `psd.layer.export` to RGBA8/PNG;
7. add release conformance for the Managed package on macOS, Windows, and Linux;
8. freeze M3 and move mutation/render work to later milestones.
