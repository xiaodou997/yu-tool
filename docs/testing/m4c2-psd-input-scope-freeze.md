# M4c-2: PSD mutation input scope and contract acceptance

**Branch:** `research/m4c2-psd-mutation-contract-freeze`

**Base:** `develop@ff61d9aa975e9decc5f19bd555e0d916e529060d`

**Freeze decision:** [ADR 0014](../decisions/0014-psd-mutation-input-scope-freeze.md)
**Policy:** [Machine contract v1](../data/psd-safe-mutation-scope-m4c2-v1.json)

## Design gate

- [x] Research-only policy freezes four exact fixture hash identities;
      **no general user document admission or public write**.
- [x] Accepted profiles pinned to Node 22.23.3 and ag-psd 31.0.2.
- [x] RGB/8-bit and header/format/geometry/source-size limits.
- [x] Canonical layer ID, previous name, exact profile target and new-name
      validation, including group and duplicate-name fixtures.
- [x] Explicit source SHA-256 version precondition.
- [x] Separate new-file-only output path, no source aliases, no-clobber
      existing files, symlinks or directories.
- [x] Fail-closed errors for arbitrary inputs, altered fixture bytes,
      unsupported operation and invalid proposal state.
- [x] Research script only: JSON stdin → JSON plan/error stdout, no output
      file, staging file, sidecar, Photoshop save or Managed engine mutation.
- [x] Explicit non-authorization in every admitted plan.
- [x] Explicit future write gates: raw unsupported-block scanner, linked
      asset preservation, independent Photoshop/editor fidelity, staged
      verification and M4b-style post-write receipt.
- [x] Frozen v0.1 `main` and existing PSD v1 protocol untouched.

## Local verification

From the repository root, use the pinned Managed build dependencies and
an **exact Node.js 22.23.3 executable**. The temporary Node toolchain and
npm dependencies belong in ignored local build directories, not in Git.

```bash
npm ci --ignore-scripts --no-audit --no-fund --prefix packaging/ag-psd-engine
NODE22=/path/to/node-v22.23.3
"$NODE22" --test crates/yu-psd-spike/adapters/typescript/m4c2_safe_mutation_preflight.test.cjs
```

The preflight itself consumes one JSON request from stdin. See ADR 0014 for
the exact required fields. It **never** saves PSD/PSB bytes.

- [x] Pinned Node 22.23.3 preflight: **9 tests passed, 0 failed**.
- [x] Existing Rust workspace: `cargo test --workspace --locked -- --test-threads=1`;
      **174 passed, 0 failed** (local macOS).
- [x] Rustfmt `cargo fmt --all -- --check` (local macOS).
- [x] Clippy `cargo clippy --workspace --all-targets --locked -- -D warnings`
      (local macOS).
- [x] JavaScript syntax checks and `git diff --check`.

**First parallel Rust run was not green.** It produced 173 passed / 1 failed
in an unchanged existing `yu-runtime-psd` nonblocking-pipe timing test:
`empty_nonblocking_pipe_is_not_eof_until_peer_closes`, with OS
`WouldBlock` during an immediate read. The targeted test passed in
isolation (1/1), and the full serial regression passed (174/174). No
unrelated production/process-test change was added to the PSD scope
freeze. Do not misreport the first run as passed, and retain the timing
flakiness for a separate reliability follow-up.

## What this freeze does not prove

- [ ] No verified general-purpose raw PSD block scanner (unknown records
      must remain blocked).
- [ ] No independent Photoshop app-level visual/fidelity acceptance.
- [ ] No independent Linux/Windows replay of these research-only contracts.
- [ ] No public PSD rename, visibility, opacity or general save command.

Those items are **intentionally deferred blockers for production writes**,
not reasons to widen the research-only admission in M4c-2.
