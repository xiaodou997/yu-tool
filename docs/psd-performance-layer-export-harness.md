# PSD Performance / Layer Export Benchmark Harness

PR #16 adds a controlled M3 benchmark contract without selecting a default PSD engine.

## Why this exists

The earlier corpus report records correctness durations, but those numbers are not a valid performance comparison:

- `rawpsd` runs in-process;
- `psd-tools` runs through a Python subprocess;
- `ag-psd` runs through a Node.js subprocess;
- GitHub-hosted runners add scheduler and machine noise.

The benchmark harness therefore separates three different costs instead of presenting one ambiguous duration.

| Operation | What is measured |
| --- | --- |
| `cold_inspect` | End-to-end adapter inspection, including file I/O and Python/Node process startup where applicable |
| `warm_parse` | Repeated parsing inside one already-started runtime, with input bytes already loaded |
| `layer_export_materialize` | Parsed document held outside the timer; layer bitmap decode/materialization to RGBA bytes plus SHA-256 fingerprinting |

## Committed smoke plan

The machine-readable plan is:

```text
docs/data/psd-benchmark-plan-v1.json
```

It intentionally uses the existing `simple-pixel-layers-psd` fixture.

The fixture is only about 14 KB, so the plan freezes:

```text
ranking_allowed = false
```

Numbers produced by this plan validate the measurement machinery and make regressions inspectable. They must not be used to claim that one candidate is generally faster.

A later representative benchmark should add larger local or redistributable PSD/PSB inputs before performance ranking is allowed.

## Layer export contract

PR #16 treats layer export as bitmap materialization, not full Photoshop rendering fidelity.

### psd-tools

The adapter opens the document once per export sample and measures `Layer.topil(apply_icc=False)` materialized as RGBA bytes.

### ag-psd

The adapter reads with `useRawData: true` and uses `getLayerImageData(layer)`. Because that helper still allocates through ag-psd's ImageData hook, the benchmark installs a pure in-memory `Uint8ClampedArray` ImageData shim. This exercises bitmap decoding without requiring `node-canvas` or a native graphics runtime.

### rawpsd

`rawpsd 0.2.2` exposes low-level image data, but the current M3 adapter does not yet expose a normalized RGBA layer-export contract. The report therefore records layer export as `unsupported` rather than pretending the capabilities are equivalent.

## Fingerprints

Each export-capable adapter records:

- exported layer count;
- total RGBA byte count;
- SHA-256 fingerprint of materialized layer bytes.

The fingerprint detects instability inside one candidate's repeated runs. PR #16 does **not** require fingerprints from different engines to match, because cross-engine pixel fidelity is a separate Render/Fidelity evidence task.

## Run locally

Install the pinned optional runtimes first:

```bash
python -m pip install "psd-tools==1.20.0"

cd crates/yu-psd-spike/adapters/typescript
npm install --ignore-scripts --no-audit --no-fund
cd ../../../..
```

Then run:

```bash
cargo run -p yu-psd-spike -- \
  benchmark \
  fixtures/psd/corpus.json \
  docs/data/psd-benchmark-plan-v1.json
```

To persist a machine-readable report:

```bash
cargo run -p yu-psd-spike -- \
  benchmark \
  fixtures/psd/corpus.json \
  docs/data/psd-benchmark-plan-v1.json \
  target/psd-benchmark-report.json
```

Optional runtime executable overrides remain:

```text
YU_PSD_TOOLS_PYTHON
YU_TYPESCRIPT_PSD_NODE
```

## CI evidence

`PSD Spike` runs the complete benchmark contract on:

- Ubuntu;
- macOS;
- Windows.

Each platform uploads `psd-benchmark-<OS>` containing `psd-benchmark-report.json`.

The gate fails when:

- a required candidate cannot produce cold/warm samples;
- `psd-tools` or `ag-psd` cannot materialize at least one layer;
- a sample count changes unexpectedly;
- export byte/fingerprint metadata is missing;
- somebody changes the v1 plan to permit ranking.

## Still not measured

PR #16 does not close these M3 gaps:

- representative large PSD / PSB throughput;
- peak memory;
- render/composite fidelity;
- effects fidelity;
- Smart Object behavior;
- higher-bit-depth behavior;
- round-trip/write safety;
- production distribution/update strategy.

The benchmark harness is the measurement foundation for those later decisions, not the final Engine Strategy verdict.


## PR #18 representative suite

PR #18 keeps the frozen seven-fixture conformance corpus unchanged and adds a separate benchmark-only corpus:

```text
fixtures/psd/benchmark/corpus.json
```

The representative suite is declared in:

```text
docs/data/psd-benchmark-suite-v2.json
```

and covers:

- a larger 8-bit RGB baseline shared by all three candidates;
- advanced blending;
- mask-heavy content;
- layer effects;
- placed / Smart Object metadata;
- 16-bit RGB PSD;
- 32-bit PSB.

Each workload still has `ranking_allowed = false`. The suite exists to collect evidence across feature classes and sizes, not to manufacture a single winner score.

The canonical report job runs all workloads sequentially on one `ubuntu-latest` runner and uploads one aggregate JSON artifact. Existing Ubuntu/macOS/Windows smoke jobs remain the cross-platform compatibility gate.

### Peak RSS

Candidate reports now include `peak_rss_bytes` for the benchmark worker:

- rawpsd: process peak RSS from the Rust benchmark process using native OS APIs;
- psd-tools: Python worker peak RSS;
- ag-psd: Node worker peak RSS.

This is the peak resident set for the warm benchmark worker/process. It is not a whole-application memory budget and does not include package installation.

### Controlled-report rule

The committed controlled report may summarize the canonical Ubuntu suite, but it must retain:

```text
ranking_allowed = false
```

and must not turn GitHub-hosted-runner timing into a universal performance ranking.


## Controlled report snapshot

The first canonical suite snapshot is committed at:

```text
docs/data/psd-benchmark-report-v2.json
docs/psd-controlled-benchmark-report.md
```

It is sourced from PSD Spike workflow run `36135069883` and remains non-ranking. Standard Rust tests guard the report ID, workload count, source run, and the currently unresolved high-bit-depth export divergence.
