# ADR 0019 — M4c-7 PSD Safe Rename Scope Freeze / Native Save Gate

- **Status:** M4c-7a accepted; M4c-7b/c native save-and-reopen **BLOCKED**
- **Date:** 2026-10-11
- **Base:** `develop@d197334c88d123b4a2a68600ea784c543b41755b`
- **Protected:** `main@5526dc4465c4e79ad7f2757d06b3b1693642b8af`

## Decision

Freeze a **research-only** safe layer-name allow-list. Do not interpret
M4c-5's bitwise preservation or M4c-6's independent reader successes
as authorization to write arbitrary PSD files. M4c-7a applies four
separate, fail-closed evidence gates: (1) exact known source hash;
(2) canonical ID, raw layer index and name alignment; (3) Adobe Photoshop
native **non-Background** semantics; (4) synchronizable matching ASCII
Unicode `luni` and legacy Pascal name representations of exactly the
same length. Only fixed UTF-16 code-unit-count edits are studied.

The rule is recorded in
[`psd-rename-scope-m4c7-v1.json`](../data/psd-rename-scope-m4c7-v1.json)
and verified by
[`m4c7_scope_freeze.cjs`](../../crates/yu-psd-spike/adapters/typescript/m4c7_scope_freeze.cjs).

| Previously reviewed case | Research gate | Reason |
|---|---|---|
| Simple PSD `Фон→Дом` | **BLOCKED** | Photoshop reports localized native Background; divergent legacy data |
| Simple PSB `Фон→Дом` | **BLOCKED** | Same special Background semantics |
| Nested group `Group 1→Group 2` | Research save/reopen candidate | Both Unicode and ASCII legacy name agree and can change together |
| Duplicate-name pixel `X→Y` | **BLOCKED** | Pre-existing Unicode-vs-Pascal discrepancy |

The fact that a duplicate-name pixel layer was previously shown correctly
by Photoshop does **not** make its alternate old name representation
safe. A new ordinary pixel fixture with matching legacy and Unicode
names must pass separate admission tests before this class is opened.
No generalized 'pixel' or 'group' write capability is authorized.

## M4c-7b/c controlled Photoshop save design

[`m4c7_photoshop_save_probe.cjs`](../../crates/yu-psd-spike/adapters/typescript/m4c7_photoshop_save_probe.cjs)
contains a manually invoked native Photoshop 2026 experiment:

1. Verify Photoshop's scripting channel responds before creating any
   files or attempting a save. If even a document-count query cannot
   respond, **fail closed** with a structured blocked receipt.
2. Place two independent inputs inside a fresh ignored `target/` run
   directory: the *unmodified original fixture copy* and its
   fixed-width, two-name-synchronized patched copy.
3. For each input, open in Photoshop with dialogs disabled, `saveAs`
   to a distinct, initially absent output **in the run directory**;
   close without saving the opened input. Do not use `save` or
   replace any fixture, customer file or old output.
4. Reopen and inspect the saved copies' native layer names. Cross-check
   with `psd-tools` 1.20.0 for logical layers and decoded RGBA preview
   hashes, plus source-vs-save raw metadata differences.
5. Compare *original-save control* vs *patched-save candidate* to
   distinguish Photoshop's normal reserialization from damage specific
   to the proposed rename. Rehash both experiment inputs and original
   committed fixture afterward.

The saved files, if any, are **experimental only**. Even a green research
run does not establish Photoshop's complete masks/effects/ICC/Smart Object
fidelity, general crash durability, or product release permission.

## Actual observed blocker

On the current macOS device Photoshop 2026 was installed and running, but
the AppleScript channel was **unresponsive**. The first read-only
`osascript` command timed out after 90 seconds, and a separate document
count query also timed out. Therefore no matched control/candidate native
save-and-reopen receipt was obtained. The script now performs a bounded
12-second health check and returns a **blocked native application**
result rather than repeatedly waiting or claiming success.

The [native evidence](../data/m4c7-photoshop-control-v1.json) records
`paired_research_save_reopen_passed=0`, and specifically indicates
that **no control or candidate was saved during this gated run**.
Do not force-kill Photoshop, silently change user permissions, or trigger
unattended native saves merely to satisfy a green check.

## M4c-7 exit criteria

- **7a scope freeze:** complete, default deny, research candidate 1/4.
- **7b dual-name ordinary-layer validation:** only the ASCII group
  research fixture qualifies. More normal pixels, non-ASCII and
  duplicate/legacy cases require independent controlled fixtures.
- **7c Photoshop paired save/reopen:** **BLOCKED** until native scripting
  responds and the experiment runs completely.
- **7d raw/pixel/metadata review:** comparator implemented for the paired
  experiment, but **no native save artifacts were obtained in this run**;
  full independent editorial fidelity still outstanding.
- **7e decision:** no promotion or public mutation capability.

Once Photoshop scripting becomes responsive, run the paired experiment
again with the exact isolated workspace, retain the successful receipts
and classify resource/channel/preview differences. Do not change
`main`, advertise `yu psd layer rename`, or infer permission from
a future green research receipt without a separate accepted release gate.
