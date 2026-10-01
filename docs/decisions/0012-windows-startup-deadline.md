# ADR 0012 — Treat Windows startup as part of the operation deadline without unsafe preemption

## Status

Accepted for PR #34 implementation; public release readiness remains separate.

## Context

A Windows package run returned from synchronous engine startup after roughly 66.6 seconds despite a 30-second operation timeout. The runtime clock began before startup, but the first deadline branch lived in the protocol exchange loop, so the late startup was reported as a transport/process-exit timeout and could reach exchange after the budget was already exhausted.

Windows does not provide this runtime with a safe general-purpose way to forcibly cancel every synchronous process-creation/filesystem call. Moving startup to a detached thread would make return time look bounded while allowing hidden work and handles to outlive the invocation. `TerminateThread` would violate Rust/native resource safety.

## Decision

The original operation clock remains authoritative. Immediately after startup returns and before protocol exchange, YuTool rejects an already-expired invocation and performs ordinary owned cleanup. Windows startup records phase timings so a future slow return can be localized to Job creation, path/command preparation, environment, pipes, inheritance, attributes, `CreateProcessW`, or post-create cleanup.

Do not claim hard preemption of synchronous Windows OS calls. Do not introduce detached startup workers, forced thread termination, wider timeouts or unrelated-process control.

## Consequences

- `--timeout-secs` now has an explicit startup-to-exchange gate: expired startup cannot intentionally consume protocol I/O.
- Wall-clock return can still exceed the requested timeout if a synchronous native call itself blocks beyond it.
- Startup timing observations improve diagnosis without changing the stable JSON result schema.
- The existing two-second owned cleanup budget remains separate.
- Historical Windows timeout/OS5 root causes remain unresolved until evidence establishes them.
