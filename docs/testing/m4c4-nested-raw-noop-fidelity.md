# M4c-4 Acceptance — nested PSD blocks and no-op fidelity comparison

Branch: `research/m4c4-psd-nested-block-fidelity`

Base: `develop@0151a31e11aadb361505b1d87bbf24f2d4a434f1`

## Research acceptance

- [x] Parse embedded `Lr16` / `Lr32` structures with bounded signed layer
  count, record fields, per-channel length/offset/hash and tagged metadata.
- [x] Decode PSD/PSB 32-/64-bit framing without integer truncation.
- [x] Reject malformed nested lengths, signatures or excessive layer counts.
- [x] Compare raw Image Resources, layer Additional Layer Information,
  per-layer/channel bytes, merged-image bytes and nested raw records.
- [x] Distinguish raw-byte changes from decoded pixel changes.
- [x] Only write throwaway outputs under ignored `target/` via create-new,
  never into original PSD fixtures.
- [x] Bind no-op evidence to pinned Node **22.23.3** and
  `ag-psd 31.0.2`.
- [x] Block public PSD mutation on all results, even if some normalized
  information appears unchanged.
- [x] Machine-readable evidence and ADR 0016 produced.
- [x] M3 PSD v1 public protocol, Managed engine capabilities and v0.1
  technical `main` baseline remain unchanged.

## Validation

```bash
npm ci --ignore-scripts --no-audit --no-fund --prefix packaging/ag-psd-engine

# Pinned Node 22.23.3 is installed in ignored target/ for local testing.
node --test crates/yu-psd-spike/adapters/typescript/m4c4_nested_layer_inventory.test.cjs
NODE22=target/m4c4-node22/node_modules/node/bin/node
"$NODE22" --test crates/yu-psd-spike/adapters/typescript/m4c4_noop_fidelity.test.cjs
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c4_noop_fidelity.cjs --check

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1
git diff --check
```

- [x] Nested parser: 5 tests passed, 0 failed.
- [x] Pinned fidelity comparison: 2 tests passed, 0 failed.
- [x] 13 fixture attempts, 11 reopened, 2 explicitly refused;
  machine report deterministically reproduced.
- [x] Rust workspace `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** (local macOS).
- [x] Rustfmt `cargo fmt --all -- --check`: passed.
- [x] Clippy `cargo clippy --workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] JavaScript syntax and Git whitespace checks: passed.
- [x] Committed no-op snapshot reproducible with `--check`.

Deferred production gates: Windows/Linux replication; independently decoded
PSD layer/composite appearance; Photoshop editor round-trip; byte-preserving
opaque metadata support; and future explicit version-bound PSD output
contract. No GitHub Actions/PR needed for this research increment.
