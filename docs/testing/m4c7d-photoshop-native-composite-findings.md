# M4c-7d — Photoshop Native Composite and Metadata Differential Findings

- **Date:** 2026-10-13
- **Status:** Evidence captured; raw-metadata fidelity **BLOCKED**
- **Source baseline:** `develop@80af5544293a289264759cfa89ea39d28f43cf51`
- **Frozen v0.1 main:** `5526dc4465c4e79ad7f2757d06b3b1693642b8af`
- **Scope:** One exact-hash nested-group research fixture only. No production PSD writes.
- **Evidence capture:** Two independent Adobe Photoshop 2026 original-control / fixed-width group-rename saveAs experiments in isolated, ignored `target/` worktree directories. Original fixtures not overwritten.

## Verified pixel findings

Both Photoshop experiments showed identical, repeatable results:

| Comparison | Changed RGBA pixels | Interpretation |
|---|---:|---|
| `psd-tools 1.20.0` original PSD preview vs its unchanged Photoshop-saved control | 160 / 20,000 | Repeated in both independent runs |
| `psd-tools` patched PSD preview vs Photoshop-saved renamed candidate | 160 / 20,000 | Same pattern as unchanged control |
| `psd-tools` *saved* original-control vs *saved* renamed-candidate composite | 0 | Pixel-equivalent saved pair |
| **Photoshop 2026 native PNG** of original PSD vs Photoshop native PNG of its saved control | **0** | **Native rendering unchanged by save** |
| Photoshop native PNG of patched input vs Photoshop native PNG of saved candidate | 0 | Native rendering unchanged by save |
| Photoshop native PNG saved-control vs saved-candidate | 0 | Rename does not alter this fixture's native composite |
| `psd-tools` original PSD composite vs **Photoshop native PNG** from the same original | 160 | Independent parser disagrees with native render *before save* |
| `psd-tools` saved PSD composite vs Photoshop native PNG from the same saved PSD | 0 | Both renderers agree *after save* |

The 160 differing pixels all lie within the bounding rectangle **[26,25,65,97)** (0-based xyxy). The R, G and B channels differ at the same 160 pixels, alpha at **0**, and the maximum 8-bit channel difference is **11**. Both independent experiments reproduced the exact same difference pattern and count.

**Revised conclusion:** Earlier `psd-tools` source-vs-save pixel differences cannot be called Photoshop-induced native visual damage. Photoshop native PNG exports of source and save are pixel-identical on this one fixture. The difference is between the independent `psd-tools` composite of the original PSD and the native Photoshop composite; after native saving, they agree. The underlying PSD feature/compositing explanation remains unproven. This does **not** generalize beyond the exact fixture.

## Metadata and byte-level findings

Photoshop successfully saved, closed and reopened both scratch PSDs. Native group name persists as `Group 1` (untouched control) or `Group 2` (edited candidate). No other logical layer-name differences were detected. The strict raw-pair gate is nevertheless **FAILED**.

- Original/control and modified/candidate Photoshop saved file lengths are **33,256** and **33,240** bytes respectively, a **16-byte** difference. They cannot be declared identical outside name ranges.
- Within both saved pairs, compressed channel payloads and encoded merged composite are identical; several metadata blocks are not. Image Resource IDs **1060** and **1050** differ in both repeated pairs; ID **1058** differs in at least one pair. Several `shmd` layer tagged records differ in addition to the intended `luni` group-name record.
- Comparing **the same unchanged control saved in two separate Photoshop experiments**, IDs **1060** and **1058** and multiple `shmd` hashes change, while the saved composite byte hash is unchanged. The edited candidate repeats the same cross-run variability. This confirms Photoshop itself produces some variable metadata; it does **not** prove ID **1050**, all `shmd` differences, or the saved-pair size discrepancy are harmless.
- Photoshop-saved files contain a small zero-padding tail in Additional Layer Information. The research scanner now recognizes such framing in the isolated worktree while continuing to mark it unaudited and reject malformed nonzero tails. This parser extension has **not yet been pushed**.

## Gate decision

- **M4c-7c native rename persistence:** verified for **one** eligible English group fixture.
- **M4c-7d native visual composite comparison:** verified *unchanged* for the same exact group fixture and two repeated paired runs.
- **M4c-7d comprehensive opaque data/resource preservation:** **NOT VERIFIED / BLOCKED**.
- **M4c-7e product release approval:** **BLOCKED**. No `yu psd rename`, broad PSD save or arbitrary PSD/PSB mutation is authorized.

## Engineering handoff

The research session also authored a read-only pixel audit, Photoshop native PNG export probe, native-vs-independent renderer comparison and a two-run metadata report generator, with local scratch experiment outputs under `target/`. **The local WebCodex tunnel disconnected before those source changes and the full machine-readable report could be finalized, regression-tested, committed and pushed.** This note is a durable, manually reviewed findings record, **not** evidence that the new analysis code has merged into `develop`.

After the local development connection recovers: verify the existing worktree `research/m4c7d-psd-photoshop-fidelity-differential`, rerun the M4c-7d scripts, review the generated evidence, run Rust/Node/Python checks, then merge/push the actual research code. Do not rerun Photoshop saves against the user's documents; only use isolated scratch runs.
