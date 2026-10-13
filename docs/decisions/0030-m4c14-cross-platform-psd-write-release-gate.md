# ADR 0030 — M4c-14 Device Evidence and PSD Mutation Release Gate

**Date:** 2026-10-13

**Status:** Research/device handoff ACCEPTED; public PSD writing **BLOCKED**.

**Baseline:** `develop@c6f80b4beb6b51ec64904405942b43c5ec312426`.
**Protected v0.1 `main`:** `5526dc4465c4e79ad7f2757d06b3b1693642b8af`.

## 1. Device and filesystem evidence actually available

The WebCodex Runner fleet was checked before development: **one online
Runner**, on the existing Apple Silicon macOS 27.0 device. There is no
connected independent Windows or Linux native Runner capable of
providing a live M4c-14 receipt during this stage.

The source-bound Node.js **22.23.3** M4c-14 device probe actually ran on
this Mac, inside a new private directory under ignored `target/`. It
observed successful exclusive file creation, file `fsync`, hard-link
creation, source/destination inode equality, no-overwrite `EEXIST`,
and directory `fsync` syscall return. These are **local syscall
observations**. The script records actual OS, architecture, runtime,
kernel release, Git checkout SHA, probe source SHA-256, opaque native
`statfs` fields, and explicit non-authorization flags.

It does **not** infer a filesystem brand from a numeric `statfs` type
alone, authenticate itself as a real device, claim power-loss
durability or read/write any customer PSD.

| Required environment | M4c-14 status | Interpretation |
|---|---|---|
| macOS 27.0, arm64, local tested volume | **OBSERVED, REVIEW REQUIRED** | Only research-only system-call evidence |
| Windows native NTFS | **PENDING_REAL_DEVICE_RECEIPT** | No live Windows device receipt, no pass |
| Linux native ext4/XFS/Btrfs (or relevant target filesystem) | **PENDING_REAL_DEVICE_RECEIPT** | No live Linux device receipt, no pass |
| macOS mounted NTFS | **NOT WRITE TESTED** | Previously found read-only |
| OrbStack NFS | **NOT WRITE TESTED** | Mounted, no authorization to modify |
| Real power loss / recovery | **NOT TESTED** | SIGKILL != power removal |

Do not promote historical GitHub CI green checks, a synthetic unit-test
receipt, a macOS VM, an emulation run, or a simulator into a **real
Windows/Linux device gate**. A physical device or independently
qualified native environment must execute the fixed probe and
report its actual filesystem behavior.

## 2. Portable device handoff contract

The
[`m4c14_device_probe.cjs`](../../crates/yu-psd-spike/adapters/typescript/m4c14_device_probe.cjs)
uses only pinned Node.js 22.23.3 builtins and ordinary local filesystem
operations. Its fixed contract baseline is the parent M4c-13
`c6f80b4`; the actual checkout Git SHA is recorded **separately**,
so later Windows/Linux executions on the committed M4c-14 source can
be reviewed. The probe script SHA-256 must equal the accepted M4c-14
implementation hash, and the operator must verify that the checkout
matches the intended source revision.

On macOS the developer's bounded research snapshot is
[`m4c14-local-device-v1.json`](../data/m4c14-local-device-v1.json).
On Windows/Linux, `--write` creates a **unique** self-reported JSON
receipt under ignored `target/m4c14-device-probe/`; it does not
overwrite that macOS evidence file. `--check [RECEIPT_FILE]` is
offline and makes no filesystem probes.

A self-reported JSON file is **not a secure device attestation**.
The [release assessor](../../crates/yu-psd-spike/adapters/typescript/m4c14_release_gate.cjs)
requires the exact source/implementation contract and full test list,
but flags every supplied receipt as **independent review required**.
Even in unit tests containing synthetic Windows/Linux-shaped receipts,
the release gate **cannot** become accepted. Such test objects are
never stored as real device evidence.

## 3. Explicit PSD mutation release gates

The
[`M4c-14 release decision`](../data/m4c14-psd-write-release-decision-v1.json)
freezes **seven unresolved gates**:

1. **Windows native receipt:** actual source-bound device run, OS,
   filesystem semantics and independent reviewer approval.
2. **Linux native receipt:** separately verified native filesystem
   run and reviewer approval.
3. **Cross-filesystem agreement:** link/no-clobber/sync behavior,
   unsupported filesystem errors, removable/network policy and
   independent supported-filesystem scope.
4. **Crash/power-loss durability:** actual supported storage recovery
   qualification; process SIGKILL does **not** count.
5. **PSD metadata and editor fidelity:** arbitrary XMP, EXIF,
   `shmd/cust`, Slices and unknown blocks, and Photoshop fidelity
   across the admitted PSD feature set.
6. **Production source concurrency and recovery:** race-free source
   binding, no-clobber publication, durable receipt and explicit
   handling of output-present/receipt-missing conditions.
7. **Manual production release decision:** separate review and
   authorization, not inferred from research code or passing CI.

The existing M4c-7e, M4c-11, M4c-12 and M4c-13 frozen denials
are inputs to the assessor and cannot be silently overwritten by a
new OS probe.

## 4. Gate outcome and separation from YuTool v0.1

**M4c-14 is a successful research-handoff increment, not a successful
PSD production release gate.**

- Mac local observation: present, **not independently attested**.
- Windows/Linux device results: **PENDING**, no acceptance claims.
- Power-loss durability: **BLOCKED**.
- Full PSD metadata preservation: **BLOCKED**.
- Production PSD output transaction: **BLOCKED**.
- Manual release authorization: **BLOCKED**.

The final machine decision is
`PSD_PRODUCTION_MUTATION_BLOCKED`. No `yu psd rename` command,
general PSD writer, source overwrite or new engine capability is
registered. No tag, Release-only PR or GitHub Actions workflow was
started.

This restriction is **specific to PSD modification**. The existing
read-only PSD and image-processing v0.1 release candidate on `main`
remains a separate decision with its own redistribution, signing,
packaging and manual release gates. PSD mutation research must not
retroactively force a change to that frozen baseline.

See [M4c-14 acceptance and operator handoff](../testing/m4c14-cross-platform-release-acceptance.md).
