# Representative PSD Benchmark Corpus

This directory is separate from the frozen M3 conformance corpus.

It contains redistribution-safe benchmark-only inputs copied from `psd-tools/psd-tools` at commit `f1256273ffb9b39b9efa19d486c86f594c831f42` under the upstream MIT license already stored at `fixtures/psd/licenses/psd-tools-MIT.txt`.

The benchmark corpus intentionally expands file size and feature diversity without changing the PR #15 seven-fixture correctness baseline.

| Fixture | Approx. size | Benchmark purpose |
| --- | ---: | --- |
| `bench-baseline-rgb` | 500 KB | shared 8-bit RGB baseline for all three candidates |
| `bench-advanced-blending` | 961 KB | larger RGB / advanced blend structure |
| `bench-masks` | 816 KB | mask-heavy document |
| `bench-layer-effects` | 735 KB | layer-effects structure |
| `bench-smart-object` | 115 KB | placed/smart-object metadata |
| `bench-high-bit-rgb` | 415 KB | 16-bit RGB PSD |
| `bench-high-bit-psb` | 283 KB | 32-bit PSB / Large Document path |

These files are benchmark inputs, not new conformance expectations. Unsupported candidates are omitted from workloads that exceed their declared capability boundary.
