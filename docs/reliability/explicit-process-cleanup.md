# Explicit process cleanup outcome — PR #28

Base: `46fa4e11aecf9a9824b0928bdcff6453b63eb780`. Related investigation: issue #27.

This first lifecycle-hardening increment changes error propagation, not process-creation containment or cancellation deadlines. No Release/tag, candidate promotion, dependency upgrade, engine payload change or directory-query experiment is included. The saved experiment remains separate.

## Implementation

`Running` owns the child, process group/private Job and I/O workers. Normal execution separates `exchange()` from `finish()`. Once the guard exists, input/output setup errors, worker-creation failures, exchange errors, deadline errors and successful exchange all settle cleanup before returning.

Cleanup records process-group/Job termination failure, direct-child observation/kill/reap failure and worker-join panic. It still attempts subsequent cleanup steps after a recorded error. Unix ESRCH means that no process group remains; other errors are retained. A direct-child kill error is disregarded only when a new successful observation confirms the child has already exited. Panic payloads are not copied into the returned cleanup message or trace; Rust's normal panic hook is not globally replaced.

| Execution | Cleanup | Result |
| --- | --- | --- |
| success | success | original output |
| error | success | original error |
| success | error | cleanup error, successful output discarded |
| error | error | original error first, cleanup error appended |

Both completed cleanup outcomes are cached. Re-entering cleanup returns that result; Drop does not issue a second cleanup attempt. Drop still provides a fallback during unwinding. Error recording does not guarantee all resources were released when an underlying OS operation fails.

The existing `invoke()` maps transport errors to `EXECUTION_FAILED`; selected-engine metadata is preserved by its callers. The export path cannot decode/publish after a failed transport settlement. A private test callable exercises that exact staging/publication path. There is no public fault option or environment variable that injects failures.

The opt-in Windows trace retains previous fields and adds `cleanup_succeeded` and `cleanup_errors` at `cleanup_end`. A returned worker join and a successful join are different observations. Whole-Job accounting stays a snapshot, not an enforced exit guarantee.

## Regression evidence

Before applying the cleanup fix, a local test-only seam and exchange extraction retained the original Drop-based result behavior. Four tests executed: two controls passed and two assertions failed. A real test child returned output and exited; an owned thread panicked, but the caller still received success. In the early missing-stdout case the original error survived but the cleanup failure was lost. The original run is retained at `target/pr28-validation/before-cleanup-fix.log` in the development checkout; it is not claimed to be an untouched-main binary or a historical Windows reproduction.

After the fix, the same two regression tests pass. Additional tests cover all result combinations, cleanup-result caching, worker draining, unwind fallback, unchanged normal output and PNG publication rejection. The export fault test creates and validates a real RGBA8 staged PNG, then uses the actual process/cleanup path with a panicking owned worker. It verifies no final file, unchanged source and no staging residue; a racing writer's existing file also stays unchanged. It is controlled fault injection, not a real ag-psd/kernel failure.

The ordinary three-platform Rust gate runs these tests. Existing actual ag-psd and extracted-release tests, pixel fingerprints, parallelism and first-failure retention remain unchanged. Final exact-commit workflow results belong in the PR receipt; they are not predeclared by this document.

## Remaining gaps

- Windows still starts the process before private Job assignment. No startup race closure is claimed.
- Cleanup `wait()` and `join()` still block. There is no new hard cleanup deadline or cancellable-I/O implementation; a failed termination may still leave waits blocked.
- Whole-Job completion is not enforced; successful termination requests do not prove all descendants or external references are gone.
- Spawn/attach failures before `Running` exists retain their previous best-effort cleanup behavior.
- Tests inject an actual worker panic, not kernel-level Job termination/reap failure. Those API results are checked but OS-level fault injection is not claimed.
- Historical OS5 and 30-second inspect timeout are not attributed or closed by this change. The two-second rename retry budget is unchanged.

Next lifecycle work should separately establish creation-time containment and bounded process/I/O settlement, with its own controlled tests and budget contract. This increment must not be presented as full Windows reliability acceptance.

## Primary references

- Rust Child lifecycle and wait: https://doc.rust-lang.org/std/process/struct.Child.html
- Rust thread join: https://doc.rust-lang.org/std/thread/struct.JoinHandle.html
- Windows Job termination: https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-terminatejobobject
