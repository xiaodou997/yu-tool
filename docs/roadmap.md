# Roadmap

This roadmap intentionally prioritizes proving YuTool's runtime model before expanding into many tool categories.

Dates are not committed here. Milestones are capability-based.

## M0 — Documentation bootstrap

**Goal:** freeze the product language and architecture boundaries before implementation.

- [x] Define YuTool / `yu` naming
- [x] Define project vision
- [x] Define capability vs engine model
- [x] Define Built-in / Managed / System engine classes
- [x] Draft CLI contract
- [x] Draft capability matrix
- [x] Define agent usage rules
- [x] Record initial engine-strategy ADR
- [x] Rename repository from `img-cli` to `yu-tool`
- [ ] Set repository description/topics
- [ ] Choose license

Exit criteria:

- contributors and coding agents can explain what YuTool is and is not;
- v0.1 command shape is documented;
- optional engines are not accidentally treated as hard dependencies.

## M1 — Rust core and first built-in capability

**Goal:** prove the smallest useful YuTool runtime without external engines.

Progress:

- [x] Rust workspace bootstrap
- [x] `yu` CLI executable
- [x] shared core result/error types
- [x] capability registry
- [x] engine registry
- [x] engine resolver
- [x] `yu doctor`
- [x] `yu capabilities`
- [x] `yu engine list`
- [x] built-in raster engine (`raster-rs`)
- [x] initial `image.info`
- [x] initial `image.resize`
- [x] JSON output contract
- [x] integration tests

The first built-in raster implementation uses the Rust `image` crate with a deliberately small default format set: PNG, JPEG, and WebP.

Performance-specialized resizing (for example a future `fast_image_resize` integration) is intentionally deferred until the public resize semantics are proven.

Exit criteria:

```bash
yu doctor
yu capabilities --json
yu image info input.png --json
yu image resize input.png --width 1024 -o output.png --json
```

work on supported platforms without ImageMagick, Python, Node, or another optional engine.

### M1 Freeze

M1 is frozen once PR #3 lands and the freeze receipt in `docs/milestones/m1-freeze.md` records the accepted CLI/protocol baseline and green cross-platform CI.

## M2 — Engine Manager foundation

**Goal:** prove optional engine lifecycle management.

Progress:

- [x] engine manifest v1 model;
- [x] platform/architecture matching foundation;
- [x] managed-engine storage layout;
- [x] download staging;
- [x] checksum/integrity verification;
- [x] safe archive extraction;
- [x] atomic activation;
- [x] installed version discovery;
- [x] active/current version state;
- [x] managed version removal;
- [x] inter-process engine mutation lock;
- [x] public managed-engine CLI;
- [x] system executable discovery foundation;
- [x] system engine version probing (ImageMagick / FFmpeg / ExifTool);
- [x] unified Built-in / Managed / System inventory;
- [x] `yu engine info`;
- [x] `yu doctor` unified inventory integration;
- [x] managed engine state diagnostics.

CLI target:

```bash
yu engine list
yu engine install --manifest <file>
yu engine versions <engine>
yu engine activate <engine> <version>
yu engine deactivate <engine>
yu engine remove <engine> <version>
yu doctor
```

Important constraint:

YuTool should not silently invoke Homebrew, apt, winget, sudo, or administrator elevation as part of an unrelated operation.

Exit criteria:

- a managed engine can be installed, verified, activated, listed, used, and removed;
- a compatible system engine can be discovered without YuTool taking ownership of it.

### M2 Freeze

M2 functional behavior is frozen at `d95ccae8cdaefd4ea63a35d39d6ec803748036e8`. PR #9 records the accepted Engine Manager, lifecycle, unified inventory, CLI, security, and schema-v1 compatibility baseline in `docs/milestones/m2-freeze.md`.

M3 may extend YuTool with PSD capabilities and new engines, but should not casually break the M2 lifecycle or inventory contracts.

## M3 — PSD strategy and runtime

**Goal:** select a practical PSD/PSB strategy based on fixtures, then prove a bounded Managed runtime and selected-layer RGBA8 PNG export.

Progress:

