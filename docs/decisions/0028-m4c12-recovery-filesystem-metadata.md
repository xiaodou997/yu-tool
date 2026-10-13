# ADR 0028 — M4c-12 PSD Research Output Restart and Filesystem Gates

**Date:** 2026-10-13

**Decision:** accept *research-only* interrupted-output state classification and
same-filesystem capability evidence. **PSD production mutation remains BLOCKED.**

**Base:** `develop@d41650622ce8b9ee9e7900439b86e67e5cdb1140`

**Frozen v0.1 `main`:** `5526dc4465c4e79ad7f2757d06b3b1693642b8af`

## 1. Model and limits

M4c-11 confirmed no-clobber, SHA-256-bound, same-filesystem hard-link
output for the **single known** M4c-8 normal RGB pixel-layer
`Layer 1 → Layer A` byte patch, with no user PSD write capability.
M4c-12 studies what can be *observed* when that process ends partway
through publication, and how to **refuse** unsupported filesystem
conditions or unknown metadata.

This is not a new generic output function. Its only allowed input is a
private, regular `pinned-source.psd` copy of the exact known
SHA-256 source. Its only destination is an absent
`renamed-research.psd` within a new ignored
`target/m4c12-recovery/run-*` directory.

The versioned private intent records source SHA-256, filesystem device,
inode and size, expected candidate SHA-256 and the fixed output name.
The candidate and intent use exclusive creation, private mode, file
`fsync`, followed by directory `fsync` of the intent and its parent;
the research run and source filename are also directory-synced at
initial creation.
The final output is created with a same-volume hard link: **existing
target files are never replaced**. After linking, the experiment tries
to `fsync` the output parent directory, then validates the candidate
and source before creating and syncing a private receipt.

**Important:** these barriers improve the model's observability but do
not prove recovery from power failure, storage-controller failure or
unexpected filesystem semantics. The original source can still change
after a final prepublication check in concurrent workflows.

## 2. Interruption and recovery observations

Four deterministic interruption cutpoints and **two real SIGKILL
terminations of disposable child processes** were tested on the local
macOS research filesystem:

| Checkpoint | Recoverable observation | Automatically republish? |
|---|---|---|
| After intent/stage, before link | Stage exists; destination absent | **No** |
| After hard link, before output directory `fsync` | Correct output bytes visible; receipt absent | **No** |
| After output directory `fsync`, before receipt | Correct output bytes visible; receipt absent | **No** |
| Normal completion | Receipt and matching candidate observed | **No** |

A recovery probe **never writes, deletes or renames** any source,
published destination or transaction artifact. It validates the
frozen intent, staged SHA-256, source device/inode/hash, output SHA-256,
hard-link **device and inode identity**, and receipt (if present).

Do not infer that a byte-identical output belongs to this transaction:
another publisher can create the exact same bytes in a different
inode. Conversely, publishing a hard link means staged and published
paths initially **share the same inode**; an in-place modification
through either alias corrupts both and must be reported as ambiguous,
not silently repaired.

An output replaced after publication remains in place. Even if an old
valid receipt exists, present bytes and inode must be checked before
any claim about the current output.

SIGKILL experiments terminate **only the purpose-spawned test
subprocess**, not Photoshop, the main application or the user's other
processes. They demonstrate process-exit state recognition, not actual
power-loss durability.

## 3. Filesystem compatibility

The local host's existing mounted filesystem was observed with
`statfs` and an actual directory `fsync`; exact values are in
the [machine evidence](../data/m4c12-recovery-filesystem-v1.json).
This is a **one-filesystem observation** and does not establish
support for every macOS volume, SMB/NFS share, Windows NTFS, FAT,
exFAT, network drive or Linux filesystem.

Injected `EXDEV`, `ENOTSUP` and `EOPNOTSUPP` hard-link failures
cause **explicit refusal with no copy/rename fallback**.
A simulated output directory `fsync` failure leaves any already
linked output untouched but refuses a durability success receipt.
Directory symlinks, source symlinks, source inode replacement,
changed source contents, existing/symlink output, unrecognized
transaction intent and source-stage tampering also fail closed.

**Unresolved:** parent directory creation durability, cross-platform
directory-sync guarantees, storage crash tests, stable source binding
across concurrent edits, staged artifact cleanup policies, and
publication state after process crashes before the intent has
been durably recorded.

## 4. Metadata retention policy

The M4c-11 Photoshop-saved metadata examination is retained as a
separate gate. In the exact two saved multilingual control/rename
pairs, EXIF `DateTime`, XMP provenance/history and preserved
`shmd` data can be independently reported.

The M4c-12 research byte patch accepts **one exact candidate SHA-256**
and leaves all source bytes outside the two fixed-width name ranges
unchanged *before Photoshop save*. No metadata is stripped or
automatically normalized. The policy grants **zero** EXIF, XMP,
Slices or `shmd` metadata exclusions and no general PSD semantic
equivalence certificate.

## Decision and next gate

- [x] Four cutpoint simulations and two real disposable-process
  terminations recovered in strict **read-only observation mode**.
- [x] Source identity and exact SHA-256 binding, hard-link identity
  and receipt validation.
- [x] Existing/racing output and hostile modified candidate refused.
- [x] Unsupported filesystem hard-link and directory sync simulated
  failures **do not** silently fall back.
- [x] One local directory-sync capability observed without
  claiming platform-wide durability.
- [ ] Real power-loss recovery / multiple filesystems and operating
  systems, postlink source-concurrency contract, robust cleanup
  and transaction conflict resolution.
- [ ] Broad metadata equivalence, platform-specific Photoshop
  fidelity and separate public writer release decision.

**Conclusion:** M4c-12 research gate can freeze. Public
`yu psd rename`, in-place PSD saving and generalized output
publication remain **BLOCKED**.

See [M4c-12 acceptance](../testing/m4c12-recovery-filesystem-acceptance.md).
