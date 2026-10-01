# Distribution notices and isolated acceptance — PR #33

Originally developed from the reviewed #32 baseline and rebased for final acceptance
onto `main@f0a3f052e52ce9ba89c59bbebbce813b3b84410e` after #34/#37. The current
Windows startup-deadline and removal-diagnostic work stays intact. Old #29 remains
closed as superseded with its failure history retained. Issue #27 remains open.
No Release, tag, signing, public upload or candidate promotion is included here.

## What the candidate contains

New builds use developer archive layout **v2**, keeping the CLI build-info schema
and executable paths while adding three bounded fixed-name files:

```text
bin/yu[.exe]
build-info.json
RELEASE-STATUS.txt
THIRD-PARTY-NOTICES.txt
dependency-inventory.json
USAGE.md
```

Each additional file has a SHA-256 in `build-info.json`. Extraction verifies the
fixed path set, file type, size bounds, hashes, source/target identity and unapproved
review status BEFORE writing files. No archive-supplied path becomes an arbitrary
output path. Legacy layout v1 remains readable; old artifacts are not rewritten.
This changes developer packaging, not the public `yu` CLI or PSD engine protocol.

## Source-backed notice inventory

The builder invokes locked Cargo metadata for the selected target and follows the
resolved `yu-cli` normal/build dependency closure, excluding dev-only edges. It
records package source, version, upstream license expression, Cargo.lock checksum
reference, and discovered license/notice-file bytes and hashes. Paths in the public
inventory are relative to their dependency package, not developer home paths.
Files matching LICENSE/LICENCE/COPYING/NOTICE/COPYRIGHT, including nested matches
and an explicit `license_file`, are preserved without rewriting their text. Source
symlinks and outside-package license paths are refused; traversal and byte counts
are bounded. Missing evidence is recorded rather than filled with guessed text.

This is deliberately a conservative **resolved dependency inventory**, not a
linker-produced bill of materials. Cargo feature unification, build scripts and
proc macros can over-include packages. Native bundled subcomponents and notices
not identifiable by the collector's names require further review, as do Rust
standard-library/compiler-runtime and OS/system library obligations. Presence of
314 files, or any other count, would not prove legal completeness. License
alternatives are NOT automatically selected. The project license remains the
owner's decision. `redistribution_review_accepted` and `public_release_ready` stay
false, including when no missing source notice was observed.

Optional engines remain separate. Acceptance inspects the existing `.yu2` archive's
Node aggregate LICENSE and the three packaged npm dependencies' notices and hashes.
It never rebuilds/relabels the frozen engine payload merely to add this inventory.
Package notice presence is not a final redistribution audit of embedded code.

## User-facing path, independently of Cargo tests

`tools/accept_cli_candidate.py` verifies the candidate receipt/archive and exact
expected source, unpacks the candidate into a fresh temporary application directory,
then starts only that executable with fresh HOME/USERPROFILE/app-data/YU_DATA_HOME,
empty PATH, test Node overrides, and a working directory outside the checkout.
It copies its explicit PSD fixture, manifest and engine archive into that directory.
The CLI child receives no Cargo/npm/Python locations, credentials, or runtime trace
flags from the developer environment. Python runs the acceptance harness, not the
core CLI or private PSD engine. The harness never invokes Cargo, npm, Git or a shell.

The sequence checks core version/doctor/capabilities, a generated 2x2 RGBA PNG and
1x1 resize, PSD unavailable before install and before activation, explicit offline
install/activation, all four read-only operations, bitmap export, output-conflict
refusal, deactivation and complete removal. Input/archive and conflict-output
hashes must stay unchanged. It has no failed-case retry; an unexpected command exit
or semantic assertion stops the sequence and leaves a failed report and raw output.
Existing independent pixel fingerprints and repeated lifecycle tests remain gates.

This is **fresh process-environment acceptance on the current host**, NOT a clean
VM, clean OS, network sandbox, minimum supported OS or signing/notarization test.
The manifest/archive pair must come from a trusted source: matching hashes alone
are not authentication. The explicit offline transport is exercised but network
access is not forcibly disabled by this harness. Its outer command timeout is a
harness backstop, not a new runtime cancellation guarantee.

## CI and records

The three-platform package workflow runs isolated acceptance after its existing
pixel/lifecycle checks and before uploading a validated CLI candidate. A failure
still blocks candidate upload and manifest assembly. Original logs and a bounded
report are retained in CI; exact-head results and review decisions belong in the
PR, not predeclared in this document. The Windows lifecycle workflow keeps its
existing cases and budgets and gains only new builder-input path triggers.

Reports for local runs go beneath `target/pr33-*`; testing instructions are in
`docs/testing/pr33-distribution-readiness.md`. Do not distribute the acceptance
fixtures, local logs or unapproved developer packages as a public release.

## Primary technical references

- Cargo metadata, target filtering and dependency kinds: https://doc.rust-lang.org/cargo/commands/cargo-metadata.html
- Cargo package license declarations: https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields
- npm lockfile package metadata and integrity: https://docs.npmjs.com/cli/v11/configuring-npm/package-lock-json
