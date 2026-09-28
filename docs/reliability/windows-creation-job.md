# Windows creation-time Job ownership — PR #29

Base: PR #28 squash `62b022911dcca5bd06f04a6ca76d80202195520b`. Related open investigation: issue #27. This is **startup ownership only**, not bounded cleanup, historical root-cause closure or public-release approval. The unvalidated directory-query stash remains unapplied.

## Startup invariant

Production Windows startup creates an unnamed Job with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, builds the input/output endpoints and calls `CreateProcessW` with `EXTENDED_STARTUPINFO_PRESENT`, `CREATE_UNICODE_ENVIRONMENT`, `PROC_THREAD_ATTRIBUTE_JOB_LIST` and `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`. There is no production `AssignProcessToJobObject` call after spawn, retry without attributes, suspended-process resume workaround, elevation or external unlocker.

Job and handle arrays outlive the attribute list. The attribute allocation is aligned, bounded and owned; all pre-creation failures release local resources. Only the three child-side standard-stream handles enter the inheritance list; parent endpoints and the Job handle are non-inheritable. On success, the initial thread handle and local copies of child-side endpoints are closed before returning. The existing `Running` guard receives the already-owned process and Job and still governs execution/error cleanup. The process wrapper has no implicit successful fallback.

The whitelist limits what **this creation call** inherits. It is not a global promise about unrelated code concurrently making unrestricted inheriting CreateProcess calls, nor an executable security sandbox. Engine provenance still matters. Startup acceptance is not proof of every descendant's final exit or every external file handle being closed.

## Compatibility and serialization

Rust's raw process-attribute extension is nightly-only in the pinned stable toolchain. A private Windows wrapper therefore uses the existing windows-sys dependency; only Windows feature selections expand. No new dependency version, toolchain, protocol, CLI flag or engine package version is introduced. Unix stays on the existing std Command/process-group implementation.

The application path is an explicit absolute `.exe`, never inferred from a command string or PATH. argv[0] uses the executable-name rule and subsequent fixed package arguments use the Microsoft C-runtime quoting rules, preserving empty arguments, spaces, quotes, backslashes and UTF-16 without shell expansion. NUL and the documented 32767-unit command-line bound are checked before creation. This is not a general parser for cmd.exe/MSYS/custom command syntaxes.

The Windows environment snapshot retains ordering, hidden drive entries and UTF-16. All ASCII-case variants of NODE_OPTIONS and NODE_PATH are removed. No process-global environment changes or environment values in diagnostics are introduced. Copying is bounded at 1048576 UTF-16 units; an excessive block fails before startup rather than being truncated.

The startup working-directory spelling uses ordinary DOS/UNC syntax, since the private Node's relative main-script lookup failed when this first implementation directly supplied a canonical verbatim cwd. Conversion only handles drive/UNC prefixes, rejects components ending in dots/spaces and verifies the ordinary path canonicalizes to the same selected directory. Unsupported/changed spellings fail before process creation. It does not rename paths, follow a fallback engine or change the caller's working directory.

The Job-list attribute requires Windows 10/Server 2016 or newer. Passing current hosted Windows checks does not establish minimum OS/signing/notarization acceptance. API rejection is fail-closed; older platforms receive no uncontained fallback.

## Controlled tests

- A uniquely named test Job lets the child query **that exact Job**, rather than merely detecting GitHub's inherited runner Job. Both the immediate parent and its early descendant report membership before request input.
- A separate legacy-order control starts the same fixture, observes non-membership before assignment, then attaches and safely reaps it. Only this test contains post-spawn assignment. It proves the controlled ordering gap, not the cause of the historical OS5/30-second failures.
- A query-only handle for the test Job lacks assignment permission. Creation must fail with access denied without fixture output and without fallback; the full-rights handle then works as a positive control.
- A real child checks quoted argv, Unicode/spaced cwd, sanitized Node override state and non-inheritance of an unrelated test-created file handle. File identity is queried without reading contents, changing sharing, or closing foreign handles.
- Portable serialization tests cover empty arguments, escaping, bounds, environment preservation and rejected malformed data. Existing #28 cleanup regressions and PSD no-clobber/pixel/real-package checks remain active.

Two native helper tests return immediately in ordinary discovery; the four actual native startup cases run on Windows. The independent workflow also executes the startup module before its unchanged three-pass extracted-CLI probe. Exact-head results belong in the PR receipt, not predeclared here.

## Remaining independent work

### First-run implementation failures retained

The initial feature `4578f502` failed Windows test compilation because JOB_OBJECT_QUERY was imported from the wrong windows-sys module. Separately, the real private Node ran but its three actual-CLI suites failed at relative main-script resolution (`EISDIR`, lstat `C:`); no Windows candidate acceptance or manifest assembly was claimed. The independent startup module also stopped at compilation, before the three-pass probe. These are new implementation defects, not historical OS5/timeout reproductions. [The original failure summary](evidence/pr29-startup-first-run.json) and original workflow logs remain evidence. The correction uses SystemServices and verifies a non-verbatim child cwd plus rejection of ambiguous dot-suffix paths; subsequent exact-head results belong in the PR receipt.

Synchronous pipe operations, `wait()`/`join()` and cleanup termination ordering remain unchanged. No `CancelIoEx`, `CancelSynchronousIo`, asynchronous I/O, hard cleanup budget, wait-time relaxation or enforced whole-Job-empty condition is included. Those need a separate PR and failure/latency tests. The 1s negative cases, 30s operation default, 2s quarantine retry, concurrent tests, engine payload, M3 receipts and historical failure evidence are preserved. Startup ownership does not automatically close issue #27 or set public_release_ready/root_cause_fixed.

## Primary API references

- https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute
- https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw
- https://learn.microsoft.com/en-us/windows/win32/procthread/creating-a-child-process-with-redirected-input-and-output
- https://learn.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments
- https://doc.rust-lang.org/std/os/windows/process/trait.CommandExt.html
