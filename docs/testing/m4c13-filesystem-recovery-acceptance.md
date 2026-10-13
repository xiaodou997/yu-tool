# M4c-13 — Filesystem Matrix and Manual Recovery Acceptance

**Base:** `develop@7e87e7c5f3a65b6e801a54a4e60210af34c41089`

**Protected main:** `5526dc4465c4e79ad7f2757d06b3b1693642b8af`

**Gate:** Research evidence accepted; PSD production write **BLOCKED**.

## Real host checks

- [x] Local host macOS 27.0 on Apple Silicon.
- [x] Actual APFS scratch output supports exclusive file write,
      hard-link creation, matching inode, `EEXIST` no-clobber
      and file/directory `fsync` syscall returns.
- [x] NTFS mount identified as **read-only**; no write performed.
- [x] OrbStack NFS mount identified; no remote filesystem write
      performed or assumed authorized.
- [x] No Windows or Linux test environment invented and no
      power-failure durability claim made.

## Recovery and operator safety

- [x] Four deterministic M4c-12 transaction cutpoints classified,
      including two published-without-receipt states.
- [x] Human decision includes explicit instructions to preserve
      existing output and avoid automatic retry.
- [x] Published output SHA-256 and inode match private stage
      before claiming provenance.
- [x] Competing byte-identical output with different inode
      **not** credited as the transaction's link.
- [x] Changed source SHA or inode, symlinked output, replaced
      output, tampered stage and intent fail closed.
- [x] Old receipt is not enough to accept currently changed output.
- [x] `--inspect` is strictly read-only; `--write` creates only
      new ignored scratch research transactions.
- [x] Public PSD writer, general file edits, auto-repair, deletion
      and silent metadata waivers remain prohibited.

## Reproduction

```bash
NODE22=target/m4c13-node22/node_modules/node/bin/node

# Only ignored local research scratch files are created.
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c13_filesystem_matrix.cjs --write
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c13_manual_recovery.cjs --write

# Offline snapshot check; neither touches Photoshop or customer PSDs.
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c13_filesystem_matrix.cjs --check
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c13_manual_recovery.cjs --check

# Read-only operator inspection of an existing M4c-12 research run:
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c13_manual_recovery.cjs \
  --inspect target/m4c12-recovery/run-...

"$NODE22" --test \
  crates/yu-psd-spike/adapters/typescript/m4c13_filesystem_manual.test.cjs
```

- [x] M4c-13 scoped negative/positive suite: **12 passed, 0 failed**.
- [x] Complete M4c-3 through M4c-13 pinned Node.js 22.23.3 suite:
      **109 passed, 0 failed**.
- [x] Rust workspace `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** on local macOS.
- [x] `cargo clippy --workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] Rustfmt, JavaScript source/test syntax, both machine receipts'
      offline checks and Git whitespace checks: passed.

## Remaining release-only/manual gates

- [ ] Real Windows and Linux filesystem and device receipts.
- [ ] Approved removable/network filesystem tests and
      non-destructive crash/power-loss environment.
- [ ] Full PSD feature + metadata semantic preservation.
- [ ] Explicit production writer release policy and implementation.

Supporting artifacts:
[ADR 0029](../decisions/0029-m4c13-filesystem-matrix-manual-recovery.md),
[filesystem matrix](../data/m4c13-filesystem-matrix-v1.json),
[manual recovery evidence](../data/m4c13-manual-recovery-v1.json).
