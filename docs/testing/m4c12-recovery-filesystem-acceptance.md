# M4c-12 — Crash/Filesystem Research Gate Acceptance

**Base:** `develop@d41650622ce8b9ee9e7900439b86e67e5cdb1140`

**Frozen main:** `5526dc4465c4e79ad7f2757d06b3b1693642b8af`

**Status:** Research accepted; `PSD_PRODUCTION_MUTATION_BLOCKED`.

## Safeguards

- [x] Fixed exact known PSD SHA-256 and deterministic candidate SHA.
- [x] Private 0700 scratch run, per-transaction private stage and
      versioned intent; exclusive writes and staged file `fsync`.
- [x] Source device/inode/size/hash recorded and rechecked before
      atomic hard-link publication.
- [x] Actual directory `fsync` attempted after output link;
      failure is **not** a success receipt.
- [x] Research output hash and source revision checked after link;
      private receipt synced separately.
- [x] Recovery reads existing intent, stage, output and receipt
      without automatically deleting or republishing anything.
- [x] Distinguishes staged-only, linked-without-receipt,
      linked-and-parent-synced-without-receipt, and complete states.
- [x] Two separate **SIGKILL test-child only** runs show recovery
      after forced process exit at different cutpoints.
- [x] Real output and stage device/inode matching required;
      identical bytes from a rival writer never prove ownership.
- [x] Broken/tampered source, intent, private stage, target,
      symlink or path scope is rejected.
- [x] Injected `EXDEV`, `ENOTSUP`, `EOPNOTSUPP`, destination
      race and parent-directory `fsync` failure are fail-closed.
- [x] One local macOS filesystem `statfs`, hard-link and directory
      `fsync` capability observed.
- [x] No general PSD mutation engine, write command, existing file
      overwrite, actual power-cut experiment or universal metadata
      waiver.

## Reproduce

```bash
NODE22=target/m4c12-node22/node_modules/node/bin/node

# Explicit new ignored research runs; do not run on customer PSDs:
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c12_recovery_fs_gate.cjs --write

# Offline snapshot check; does not publish, delete or repair any PSD:
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c12_recovery_fs_gate.cjs --check

"$NODE22" --test \
  crates/yu-psd-spike/adapters/typescript/m4c12_recovery_fs_gate.test.cjs
```

- [x] M4c-12 recovery/negative suite: **14 passed, 0 failed**.
- [x] M4c-3 through M4c-12 pinned Node.js 22.23.3 PSD research suite:
      **97 passed, 0 failed**.
- [x] Full Rust workspace `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** on local macOS.
- [x] `cargo clippy --workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] Rustfmt, JavaScript syntax, offline evidence and Git whitespace
      checks: passed.

## Unfinished production requirements

- [ ] Physical power loss or storage failure simulation on
      supported OS/filesystems and safe durable-recovery proof.
- [ ] Portable directory sync behavior, network and removable disks.
- [ ] Full source-version concurrency contract and verified
      postpublication recoverable acknowledgement.
- [ ] Sufficient metadata and Photoshop editor fidelity across
      real PSD features.
- [ ] Explicit release policy approval before any public PSD write.

Machine evidence: [M4c-12 recovery experiments](../data/m4c12-recovery-filesystem-v1.json).
Architecture rationale: [ADR 0028](../decisions/0028-m4c12-recovery-filesystem-metadata.md).
