# M3 Controlled PSD Benchmark Report v2

This report freezes the first representative benchmark evidence produced by PR #18. It is **not** an engine ranking and does not select a default PSD engine.

Machine-readable snapshot: `docs/data/psd-benchmark-report-v2.json`.

## Evidence source

```text
Workflow:      PSD Spike
Run ID:        36135069883
Source head:   9ba3501546b7939188972f2ddd2d92951227f689
Artifact:      psd-benchmark-representative-ubuntu
Artifact hash: sha256:30649888dfe0ae88d5f2b8925ead3092986d9e52501ca3986da8a0f97cf668c2
Environment:   linux / x86_64 / ubuntu-latest
```

The suite ran all seven workloads sequentially on one GitHub-hosted Ubuntu runner.

```text
ranking_allowed = false
```

The numbers below describe this recorded environment only.

## Representative workloads

| Workload | Size | Coverage |
| --- | ---: | --- |
| baseline RGB | 500 KB | larger 8-bit RGB baseline |
| advanced blending | 961 KB | advanced blend structure |
| masks | 816 KB | mask-heavy PSD |
| layer effects | 735 KB | effects-heavy PSD |
| Smart Object | 115 KB | placed/smart-object metadata |
| high-bit RGB | 415 KB | 16-bit RGB PSD |
| high-bit PSB | 283 KB | 32-bit PSB |

## Median observations

| Workload | Candidate | Cold inspect ms | Warm parse ms | Layer export ms | Peak RSS MiB |
| --- | --- | ---: | ---: | ---: | ---: |
| baseline RGB | rawpsd | 62.25 | 62.45 | unsupported | 135.9 |
| baseline RGB | psd-tools | 280.36 | 1.18 | 12.92 | 60.1 |
| baseline RGB | ag-psd | 57.04 | 0.44 | 11.63 | 74.8 |
| advanced blending | psd-tools | 273.61 | 5.44 | 6.01 | 51.7 |
| advanced blending | ag-psd | 82.89 | 4.43 | 5.12 | 73.1 |
| masks | psd-tools | 304.68 | 28.64 | 11.43 | 56.6 |
| masks | ag-psd | 66.85 | 4.60 | 7.26 | 71.6 |
| layer effects | psd-tools | 412.83 | 129.91 | 10.87 | 55.7 |
| layer effects | ag-psd | 95.13 | 14.65 | 7.51 | 72.8 |
| Smart Object | psd-tools | 268.48 | 2.13 | 1.95 | 44.6 |
| Smart Object | ag-psd | 60.21 | 0.91 | 1.19 | 59.8 |
| 16-bit RGB | psd-tools | 271.67 | 2.78 | 45.20 | 44.8 |
| 16-bit RGB | ag-psd | 58.66 | 0.65 | 7.55 | 75.2 |
| 32-bit PSB | psd-tools | 269.54 | 1.23 | 60.36 | 43.9 |
| 32-bit PSB | ag-psd | 57.03 | 0.53 | 3.84 | 63.7 |

These medians are observations from one canonical run, not portable performance scores.

## What the run establishes

### 1. 8-bit layer export is consistent between psd-tools and ag-psd

For the five 8-bit workloads, both external candidates produced the same:

- exported layer count;
- total materialized byte count;
- SHA-256 fingerprint.

That includes baseline RGB, advanced blending, masks, layer effects, and the Smart Object workload.

This is strong fixture-level evidence that the current bitmap-materialization paths agree on these inputs.

### 2. High-bit-depth layer export is not normalized

The 16-bit PSD produced:

```text
psd-tools: 196,192 bytes
ag-psd:    392,384 bytes
```

The 32-bit PSB produced:

```text
psd-tools:  60,000 bytes
ag-psd:    240,000 bytes
```

The 2x and 4x byte-count ratios are consistent with different sample-width/materialization semantics. Their fingerprints differ as well.

Therefore the current M3 layer-export contract is **not cross-engine equivalent for high-bit-depth files**. A later contract must decide whether YuTool normalizes to RGBA8, preserves source bit depth, or exposes both representations.

### 3. Runtime startup materially affects cold inspect

On this Ubuntu run, Python subprocess-based cold inspect was much larger than psd-tools warm parse time. ag-psd also paid process startup, but its observed cold inspect was lower in this run.

This reinforces PR #16's decision to keep cold and warm measurements separate.

### 4. ag-psd had lower observed warm medians in this run

For every workload where both psd-tools and ag-psd participated, ag-psd had a lower observed median for warm parse and layer export on this runner.

This is an environment-specific observation, not a universal engine ranking.

### 5. psd-tools used lower worker peak RSS than ag-psd in the shared external workloads

On this run, psd-tools benchmark-worker peak RSS was roughly 44–60 MiB, while ag-psd was roughly 60–79 MiB.

The rawpsd baseline measured about 136 MiB, but that number is the in-process Rust benchmark/harness process and is not directly equivalent to the isolated external worker measurements. It must not be interpreted as parser-only heap use.

## Decision impact

The evidence now separates the candidates more clearly:

- **rawpsd** remains attractive for an in-process Rust path but is still functionally incomplete for the current PSD/PSB contract and lacks normalized layer export.
- **psd-tools** has complete corpus-v1 coverage, consistent 8-bit export, lower observed external-worker RSS in this run, and a mature compositing/export API surface.
- **ag-psd** has complete corpus-v1 coverage, consistent 8-bit export, and lower observed warm/cold operation medians than psd-tools on this canonical run.
- **High-bit-depth export semantics are unresolved** between psd-tools and ag-psd and should be treated as a strategy blocker for any promise of bit-depth-preserving layer export.

## Remaining gaps before Engine Strategy

PR #18 closes the representative benchmark-corpus and first controlled timing/RSS evidence gap. It does not close:

- render/composite fidelity against Photoshop reference output;
- explicit high-bit-depth normalization semantics;
- cross-engine effects fidelity;
- Smart Object content extraction fidelity;
- round-trip/write safety;
- production packaging/update strategy for Python or Node runtimes.

The next Engine Strategy step should either close those blockers or explicitly scope v0.1 PSD support to inspection/tree/list plus 8-bit layer export.
