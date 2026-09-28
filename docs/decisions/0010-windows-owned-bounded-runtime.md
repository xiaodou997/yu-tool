# ADR 0010 — Adapt creation-time Job ownership to bounded transport

Status: accepted for implementation; exact-head joint acceptance required.

## Context

#31 replaces blocking transport and cleanup waits. Unmerged #29 proved focused creation-time ownership but failed its full independent lifecycle gate. Directly merging that older implementation would conflict with the current pipe/process interfaces and could reintroduce blocking waits.

## Decision

Independently port only the documented CreateProcessW Job-list/handle-list startup contract, argument/environment serialization and directory validation onto #31. Reuse #31's nonblocking pipe pairs and shared cleanup deadline. Keep process and pipe ownership separate; expose no infinite native wait. Do not change Unix startup, engine payload, uninstall policy or diagnostics tools. Test early descendant ownership, inherited-pipe timeout, real native query denial and normal completion on one combined path.

## Consequences

Windows requires the Job-list API baseline (Windows10/Server2016+); actual supported-system acceptance remains separate. The explicit handle list is not a sandbox. Failure before creation has no uncontrolled fallback; failures after creation use the unchanged bounded settlement. Old #29 evidence remains immutable and its branch is not automatically merged/closed. Joint success does not close historical OS5/timeout attribution or authorize public v0.1.