- [x] engine-neutral PSD spike harness;
- [x] fixture corpus schema/provenance rules;
- [x] initial synthetic malformed fixture;
- [x] PSD/PSB fixture corpus v1 (pixel layers, groups, duplicate names, text, masks);
- [x] pinned third-party fixture provenance/license receipts;
- [x] Rust / psd-tools / TypeScript candidate adapter skeletons;
- [x] machine-readable per-candidate report model;
- [x] representative redistributable PSD/PSB benchmark corpus;
- [x] first Rust-native candidate integration (rawpsd 0.2.2);
- [x] psd-tools 1.20.0 reference candidate integration (Python 3.12);
- [x] TypeScript/Node candidate integration (ag-psd 31.0.2);
- [x] candidate comparison report v1 (conformance, distribution, maintenance evidence);
- [x] controlled performance/memory benchmark report v2;
- [x] PSD engine strategy decision (ADR 0006: ag-psd Managed primary, psd-tools reference/compatibility);
- [x] PSD capability contract v1;
- [x] External Engine Protocol v1;
- [x] Managed ag-psd package prototype (Node 22.23.3 + ag-psd 31.0.2; Linux x64 / macOS arm64 / Windows x64);
- [x] runtime wiring for inspect / tree / layer list / layer info (PR #22; bounded Protocol v1, active-version capability checks, public CLI);
- [x] 8-bit RGB layer bitmap export (PR #23; RGBA8 PNG, validated private staging, atomic no-clobber publication);
- [x] M3 implementation freeze (PR #24; functional baseline `c793279e3c131cae85adf73571590ab022b2ed05`, exact package/evidence snapshots and regression guards).

Build a representative fixture corpus covering, where legally distributable:

- simple pixel layers;
- nested groups;
- duplicate layer names;
- text layers;
- masks;
- blend modes;
- effects;
- Smart Object metadata;
- higher bit depth where applicable;
- PSD and PSB;
- malformed/edge-case inputs.

Evaluate candidate implementations across:

- parse success;
- layer-tree fidelity;
- metadata coverage;
- layer export;
- rendering fidelity;
- PSD vs PSB;
- round-trip behavior where writing is supported;
- performance;
- memory usage;
- platform/distribution cost;
- maintenance risk.

Candidate categories include:

- Rust-native PSD implementations;
- psd-tools;
- TypeScript/Node PSD implementations where useful;
- other mature implementations discovered during the spike.

Deliverables:

- benchmark/conformance report;
- recommended built-in PSD engine, if one is mature enough;
- recommended compatibility/managed engine, if useful;
- explicit unsupported/partial capability list.

Strategy exit criteria are now satisfied by ADR 0006 and the M3 evidence chain.

The accepted implementation includes:

```bash
yu psd inspect design.psd --json
yu psd tree design.psd --json
yu psd layer list design.psd --json
yu psd layer export design.psd --id <id> -o layer.png --json
```

using the selected Managed ag-psd engine strategy.

### M3 Freeze

M3 functional behavior is frozen at `c793279e3c131cae85adf73571590ab022b2ed05` (PR #23 squash merge). [Freeze receipt](milestones/m3-freeze.md), [machine-readable evidence](data/m3-implementation-freeze-v1.json) and [acceptance checklist](testing/pr24-m3-freeze.md) distinguish implemented behavior from release readiness. Four read-only commands plus partial 8-bit RGB stored-layer export are accepted; rendering, non-RGB/high-bit export and mutation remain outside the baseline.

Before public distribution, separately address persistent artifacts/trusted HTTPS manifests, license decisions, packaged CLI acceptance, dependency-lock/reproducibility policy and platform signing/notarization. This is a release-readiness recommendation, not a redefinition of the M4/M5 capability milestones or permission to expand them automatically.

### v0.1 release preparation (PR #25)

Engineering preparation adds verified offline `--archive` installation, quarantine diagnostics/repeated lifecycle acceptance, locked Rust/npm dependencies and source-qualified CLI developer candidates tested after extraction. See [readiness and unaccepted gates](releasing/v0.1-readiness.md) and [test checklist](testing/pr25-release-readiness.md). This does not authorize public publishing or mark Windows root cause, licensing, signatures, minimum OS support or durable hosting as accepted. M3 historical evidence remains immutable.

### Windows lifecycle investigation (PR #26)

Separate diagnostics/reproduction work records process-exit versus pipe-EOF phases and output byte counts, adds controlled CLI cases including Windows directory occupancy, and retains finite repeated extracted-candidate runs with first-failure logs. See [investigation boundaries](reliability/windows-lifecycle.md) and [acceptance checklist](testing/pr26-windows-lifecycle.md). Native process/Job cleanup hardening and historical root-cause closure remain unimplemented; candidate promotion is prohibited.

### Explicit process cleanup result propagation (PR #28)

First lifecycle-hardening increment: explicitly settle owned cleanup, retain primary and cleanup errors, and reject success/artifact publication on cleanup failure. Controlled tests reproduce swallowed worker failure before the change. [Scope and deferred work](reliability/explicit-process-cleanup.md) and [acceptance](testing/pr28-explicit-process-cleanup.md) separate this implemented error path from creation-time Job assignment, bounded I/O cancellation, enforced whole-Job completion and historical OS5/timeout attribution. Issue #27 stays open; no release promotion.

### Independent file-resource attribution (PR #30)

This main-derived diagnostics increment refines verified batch resource users into bounded, timestamped single-file observations, with separate real-Windows calibration. It is not #29 startup integration, a directory-blocker proof, bounded runtime cleanup or release approval. See [scope](reliability/windows-resource-attribution.md) and [acceptance](testing/pr30-windows-resource-attribution.md). Historical OS5/timeout evidence and issue #27 remain open; #29's failed independent gate is not waived.

### Bounded process waiting and cancellable I/O (PR #31)

Independent runtime change on main after #30: nonblocking pipe pump, stop-and-close cancellation, bounded direct-child and Windows Job settlement, and preserved explicit cleanup/error/publication semantics. No #29 startup code or additional occupancy investigation is included. [Scope](reliability/bounded-process-io.md) and [acceptance](testing/pr31-bounded-process-io.md) distinguish tested transport bounds from hard OS-call guarantees and historical root-cause closure. Issue #27 remains open; no candidate promotion.

### Combined Windows owned / bounded runtime (PR #32)

Independent adaptation after #31: the #29 creation-time Job/handle-list contract feeds the #31 nonblocking pipe pump and shared cleanup deadline. Joint native controls cover early descendant membership plus timeout/exit, overlapping invocations and real Job-query denial. [Scope](reliability/windows-owned-bounded-runtime.md) and [acceptance](testing/pr32-windows-owned-bounded-runtime.md) retain old #29's failed gate and issue #27. No extra attribution tooling, engine changes or public candidate promotion.

### Windows startup deadline semantics (PR #34)

Independent runtime increment on #32, rebased for final acceptance after #37: classify startup overruns before protocol exchange and record bounded Windows startup-stage timings. It does not use unsafe thread termination or claim a hard wall-clock bound over `CreateProcessW`; the existing operation timeout, two-second cleanup budget and Issue #27 history remain. See [scope](reliability/windows-startup-deadline.md) and [acceptance](testing/pr34-windows-startup-deadline.md).

### Exact-directory user attribution (PR #35)

Diagnostics-only follow-up for the remaining OS5 blind spot: calibrate an exact opened-directory PID query on real Windows and attach it to the existing post-failure collector before file-level Restart Manager attribution. Because the native information class is reserved for system use, this remains investigation tooling rather than a runtime API. A directory user is not automatically a proven rename blocker. See [scope](reliability/windows-directory-attribution.md), [acceptance](testing/pr35-windows-directory-attribution.md), and [ADR 0013](decisions/0013-windows-directory-query-diagnostic-only.md); Issue #27 remains open.

### In-flight remove-window sampling (PR #36)

Test-only follow-up after a successful remove required 52 quarantine attempts / ~1317 ms and therefore escaped post-failure capture. While an explicitly instrumented `engine remove` is running, the harness samples PR #35 exact-directory identities and correlates their first/last seen times with the unchanged remove result. Three real-Windows controls cover transient-success, persistent-failure and sibling isolation. See [scope](reliability/windows-remove-window-sampling.md) and [acceptance](testing/pr36-windows-remove-window-sampling.md); this does not change the production retry loop or prove a causal rename blocker.

### Distribution notices and isolated acceptance (PR #33)

Originally developed after #32 and rebased for final acceptance onto `main@f0a3f052e52ce9ba89c59bbebbce813b3b84410e` after #34/#37. Distribution preparation packages conservative Cargo source notices, a hashed dependency inventory and usage guidance with new CLI candidates, then checks core/offline PSD workflows in a fresh process environment. See [scope](releasing/distribution-readiness.md) and [acceptance](testing/pr33-distribution-readiness.md). This is not legal clearance, clean-VM/signing/minimum-OS acceptance or public release; Issue #27 stays open.

### v0.1 Final Release Gate / RC Freeze

The release-candidate freeze starts after #33, #34 and #37 are integrated. The freeze does not authorize a public release: it freezes the candidate source, requires post-merge exact-`main` CI / Managed Package / Windows Lifecycle evidence, and moves any further code change back through the full release gate. See [final release gate](releasing/v0.1-final-gate.md) and [RC freeze acceptance](testing/v0.1-rc-freeze.md).

Only release-blocking fixes may change the frozen candidate line. Feature work continues after the v0.1 decision rather than being folded into the RC.

## M4 — Safe mutation

**Goal:** introduce modifications without compromising source-file safety.

M4a is developed on `feature/m4a-raster-operations`, based on the
technical-v0.1 frozen main commit `5526dc4465c4e79ad7f2757d06b3b1693642b8af`.
It adds crop, right-angle rotate and format convert to the built-in raster
engine, with explicit no-clobber publication and CLI integration tests.
This is **post-v0.1 feature development**, not a change to the frozen RC.
M4b safe replacement and PSD mutation remain separate follow-ups beyond M4a.

M4b-1 builds on the merged M4a `develop@990a237be4a1505c7d49dd87c460560c31a63768`.
It implements image-only `--dry-run` preflight and structured dry-run results
without writing outputs; safe overwrite/replacement, durable approvals, and
PSD mutation remain separate follow-ups. The frozen v0.1 `main` does not
receive M4 changes.

M4b-2 follows `develop@fbe1bf9719a5b42c1035c29edb1b0e7ae958a069`:
explicit version-bound replacement on the four built-in image operations,
same-directory stage/publish, double SHA-256 validation, safe cleanup and
YuTool-cooperating writer serialization. There is no portable filesystem
compare-and-swap and the known race against uncooperative writers is stated
in the CLI specification. The `main` RC and signing/release gates remain
untouched; GitHub Actions are not a required development gate while quota
is unavailable.

M4b-3 follows `develop@b5dccf94f89ff5b568b2cd6c6079023fd2894daf`:
verify encoded staging, reopen/hash/decode the published destination, and
return a typed receipt with old/new SHA-256, bytes, dimensions and format.
Dry-run stays a plan. Errors after publication never claim success or
automatically roll back. This feature is isolated from v0.1 frozen `main`.

### M4c-1 — PSD mutation feasibility spike

Based on `develop@9c9ea881585b7b7c1ec54f4b6c73fb507a480285`,
the pinned Node 22.23.3 / ag-psd 31.0.2 probe ran **48** source-preserving
no-op/rename/visibility/opacity cases across 12 PSD/PSB fixtures.
Forty saved and reparsed, eight high-bit writes failed closed, and the
experiment surfaced unintended layer-effect/resource changes and a no-op
cached-composite mismatch. Visibility/opacity can also leave a stale
composite preview. Four simple-document/group/duplicate-name rename
cases pass the measured metadata checks, but **none authorizes a public PSD
mutation capability**. This is a negative safety gate with a narrow research
candidate, not a completed PSD editor.

See the [M4c-1 investigation](psd-mutation-feasibility-m4c1.md) and
[machine evidence](data/m4c1-psd-mutation-evidence-v1.json).

### M4c-2 — PSD safe-mutation scope / contract freeze

Starting from `develop@ff61d9aa975e9decc5f19bd555e0d916e529060d`,
ADR 0014 freezes a **research-only, default-deny PSD mutation policy**:
exact SHA-256 identity for four prior reviewed fixtures (simple PSD/PSB,
group and duplicate names), 8-bit RGB, bounded source, canonical layer
ID + expected old name, source byte version binding, new path only and
read-only plan. Only **rename** is a research candidate. Arbitrary user
PSD files, unknown/opaque resources, complex effects, masks, text,
Smart Objects, high bit depth, visibility/opacity and general writes
remain **blocked**. No `yu psd` public write command or Managed protocol
change is authorized.

See [ADR 0014](decisions/0014-psd-mutation-input-scope-freeze.md),
[machine policy](data/psd-safe-mutation-scope-m4c2-v1.json) and
[acceptance](testing/m4c2-psd-input-scope-freeze.md).
Next: independent raw-block scanner and full-fidelity editor acceptance
before expanding any input class beyond the exact reviewed research fixtures.

### M4c-3 — PSD/PSB byte-level metadata inventory

Starting at `develop@00d0c2fb2b67f3fc5561374498d2fb3365358f92`,
the read-only standalone research scanner inventories raw Image Resources,
layer records, Additional Layer Information, PSB long-length tagged blocks
and opaque section boundaries with offsets and byte-level fingerprints.
Unknown tags/resource IDs are surfaced rather than dropped. In the
committed corpus 13 valid PSD/PSB inputs scan, one malformed file is
rejected, and **11/13** valid inputs contain unknown resource IDs or tags.
All scans report `mutation_authorized=false` and `safe_to_rewrite=false`,
including the two fixtures without unknown blocks. A recognizable block
does not prove that any editor can preserve its contents.

See [ADR 0015](decisions/0015-psd-raw-block-inventory-boundary.md),
[inventory evidence](data/psd-raw-block-inventory-m4c3-v1.json) and
[acceptance](testing/m4c3-psd-raw-block-inventory.md). M4c-4 should
investigate nested layer records and independent preservation/fidelity
before any policy expansion or PSD write CLI.

### M4c-4 — Nested records and no-op save fidelity evidence

Building on `develop@0151a31e11aadb361505b1d87bbf24f2d4a434f1`,
the research-only parser now reads `Lr16`/`Lr32` nested records, per-channel
fingerprints and embedded layer metadata. The pinned Node 22.23.3 /
ag-psd 31.0.2 no-op writer comparison attempted 13 PSD/PSB fixtures:
11 reopened, 2 high-bit writes rejected, **11/11 saved files differ in
raw bytes**, and one differs in decoded composite pixels. Multiple files
lose raw Image Resource and tagged blocks. Byte re-encoding and semantic
change are recorded separately, and **zero files are certified safe to
rewrite**. No general PSD save or public mutation operation is enabled.

See [ADR 0016](decisions/0016-psd-nested-block-noop-fidelity.md),
[machine evidence](data/m4c4-psd-noop-fidelity-v1.json) and
[acceptance](testing/m4c4-nested-raw-noop-fidelity.md). M4c-5 must study
lossless preservation and independent editor validation before any release
gate for PSD mutation can change.

### M4c-5 — PSD fixed-width byte-preserving rename feasibility

Based on `develop@3c8769c93389f9d24847df2c3e9748852d57eb7d`,
the research-only tool tested an exact, fixed UTF-16BE-length `luni`
name patch on the four known M4c-2 SHA-256 fixtures without running
ag-psd's whole-document writer. All four disposable copies passed:
same total bytes, every non-name byte unchanged, raw channel/image-resource
and merged-composite fingerprints identical, and ag-psd read-only
reinspection showing only the selected layer's name changed.
The nested-group case updates matching ASCII Pascal and Unicode names
together; **three other cases retain mismatched legacy name representations**.

See [ADR 0017](decisions/0017-psd-byte-preserving-rename-research.md),
[byte-level evidence](data/m4c5-psd-byte-patch-evidence-v1.json) and
[M4c-5 acceptance](testing/m4c5-psd-byte-preserving-rename.md).
This experiment is **not** a PSD production editor: only exact known
fixtures, equal-size Unicode renames, output to ignored `target/`,
and **zero authorized public PSD writes**. Next prioritize independent
Photoshop/legacy-name display fidelity and output-only approval semantics.

### M4c-6 — Dual layer-name semantics and independent Photoshop read tests

Starting from `develop@07fdda430c1b6b9588e1d592785d2b7772fdf044`,
the research-only matrix used ag-psd 31.0.2, independent Python
psd-tools 1.20.0 and **real Adobe Photoshop 2026 on macOS**.
The two parsers agree on 4/4 Unicode-targeted renames and psd-tools
preview pixel hashes; native Photoshop name reads verify **2/4**.
Photoshop treats both edited `Фон → Дом` cases as special localized
**Background (`背景`)** layers and does not expose the changed Unicode
name. Only the group and non-background duplicate-name cases pass native
name reading. Photoshop documents were opened read-only and closed without
saving. Length-changing renames remain **plan-only**, not implemented.

See [ADR 0018](decisions/0018-psd-dual-name-photoshop-acceptance.md),
[independent reader evidence](data/m4c6-dual-name-fidelity-v1.json),
[real Photoshop name receipt](data/m4c6-photoshop-name-receipt-v1.json)
and [acceptance](testing/m4c6-dual-name-photoshop.md). Full editor
save/reopen, Photoshop composite fidelity, non-ASCII legacy display and
safe background semantics are **not approved**. No `yu psd` write command.

### M4c-7 — PSD rename scope freeze and paired Photoshop save gate

Based on `develop@d197334c88d123b4a2a68600ea784c543b41755b`,
M4c-7a now freezes a **deny-by-default** research gate:
exact known input hashes, canonical/raw ID agreement, no native
Background layer, matching ASCII Unicode/legacy Pascal name encodings and
equal UTF-16 name length. Among four historical fixtures, only the
**nested-group** rename is admitted for a *research* save/reopen study;
the Background PSD/PSB and legacy-inconsistent duplicate-name cases are
blocked. No public mutation authorization is granted.

A native Photoshop 2026 paired saveAs/reopen experiment is implemented
with unchanged-original control, distinct private scratch outputs,
independent parser and composite comparisons. **M4c-7b/c are BLOCKED**:
Photoshop's AppleScript channel timed out, including a read-only document
count query. The bounded readiness gate did not permit any save, so there
is no editor save/reopen acceptance evidence. The code and documented
scope can freeze without declaring Photoshop fidelity passed.

See [ADR 0019](decisions/0019-psd-rename-admission-photoshop-control.md),
[scope policy](data/psd-rename-scope-m4c7-v1.json),
[scope evidence](data/m4c7-rename-scope-evidence-v1.json),
[blocked native receipt](data/m4c7-photoshop-control-v1.json) and
[acceptance](testing/m4c7-rename-scope-and-photoshop-gate.md).

### M4c-7c — Native Photoshop readiness / saved-pair strict gate follow-up

On `develop@930e1d83068926130a22274f618390e70c5f52f8`, a renewed
macOS Photoshop 2026 probe found the process running but Apple Events
still timed out. System Events could identify Photoshop as a process,
but refused UI-window inspection due to `osascript` accessibility
permissions; whether a Photoshop dialog is blocking remains unknown.

Research safeguards now explicitly classify a missing/unresponsive
application, permission denial, malformed native responses and user
documents already open. Only a responsive empty Photoshop workspace
can proceed. A strict experimental saved-control vs saved-candidate
comparator now checks **every changed byte** against the one authorized
Unicode/Pascal name range and rejects any unrelated raw resource,
channel, metadata or composite difference. These negative checks pass
in synthetic fixture experiments but do **not** count as a Photoshop
save/reopen.

The [new native receipt](data/m4c7c-photoshop-control-v2.json) records
`apple_events_unresponsive`, **zero save commands attempted**,
**zero native save/reopen passes**, and **zero PSD product writes**.
See [ADR 0020](decisions/0020-psd-native-photoshop-readiness-gate.md)
and [tests](testing/m4c7c-native-gate-and-saved-pair.md). Native
Photoshop app recovery requires separately confirmed user control;
do not force close or silently alter macOS permissions.

### M4c-7c — Native Photoshop save/reopen reacceptance (2026-10-13)

After the user restarted Photoshop 2026, the native readiness preflight
reported zero open documents and accepted Apple Events. The one
strictly scoped `nested-group` fixture was saved to two independent
scratch outputs: unchanged `Group 1` control and byte-patched
`Group 2` candidate. **Both saved and reopened successfully in actual
Photoshop**, and the expected names persisted. Independent psd-tools
found matching logical trees and **matching decoded pixels between the
two saved outputs**, with no original/source-input changes.

However **full fidelity is NOT accepted**: Photoshop saved outputs
produce different preview pixels compared with the pre-save inputs
(including the untouched control), and the two saved outputs have
different non-name metadata (`1060`, `1050`, `1058`, several `shmd`
records) and differ by 16 bytes in file length. Their raw layer-channel
and encoded merged-composite bytes match. The strict byte-preservation
gate still fails, **PSD public write remains BLOCKED**.

See [ADR 0021](decisions/0021-photoshop-native-save-reopen-reacceptance.md),
[v3 native receipt](data/m4c7c-photoshop-native-reacceptance-v3.json)
and [reacceptance tests](testing/m4c7c-native-reacceptance.md). The
prior [v2 unresponsive receipt](data/m4c7c-photoshop-control-v2.json)
remains preserved as history. Next investigate editor metadata and
composite recomposition deltas (M4c-7d), not public PSD mutation.

### M4c-7d — Native rendering and metadata differential (research accepted)

Two independent native Photoshop control/renamed PSD save runs now have
four **Photoshop-rendered PNGs each**. Photoshop-native pixels differ
by **0** between original, renamed, saved control and saved renamed
candidate in both runs. The earlier 160/20,000-pixel difference comes
from `psd-tools 1.20.0` independently compositing the *original* PSD,
not from a native Photoshop save-induced visual change on this fixture.

Photoshop Slices Resource **1050** encodes the different saved-document
names, explaining this exact control/candidate **16-byte** file-length
delta (20 vs 12 UTF-16 code units). Other byte-level variations in
EXIF **1058**, XMP **1060** (document IDs/history/timestamps) and
four `shmd/cust` metadata blocks remain semantically **unverified**.
See [ADR 0022](decisions/0022-photoshop-m4c7d-metadata-fidelity-boundary.md),
[native pixel/metadata evidence](data/m4c7d-photoshop-metadata-preview-v1.json)
and [targeted resource forensic evidence](data/m4c7d-metadata-forensics-v1.json).

### M4c-7e — Research exit, deny production PSD mutation

The frozen PSD/PSB corpus expands research **read-only** coverage to
13 valid fixtures, including high-bit, Smart Object, Type, shape,
artboard and masks. Three representative files have also been
opened/read/closed without saving in Photoshop 2026: ordinary pixel
layers, Cyrillic/Background, and a Type layer. Only
`bench-baseline-rgb` is a new normal-pixel **research candidate**;
it has **not** been admitted to M4c-2/M4c-7a mutation allow-lists.
General PSD writes, length-changing names, Background, text/effects
and opaque metadata preservation remain blocked.

See [ADR 0023](decisions/0023-m4c7e-psd-mutation-research-exit.md),
[corpus profile](data/m4c7e-readonly-corpus-profile-v1.json),
[Photoshop read-only receipt](data/m4c7e-native-readonly-samples-v1.json)
and [final decision](data/m4c7e-release-decision-v1.json).
The frozen v0.1 `main` baseline and production PSD capabilities remain
unchanged. A separate approved release gate is required for any write.

### M4c-8 — Ordinary pixel and multilingual name research

On `develop@30db4036368e3961213f4e69555dda9d51f00d35`,
the first **ordinary** RGB 8-bit pixel layer fixed-width rename
experiment uses frozen `bench-baseline-rgb`:
`Layer 1 → Layer A`. The byte patch changes only two name bytes
across `luni` and matching ASCII Pascal, leaving unrelated input
PSD bytes untouched. Actual Photoshop 2026 saved, closed and reopened
both scratch PSD control/candidate with correct names; independently
decoded saved pixels and encoded image/channel bytes agree.
**Strict opaque metadata fidelity remains unverified**, so no
public PSD writer or new broad allow-list is enabled.

Photoshop-native scratch files also demonstrated read/reopen support
for Chinese `测试图层`, Cyrillic `Слой` and duplicate
`Layer 0` names. The non-ASCII Pascal byte encodings are **not**
semantically equivalent to their Latin-1 inventory text; separate
Unicode-only patches display correctly in Photoshop but retain old
legacy bytes and are **denied** as a production edit strategy.
Duplicate names require stable target identity rather than name lookup.

See [ADR 0024](decisions/0024-m4c8-pixel-multilingual-rename-boundary.md),
[research policy](data/m4c8-limited-rename-policy-v1.json),
[normal pixel Photoshop receipt](data/m4c8-pixel-photoshop-fidelity-v1.json),
[multilingual receipt](data/m4c8-multilingual-photoshop-names-v1.json),
[Unicode-only mismatch experiment](data/m4c8-luni-only-multilingual-v1.json)
and [acceptance](testing/m4c8-pixel-multilingual-acceptance.md).

### M4c-9 — Legacy name charset, layer identity and metadata semantics

On `develop@24440751bf059d8696ce639e6d603ddc611d028b`,
the frozen Photoshop-generated Chinese and Cyrillic research PSDs
were cross-checked by source SHA-256. Their legacy Pascal bytes decode
to the same names as Unicode `luni` **using GB18030 on this machine**
(`测试图层`: `b2e2cad4cdbcb2e3`,
`Слой`: `a7b3a7dda7e0a7db`). This is
**not a general PSD charset guarantee**; Latin-1 raw inventory text
is a lossless byte representation, not the intended user display.

Two same-name `Layer 0` pixel layers were disambiguated using unique
Photoshop `lyid` **6** and **2**. A precisely scoped,
fixed-length scratch-only dual-name edit selected `lyid=6` and
persisted only its rename to `Layer A` through actual Photoshop
save/reopen; the other `lyid=2` layer retained its name.

M4c-9 also recognized a `layerTime` float64 field in the
observed 52-byte `shmd/cust` items (16 metadata instances
from M4c-7d's two paired saves). XMP document-instance-history fields
and EXIF remain semantically **not certified**. These recognized
fields are **not** a new metadata ignore-list, and no production
write admission is enabled.

See [ADR 0025](decisions/0025-m4c9-legacy-name-layer-id-metadata.md),
[legacy encoding and native lyid receipt](data/m4c9-encoding-lyid-v1.json),
[metadata evidence](data/m4c9-metadata-semantics-v1.json),
and [acceptance](testing/m4c9-encoding-lyid-metadata-acceptance.md).
M4c-10 should focus on locale-scoped dual-name synchronized updates
and stronger Photoshop metadata preservation; public PSD editing is
still **BLOCKED**.

### M4c-10 — Multilingual dual-name sync and lyid reorder/group research

Based on `develop@6002b1b2b844dc48ceb11357cbc814d2a041366e`,
two SHA-bound Photoshop-native ordinary pixel PSDs were used to
demonstrate **synchronized**, equal-width Unicode `luni` and
sample-observed GB18030 Pascal renames for Chinese
`测试图层 → 实验图层` and Cyrillic `Слой → Тест`.
Both passed real Photoshop 2026 paired scratch save/reopen, retained
`lyid=6` target and `lyid=2` non-target identity, and had
identical independent saved-control/candidate composite pixels.

Separately, duplicate-name layers were **reordered** and
**moved into a new native Photoshop group** in independent scratch
experiments; target `lyid=6` and other `lyid=2` remained stable,
while raw PSD layer record indexes changed and new grouping-related
IDs appeared. Raw offsets and names cannot substitute for revalidated
per-document layer identity.

Both synchronized multilingual Photoshop saved pairs still have
unexplained image resources/metadata, despite identical raw layer
channels and merged image data. No blanket GB18030 codec policy,
no global `lyid` permanence, no public PSD mutation is approved.
See [ADR 0026](decisions/0026-m4c10-multilingual-dual-name-and-lyid.md),
[paired multilingual evidence](data/m4c10-multilingual-sync-v1.json),
[native structure evidence](data/m4c10-lyid-reorder-group-v1.json),
[saved metadata audit](data/m4c10-metadata-paired-v1.json) and
[M4c-10 acceptance](testing/m4c10-multilingual-lyid-acceptance.md).

### M4c-11 — Metadata semantic classification and no-clobber research output

Based on `develop@5b20e4c3c196ac5933305bf6257e2c00df5ab137`,
two exact-hash Photoshop 2026 saved multilingual control/candidate
pairs have been analyzed at **TIFF EXIF tag level**: both have
3 IFDs and 22 entries with no entry additions/removals; the observed
changed field is `306 DateTime`. XMP `DocumentID`, `InstanceID`,
`History` and time-related markers remain semantically protected;
`shmd/cust` entry bytes agree within both saved pairs, but unknown
metadata is **not globally certified**. The exclusion list stays empty.

A **separate research-only** output test now exercises exact
SHA-256 source version binding, a deterministic fixed-width M4c-8
normal pixel candidate, exclusive same-filesystem staging, atomic
hard-link no-clobber publication, revalidation and negative race,
symlink and source-version tests. It only publishes into ignored
`target/m4c11-safe-output/run-*`, never via the public `yu` CLI.
Filesystem persistence, production concurrency, arbitrary PSD metadata
fidelity and release permission remain **BLOCKED**.

See [ADR 0027](decisions/0027-m4c11-psd-metadata-and-output-gates.md),
[EXIF/XMP/shmd evidence](data/m4c11-metadata-semantic-gate-v1.json),
[research publication evidence](data/m4c11-safe-output-v1.json)
and [acceptance](testing/m4c11-metadata-publication-acceptance.md).

### M4c-12 — Interrupted PSD output and filesystem compatibility research

Based on `develop@d41650622ce8b9ee9e7900439b86e67e5cdb1140`,
a separate **research-only** transaction and recovery probe models
exact-source SHA, filesystem inode/version binding, versioned intent,
private candidate staging, file and directory `fsync`, atomic
hard-link no-clobber output, post-output candidate verification and
a versioned receipt for the same frozen M4c-8 pixel rename.

Four deterministic interruption cutpoints plus **two real SIGKILL
terminations of disposable child processes** demonstrate how to
classify private-stage-only, output-present/no-receipt and complete
states without automatic recovery writes or deletion. The recovery
probe distinguishes **same inode vs byte-identical rival output** and
fails closed on changed/replaced source, staged bytes, intent, output
symlinks and filesystem incompatibility.

Actual directory `fsync` worked on this one local macOS filesystem.
Simulated cross-device/unsupported-link and directory-sync failure
are explicit **refusals**; no copy fallback is permitted. Neither
process SIGKILL nor a successful directory `fsync` proves
power-loss durability or cross-platform filesystem support.
No XMP, EXIF, `shmd` or unknown metadata is auto-normalized.

See [ADR 0028](decisions/0028-m4c12-recovery-filesystem-metadata.md),
[recovery evidence](data/m4c12-recovery-filesystem-v1.json) and
[acceptance](testing/m4c12-recovery-filesystem-acceptance.md).
**PSD production mutation stays BLOCKED** and `main` remains frozen.

### M4c-13 — Real filesystem matrix and manual-only output recovery

On `develop@7e87e7c5f3a65b6e801a54a4e60210af34c41089`,
YuTool ran an **actual local APFS** hard-link/no-clobber,
device/inode equality, file `fsync` and output directory `fsync`
experiment entirely inside ignored research scratch space.
The host also has mounted NTFS (**read-only**) and OrbStack NFS
volumes, but no test data was written there; separate Windows/Linux
device or power-loss receipts remain **NOT TESTED**.

A new M4c-13 `--inspect` **read-only manual recovery** tool
classifies M4c-12 staged-only, published-without-receipt, previously
verified, source-drift, competing-output and invalid-evidence states,
prescribing manual holds without automatic PSD retry, deletion,
republication or fabricated success receipts. Hard-link inode
identity is checked so even a rival output with the *same SHA-256*
cannot be confused with this transaction. Postpublication data
replacement is detected despite an old verified receipt.

See [ADR 0029](decisions/0029-m4c13-filesystem-matrix-manual-recovery.md),
[real/missing filesystem evidence](data/m4c13-filesystem-matrix-v1.json),
[manual recovery evidence](data/m4c13-manual-recovery-v1.json) and
[acceptance](testing/m4c13-filesystem-recovery-acceptance.md).
Cross-filesystem compatibility and real crash durability remain
**unverified**. Public PSD mutation stays **BLOCKED**.

### M4c-14 — Cross-platform device handoff and PSD mutation release review

Based on `develop@c6f80b4beb6b51ec64904405942b43c5ec312426`,
the Runner fleet was inspected and had only **one live macOS Runner**;
no native Windows/Linux device could produce real M4c-14 receipts.

M4c-14 adds an independently runnable, pinned Node.js 22.23.3
**research-only** filesystem device probe. It actually observed
file `fsync`, hard-link/inode, `EEXIST` and directory `fsync`
on the current Mac under ignored `target/`. On future Windows/Linux
hosts it writes distinct **unattested** scratch device receipts,
with OS, checkout SHA, probe source hash and actual observed
errors; it does **not** overwrite the macOS committed receipt.

A separate fail-closed release assessment lists **seven unaccepted
gates**: real Windows device, real Linux device, supported filesystem
matrix, power-loss durability, full PSD metadata/editor fidelity,
production-safe source/output concurrency and explicit manual
PSD writer authorization. Synthetic unit-test receipts are never
accepted as real hardware verification and cannot grant a writer.

See [ADR 0030](decisions/0030-m4c14-cross-platform-psd-write-release-gate.md),
[local device evidence](data/m4c14-local-device-v1.json),
[M4c-14 release decision](data/m4c14-psd-write-release-decision-v1.json),
and [operator handoff](testing/m4c14-cross-platform-release-acceptance.md).
M4c-14 **research handoff** may freeze; PSD production mutation and
all user PSD output remain **BLOCKED**, and v0.1 `main` stays frozen.

Candidate capabilities:

- image crop/rotate/convert;
- PSD layer rename where supported;
- PSD show/hide where supported;
- opacity changes where supported;
- safe output replacement;
- `--dry-run`;
- structured change summary;
- post-operation validation.

Any PSD mutation capability must be gated by actual engine conformance results.

## M5 — YuTool Manager

**Goal:** provide a small GUI for runtime/engine management.

Initial GUI scope:

- installed engines;
- available managed engines;
- version;
- provider class;
- capability list;
- install/update/remove;
- health state;
- disk usage;
- license/source metadata.

The first Manager is **not** an image editor.

Preferred direction: Tauri sharing the Rust core with the CLI.

## After the runtime is proven

Only after the engine/runtime architecture is stable should YuTool expand into additional capability families.

Potential areas include:

```text
PDF
media / FFmpeg
metadata
SVG/vector
OCR
archives
documents
RAW
```

Each new family should begin with:

1. a capability contract;
2. engine candidates;
3. conformance fixtures;
4. an explicit scope decision.

## Non-goals for the first release

Do not make v0.1 responsible for:

- every image format;
- Photoshop-level PSD editing;
- full PDF tooling;
- video/audio transcoding;
- OCR;
- a general plugin marketplace;
- remote/cloud execution;
- an image-editing GUI.

The first release should prove that **one lightweight runtime can safely discover, resolve, execute, and report local capabilities through `yu`.**
