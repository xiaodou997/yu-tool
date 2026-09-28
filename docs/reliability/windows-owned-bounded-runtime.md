# Windows creation-time ownership with bounded transport — PR #32

Base: `4a939e4b7c23622ce445aeb3a680c2678b0154ea`, the reviewed squash merge of #31. This independently adapts the startup contract from #29 (`c084c9edf6ece6af9f2f390ff6dfb3a21b744171`) to #31's nonblocking transport. It is not a merge of the old branch, a waiver of its failed Diagnostics7 run, or a public release. Issue #27 stays open.

## Joint contract

The Windows path prepares the private kill-on-close Job and the existing local byte pipes before `CreateProcessW`. `STARTUPINFOEXW` carries both `PROC_THREAD_ATTRIBUTE_JOB_LIST` and `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`. Only the three blocking engine-side clients are marked inheritable. Parent-side PIPE_NOWAIT endpoints, Job and process handles are not inherited. The attribute arrays and handles live through creation, and local client copies close before returning. Any creation/attribute/permission error is returned without a spawn-without-ownership retry. There is no production post-spawn `AssignProcessToJobObject` path.

The returned process interface offers `try_wait()` through a zero-time native wait and owned `kill()`, not an infinite waiter. `Running` separately owns the pipes. The #31 pump, byte limits, original operation deadline, stop-before-cleanup ordering, shared two-second child/Job observation deadline, cached outcome and no-publication-on-cleanup-error behavior remain. There are no I/O workers, detached reapers or pending overlapped buffers. Unix startup and transport keep their existing implementation.

The adapted wrapper retains explicit absolute native `.exe` and working-directory paths, shell-free CRT argument serialization, UTF-16 environment preservation with Node overrides removed, and checked ordinary DOS/UNC directory spelling for Node relative-module lookup. A verbatim path is never blindly stripped when doing so changes directory identity. No CLI flags, error codes, engine package versions or public capability contracts change. The Windows API requires Windows 10 / Server 2016 or newer; current hosted testing is not minimum-OS, desktop-machine, signing or notarization acceptance.

## Native failure found during joint acceptance

The first joint head `a2ae60e3a48840aa413ea92cdbe27bccc6acb916` reproduced the same failure in CI119 and Diagnostics10: Job ActiveProcesses was zero, but the retained early-descendant handle still returned WAIT_TIMEOUT rather than WAIT_OBJECT_0. The original one-second timeout/four-second outer assertions remain unchanged; this is not corrected by adding a wait to the test. See [first-run receipt](evidence/pr32-joint-first-run.json). The full three-pass probe did not run in that failed diagnostic attempt.

Production cleanup now captures at most128 current Job member IDs BEFORE termination, retains non-direct process handles with query/synchronize rights, and verifies each against the exact Job on that same handle. The direct child already has a stable owned handle. Only retained verified handles are polled; no PID-based process termination, global process scan or external unlocking is added. Accounting zero AND signaled retained members must both be observed within the same existing two-second cleanup deadline. Snapshot/query/identity failures, truncation and a changed TotalProcesses counter across capture/settlement fail closed; they cannot be converted into successful output. The original termination/pipe-close actions are still attempted when capture fails.

This is a bounded current-member snapshot, not a historical handle inventory: departed members before capture and arbitrary external references are not proven absent. A late member or unstable snapshot makes completion unconfirmed rather than expanding an unbounded search. Optional cleanup trace adds retained_member_count and retained_members_confirmed, with null/false on incomplete capture. Historical OS5 and30-second timeout attribution remains open.

## Combined tests, not two unrelated green checks

The Windows startup module contains eight substantive controls, one bookkeeping guard and two helper entries that are inert in ordinary discovery:

1. A parent and an immediately created descendant report membership in the exact test Job before request input. The descendant deliberately inherits output. A one-second exchange timeout is followed by closed local endpoints, confirmed direct exit and Job emptiness. A retained native descendant handle must already be signaled when the call returns within four seconds, well before the fixture's 20-second backstop.
2. The controlled legacy spawn-then-assign path reports non-membership before its explicit test-only assignment. No production fallback uses it.
3. A query-only Job refuses creation before fixture output; the full-rights control works with the same fixture. The refusal cannot leave an active process in that Job.
4. Literal arguments, Unicode/spaced cwd and exclusion of an unrelated inheritable test file survive real native startup. The same child then completes through nonblocking transport and bounded cleanup.
5. Two overlapping invocations are kept live in separate Jobs. Finishing the first neither waits for the second's pipe lifetime nor kills it; both eventually settle independently.
6. All 64 KiB request bytes reach a known-Job child before EOF; result and cleanup are checked together.
7. A real test-owned Job handle permits termination but lacks query rights. Accounting denial after a successful response must produce a cleanup failure rather than success. This is a native permission-control test, not a production fault flag or an ACL change.
8. A live process in a different test-owned Job is rejected by member retention; both processes remain untouched and subsequently finish normally. A separate guard rejects changed membership accounting instead of treating zero as universal completion.

All eight #31 retained-peer/backpressure/deadline tests remain enabled; the cases that launch an engine now reach the combined startup path on Windows. Standalone pipe/deadline cases keep their narrower assertions. #28/#31 execution-error, cleanup-cache, unwind and staged-PNG/no-clobber guards remain required. The independent Windows workflow executes startup controls and bounded controls separately before its unchanged three-pass extracted-CLI probe. Three-platform real .yu2 package and extracted candidate tests still gate delivery. Native outcomes belong in the exact-head PR receipt; they are not predeclared by this document.

## Remaining boundaries

Two-second cleanup polling is not a hard real-time deadline for process creation, OS/driver calls, filesystem/trace I/O or decoding. Job emptiness confirms accounting at observation time, not closure of all external file references. The handle whitelist scopes this creation call; it does not sandbox malicious engines or unrelated code using unrestricted handle inheritance. No external process is killed/unlocked and no privilege, ACL, antivirus or quarantine retry policy changes.

Original #29 Diagnostics7 failure (OS5 at round2/cycle10; artifact SHA256 `11fde0ca7f2e4eebc0c8a8acd02fabeb8353d2e6e0379fcdcb216b1c2fc53bfb`) remains evidence. Neither passing this joint contract nor failing to reproduce OS5 proves its historical cause. Keep old #29 unmerged with its branch and discussion intact until an explicit replacement/closure decision. Do not transplant its old blocking pipe/wait implementation onto #31. No Release/tag, public candidate promotion or root-cause closure is authorized by this PR.

See [acceptance](../testing/pr32-windows-owned-bounded-runtime.md), [ADR0010](../decisions/0010-windows-owned-bounded-runtime.md) and the historical [#31 boundary](bounded-process-io.md).

## Primary references

- https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute
- https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw
- https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-type-read-and-wait-modes
