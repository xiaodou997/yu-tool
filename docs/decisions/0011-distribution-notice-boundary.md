# ADR 0011 — Source-backed distribution notices without release approval

Status: accepted for implementation; exact-head acceptance and legal review separate.

## Decision

Keep the core/engine split. New CLI developer candidates carry fixed-path,
hash-bound notices, a target-filtered normal/build Cargo dependency inventory,
and a usage guide. Retain legacy v1 extraction; new v2 archives must validate all
auxiliary members before writing. Preserve actual upstream texts/declarations;
never select an alternative license or fabricate a missing copyright notice.
Inspect existing optional-engine notices without changing frozen engine packages.

Add a standalone fresh-environment acceptance harness that runs the packaged CLI
without using development tools for its tested commands. Keep original Cargo,
pixel-fingerprint and lifecycle gates, and stop on the first unexpected result.

## Limits

Resolved dependency closure is not an exact linked binary inventory. Collection
is not a legal audit; native embedded code, compiler/standard-library notices,
project licensing and final redistribution approval need separate review. Fresh
HOME and empty PATH on an existing runner do not certify a clean VM, minimum OS,
network isolation or signatures. Issue #27 remains open. No publishing, tag or
candidate promotion is part of this decision.
