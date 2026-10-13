# ADR 0029 — M4c-13 Filesystem Evidence Matrix and Manual Recovery Disposition

**Date:** 2026-10-13

**Decision:** accept local APFS capability evidence and a strictly read-only,
human-reviewed recovery decision workflow. **Do not authorize a PSD writer.**

**Base:** `develop@7e87e7c5f3a65b6e801a54a4e60210af34c41089`.
**Frozen v0.1 main:** `5526dc4465c4e79ad7f2757d06b3b1693642b8af`.

## 1. Scope and evidence classification

M4c-12 added a research-only, SHA-256-bound PSD transaction with
hard-link no-clobber output, file/parent directory `fsync`, intent,
receipt, process SIGKILL testing and read-only reconciliation.

This follow-up separates **actual device measurements** from
**mount observation** and **unrun platform cases**. The only
write-tested filesystem is the current macOS host's local writable
APFS Data volume, within a newly created ignored
`target/m4c12-recovery/run-*` scratch directory.
The M4c-13 capability probe independently creates a fresh
private scratch run and tests:

- Exclusive creation of a small non-PSD probe file plus file `fsync`.
- Same-filesystem hard-link creation and observed matching
  device/inode and SHA-256.
- A second hard-link attempt to the same destination is refused
  with `EEXIST`, without replacing the output.
- Actual directory `fsync` on the research scratch output parent.

The probe stores **observed syscall outcomes**, not
claims about sudden power loss or a universal API contract.

## 2. Filesystem support matrix

| Environment | Actual evidence | Decision |
|---|---|---|
| macOS 27.0 local APFS Data volume | Actual file/hard-link/no-clobber/dir-sync success in ignored scratch | **Local observation PASS only** |
| Locally mounted NTFS volume | Mount identified read-only | **SKIPPED**, no write performed |
| OrbStack NFS mount | Mount identified; no authorization to write probe files to shared volume | **SKIPPED**, no write performed |
| Windows NTFS | No separate live Windows device run | **NOT TESTED** |
| Linux ext4/XFS/Btrfs/network | No separate live Linux device run | **NOT TESTED** |
| Sudden power loss / storage crash | No destructive power-cut scenario | **NOT TESTED** |

The exact, locally observed metadata and categories are in
[M4c-13 filesystem receipt](../data/m4c13-filesystem-matrix-v1.json).
The presence of a mounted NFS or NTFS volume **never implies**
support for atomic PSD output on that filesystem.

M4c-12's simulated `EXDEV`, `ENOTSUP` and `EOPNOTSUPP`
fail-closed results remain useful unit tests; they are **not**
equivalent to a real Windows/Linux or cross-volume acceptance run.

## 3. Manual-only recovery for a missing receipt

[The M4c-13 manual recovery tool](../../crates/yu-psd-spike/adapters/typescript/m4c13_manual_recovery.cjs)
works on M4c-12's **exact restricted research run paths**.
`--inspect` invokes only the read-only transaction reconciliation
logic and produces an operator-oriented decision and checklist.
It **never** repairs a receipt, republishes, unlinks, renames
or deletes a PSD.

| Read-only observation | Decision | Action boundary |
|---|---|---|
| Valid private staged intent, no output | `HOLD_STAGED_ONLY` | Manual review; no automatic retry |
| Published byte/inode matches stage, no receipt | `HOLD_PUBLISHED_NO_VERIFIED_RECEIPT` | Preserve output, no fabricated success receipt |
| Current matching output and valid prior receipt | `OBSERVED_COMPLETED_RESEARCH` | Preserve evidence; no new write authorization |
| Source inode/hash differs from intent | `STOP_SOURCE_VERSION_CHANGED` | Stop; old version cannot be resumed |
| Output replaced, symlinked or rival-created (even same bytes, different inode) | `STOP_UNTRUSTED_OUTPUT` | Preserve target and investigate provenance |
| Stage SHA, intent, bounds, or evidence invalid | `STOP_INVALID_TRANSACTION_EVIDENCE` | No cleanup or repair without separate manual authorization |

The existing research hard-link output and stage share **one
inode**. An in-place modification through either path changes both;
reconciliation deliberately refuses a corrupted stage. A pathname
replacement of the output must also fail ownership checks, even if
the old receipt was valid.

The snapshot evaluates four M4c-12 cutpoints, **including two
different states where output exists without a complete receipt**.
Detailed operator steps are preserved in
[the machine-readable manual disposition receipt](../data/m4c13-manual-recovery-v1.json).

Human investigation of an inconclusive receipt is *not* itself
permission to create a new output. The operator must obtain
separate source version confirmation and explicit transaction
authorization for any new attempt.

## 4. Strict remaining blockers

The current proof does **not** establish:

- Physical crash or power-loss persistence and recovery on any
  platform; successful `fsync` alone cannot prove all storage
  power-failure behavior.
- Windows/Linux directory `fsync` and hard-link semantics or
  performance, or volume-specific NTFS/NFS capability.
- Atomic coupling between a source's content and publication in
  the presence of arbitrary concurrent source writers.
- Complete Photoshop metadata fidelity, including EXIF, XMP,
  Slices, `shmd/cust`, unknown tagged fields and broader PSD features.
- A transaction-safe customer-facing PSD `yu psd rename` writer.

M4c-13 adds no metadata exclusion list. It preserves the
M4c-11 decision to **not silently ignore provenance/history
or opaque Photoshop resources**.

## 5. Exit decision

- [x] One actual local macOS APFS filesystem capability experiment.
- [x] Mounted NTFS and NFS capabilities classified without writing.
- [x] Windows, Linux and power-cut evidence explicitly left missing.
- [x] Manual-only recovery disposition for staged, published/unacknowledged,
  completed, conflicting and corrupted transaction states.
- [x] Source identity/sha, hard-link device/inode, existing receipt,
  competing same-content output and hostile evidence tests.
- [ ] Broader devices/filesystems, power-cut replay, end-to-end
  metadata fidelity and separate release decision.

**M4c-13 research can be frozen on `develop`, while
`PSD_PRODUCTION_MUTATION_BLOCKED` stays enforced.**

See [acceptance](../testing/m4c13-filesystem-recovery-acceptance.md).
