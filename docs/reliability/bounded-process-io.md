# Bounded process waiting and cancellable pipe transport — PR #31

Base: main after #30, `691892c73631175ac26f5aa2421b6f5a4d5cdae7`. This is independent of unmerged #29. It changes transport/cleanup, not process creation-time Job assignment, engine payload, quarantine policy, or diagnostics attribution. Issue #27 stays open; no release or candidate promotion.

## Problem and implemented boundary

A deadline in the exchange loop does not bound a later blocking child wait or worker join. Killing the direct child/Job also does not release an output writer retained by another process. The former transport used three blocking I/O workers and joined them during cleanup.

The new transport owns three nonblocking parent endpoints and services them in one fair loop. Engine endpoints remain ordinary blocking byte streams. No I/O worker thread, detached reaper or outstanding overlapped buffer exists. Every iteration checks the unchanged operation deadline, writes at most one 8KiB request chunk and reads at most one 8KiB chunk from each output. A no-progress iteration sleeps up to5ms rather than spinning. Partial output is not EOF; every request byte must be written before closing stdin. Existing64KiB/16MiB/64KiB request/stdout/stderr bounds remain.

On completion, error or deadline, cleanup closes the local endpoints first (cancelling future pump I/O), requests termination of this invocation's process group/Job, and polls the direct child's exit. Windows additionally requires successful observation of Job ActiveProcesses=0. All cleanup polls share one2-second deadline; a later resource cannot restart it. A missing completion or failed query produces an error, not success. TerminateProcess racing Job termination is disregarded only after direct exit is actually observed within the same cleanup phase. Completed cleanup results remain cached and Drop is an unwind fallback, not a retry.

The pre-guard attach-error path also closes endpoints and uses bounded direct-child settlement. Windows startup STILL uses the main/#28 spawn-then-attach path. This does not merge #29, waive its failed lifecycle gate or claim that creation-time ownership is already present.

## Platform choice and cancellation semantics

Unix pipe parent endpoints use O_NONBLOCK; child endpoints do not. Linux creates close-on-exec descriptors atomically; other Unix targets set FD_CLOEXEC before spawn. This is trusted-engine lifecycle control, not a sandbox against hostile concurrent same-user code.

Windows uses local byte-type named-pipe server endpoints in synchronous PIPE_NOWAIT mode. Names contain128 random bits; first-instance creation and remote-client rejection are required. std::process::Command receives only the matching blocking client endpoints. Its retained client handles are dropped immediately after spawn. Empty connected reads and full writes mean WouldBlock; peer closure means EOF. No FlushFileBuffers, infinite waits, asynchronous buffer lifetime or forced thread termination is used.

PIPE_NOWAIT is a documented compatibility-mode API. Microsoft recommends OVERLAPPED for asynchronous background I/O; this design is deliberately a small synchronous polling pump, not an overlapped implementation. Overlapped I/O was not chosen because requesting cancellation does not guarantee completion and its memory cannot be freed while a request remains pending. Any future event-driven replacement needs independent completion/lifetime tests. Cancellation here means stop-and-close on transport error/deadline, not a new user-facing cancellation-token or Ctrl+C API.

## What the deadline does NOT promise

The operation timeout remains unchanged (including1-second negative controls and default30 seconds). Cleanup has a separate2-second observation budget; it does not enlarge the engine's execution allowance or the existing2-second uninstall quarantine budget. The transport has no blocking wait()/join() path, but it is not hard real-time: process creation, OS/driver calls, scheduler pauses, optional trace filesystem writes, path operations and PNG decoding cannot be forcibly interrupted by this Rust deadline. Closing a handle is not a guarantee against a stalled kernel.

If OS termination/query fails or the deadline expires, resource completion is explicitly unconfirmed. This implementation does not secretly hand work to a detached thread or promise later cleanup. On Unix an unkillable process may still require operator attention; on Windows kill-on-Job-close remains a final OS safeguard, not proof of exit. No external holder is killed/unlocked. Observed Job emptiness is not proof every external file reference has disappeared, and cannot close the historical OS5/30-second investigation.

## Regression evidence and changed tests

Eight new cases use live pipe handles and a real helper child: empty-versus-EOF, actual backpressure, a writer retained outside the engine Job, a reader retained outside the Job with a prefilled pipe, bounded observation of a still-live child followed by explicit cleanup, shared-deadline exhaustion/error, capped reads, and complete64KiB input delivery. The two retained-peer cases must return with their test-owned peer still open; they do not rely on killing that holder. A first local test assumed64KiB would fill every pipe; macOS disproved that assumption. The fixed test establishes actual WouldBlock before starting the invocation, without relaxing its deadline.

The #28 execution/cleanup result matrix, error preservation, cached outcome, unwind cleanup, engine metadata and staged-PNG/no-clobber guards remain. Worker-panic injection is replaced with a private cfg(test)-only cleanup-observation failure because production no longer has I/O workers. This is explicitly fault injection, not a claimed kernel error reproduction. The unwind test observes pipe closure rather than a joined thread. Existing CLI lifecycle/timeouts, real .yu2 pixel fingerprints and first-failure diagnostics remain mandatory.

The opt-in cleanup trace retains its developer fields, with workers_joined=0 and io_transport=nonblocking_poll, plus io_endpoints_closed, job_empty_confirmed, cleanup_budget_ms and cleanup_elapsed_ms. Controlled trace assertions check these rather than pretending three workers still exist. Historical artifacts are unchanged.

Exact-head local/Windows/three-platform evidence belongs in the PR receipt, not predeclared in this scope document. See [acceptance](../testing/pr31-bounded-process-io.md) and [ADR0009](../decisions/0009-nonblocking-process-io.md).

## Primary API references

- https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-type-read-and-wait-modes
- https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-client
- https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelioex
- https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-queryinformationjobobject
