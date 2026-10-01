# Windows startup deadline semantics — PR #34

Originally developed from the reviewed #32 baseline. Final acceptance is rebased onto `main@f8b461c14c1c71ac76117c4eb2aca43b570db0dd` after #37 so the #35–#37 Windows removal diagnostics remain intact. PR #33 remains separate. Issue #27 stays open.

## Problem

PR #33 Managed Package28 observed an extracted Windows candidate spend about 66.6 seconds in the synchronous startup interval for a 30-second PSD operation. The error reported `elapsed_ms=66595`, `spawn_ms=66595`, no request/output progress and `phase=process_exit` only because the exchange loop first noticed the already-expired clock after startup returned. That evidence did not isolate the slow native call and does not establish a security-product, filesystem or `CreateProcessW` root cause.

The operation clock already starts before process creation, but before this change there was no explicit deadline gate between synchronous startup and protocol exchange. A late-returning startup could therefore enter the exchange loop after the configured budget had already elapsed.

## Implemented boundary

After startup returns, YuTool checks the original operation deadline **before any protocol exchange**. If startup has consumed the budget, the call returns `EXECUTION_FAILED` with `phase=startup`, then runs the existing owned-process cleanup path. Request bytes are not intentionally written after that gate. A startup error that itself returns after the budget is also classified as a startup timeout while retaining the underlying startup error text.

Windows startup now records bounded phase durations in microseconds:

1. `job_create`
2. `path_validation`
3. `command_line`
4. `environment`
5. `pipe_create`
6. `handle_inherit`
7. `attribute_list`
8. `create_process`
9. `post_create_cleanup`

Successful opt-in lifecycle traces attach these timings to the existing `started` record. Startup failures include the completed timing prefix in the error message. The timings contain no request/document bytes and are diagnostic observations, not a stable public schema.

Cleanup remains separate: once a process exists, normal timeout/error completion still closes local pipe endpoints and uses the existing shared two-second cleanup observation budget. This PR does not enlarge the operation timeout, cleanup budget or uninstall quarantine retry window.

## Deliberate non-guarantee

This is **not** a hard wall-clock preemption of Windows process creation. `CreateProcessW` and other synchronous OS/filesystem calls do not gain a safe cancellation primitive from this change. YuTool does not use `TerminateThread`, detach a startup worker, leave a hidden background reaper, or kill unrelated processes to pretend otherwise. If a synchronous native startup call itself stalls past the requested deadline, the caller may still regain control only after that call returns; at that point YuTool rejects protocol exchange and cleans up the owned process.

This distinction is intentional: the operation deadline governs whether an engine invocation may proceed, while the runtime remains honest about OS calls it cannot safely interrupt.

## Unchanged scope

No engine payload, manifest/capability contract, public CLI flag, default timeout, test parallelism, package install/remove policy, dependency lock, signing policy or Release/tag behavior changes. Historical OS5 and timeout observations remain evidence; passing finite samples do not set `root_cause_fixed=true` or `public_release_ready=true`.

See [acceptance](../testing/pr34-windows-startup-deadline.md), [ADR 0012](../decisions/0012-windows-startup-deadline.md), and the preceding [owned/bounded runtime](windows-owned-bounded-runtime.md).
