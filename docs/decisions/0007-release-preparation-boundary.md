# ADR 0007 — Locked developer candidates before public release

Status: Accepted for engineering preparation; public release authorization is not included.

## Context

M3 has a frozen PSD runtime, but CI artifacts and placeholder manifests are not a production distribution channel. Prior Windows quarantine denial remains unidentified. Application dependency resolution was unlocked, and real-package tests used an in-process test downloader rather than the public CLI install route.

## Decision

Track Cargo.lock and the managed ag-psd package-lock.json. Pin core/package CI to the already validated Rust 1.98.1 and require locked dependency resolution. Keep optional Node/ag-psd out of the core archive. Add explicit `engine install --archive` as a bounded offline source for the existing verified installer, not a fallback or a weaker manifest policy.

Produce only source-qualified developer candidate archives with compiler/lock/binary identity, fixed member names and explicit unaccepted release gates. Test the extracted release executable, including actual package install, PSD pixel fingerprints and repeated lifecycle cleanup. Retain original failures and do not equate rerun success with cause removal.

Do not choose the project's license, accept redistribution, claim supported minimum OS/signing/notarization, create a tag or publish. Those require separate decisions/evidence. Candidate transport checksums do not establish publisher authenticity or whole-application reproducibility.

## Consequences

CI builds both ordinary tests and a release candidate, increasing validation cost while testing what would actually be shipped. Local offline installs are usable without hosting or a system Node runtime. Windows persistent denial still fails closed and preserves the installed version; additional diagnostics improve investigation, not permissions. Frozen M3 receipts keep their original historical meaning.
