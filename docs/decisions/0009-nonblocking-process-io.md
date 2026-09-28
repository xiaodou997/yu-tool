# ADR0009 — Deadline-driven nonblocking process transport

Status: implemented for review in PR #31; acceptance evidence is separate.

The main/#28 transport can observe an exchange timeout and then block indefinitely in worker joins. A thread detached at timeout would hide resource lifetime, and freeing an unfinished OVERLAPPED buffer is unsafe. This increment replaces blocking workers rather than wrapping them in another timer.

Use synchronous, nonblocking local pipe endpoints and a single bounded/fair pump. Unix uses O_NONBLOCK; Windows uses byte PIPE_NOWAIT on the private server end, with normal blocking engine clients. It is deliberate polling, not an attempt to emulate OVERLAPPED background I/O. A no-progress turn sleeps up to5ms; byte and operation deadlines stay unchanged. Cancel pump work by closing local endpoints before terminating/waiting for owned processes. Child and Windows Job observations share a2-second cleanup deadline and return explicit incomplete-cleanup errors.

Tradeoffs: polling/extra wakeups instead of an async reactor; a small additional Windows pipe wrapper and tests; no pending buffer or I/O worker lifetime. Windows documents PIPE_NOWAIT as compatibility mode and recommends OVERLAPPED for real asynchronous I/O, so a future reactor needs its own design. No hard wall-clock promise for a stalled OS call. No user cancellation-token API is added.

Process startup stays on current main. This decision neither copies nor waives the pending PR #29 creation-time Job change. Transport integration with that branch remains explicit future work. File ownership diagnostics, quarantine retries, package dependencies and public release policy are not expanded.

References: the Microsoft named-pipe mode and CancelIoEx documents linked in `docs/reliability/bounded-process-io.md`.
