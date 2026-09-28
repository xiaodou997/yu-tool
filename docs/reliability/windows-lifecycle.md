# Windows process, pipe and removal investigation — PR #26

Status: **diagnostic/reproduction foundation; root-cause repair not accepted**.

Base: PR #25 squash `bf7509b1997e1efd3ff8e28deda92337da3880fd`. This is a separate workstream from v0.1 distribution. No release/tag is created and no developer candidate is promoted.

## Preserve the three distinct observations

| Evidence | Observation | What is not established |
| --- | --- | --- |
| Managed Package14 / 36367957120, attempt1, Windows job108758042352 | Read-only completion followed by removal quarantine OS5 despite the two-second budget | Handle owner, permission cause, or a process-tree leak |
| Managed Package15 / 36372332298, Windows job108771070567, feature0af8f96 | Extracted release CLI repeated-lifecycle inspect hit30s; other read-only/pixel tests passed | Exact cycle/partial output; earlier instrumentation did not capture these |
| Managed Package16 / 36373168210, Windows job108773503170, feature329efeb | Ten debug and ten extracted-release cycles passed | A finite pass does not supersede either failure or prove a root-cause fix |

Keep M3 and #25 evidence immutable. Every subsequent run records its own checkout, runner image, inputs, steps and failure. Do not label a diagnostics-only change as a repair.

## First independent Windows result: failure retained

Windows diagnostic run `36375481917`, attempt 1, tested feature `2e162c1da219b2eae3bc5870322aab396ee8f704` via checkout `96eba61ebe6dce8869e539139ec6985d5a783232`. All five native cases passed. In the first real-package pass, read-only and independent pixel tests passed, but repeated lifecycle cycle 2 failed at removal: OS 5 after 62 quarantine attempts / 2017ms. Cycle 1 had completed. Passes 2 and 3 were not executed; the first failure stopped the probe as intended.

The original report/log ZIP was retrieved and its Actions-wrapper SHA-256 verified. [Immutable failure summary](evidence/pr26-windows-run1.json) records provenance, input hashes and exact observations. Source-present/read-only/current-directory observations do not identify an owner or prove ACL correctness. Do not infer that the failure was caused by the reader threads merely because this workstream also tests EOF.

The first report parser counted only successful suite summaries and therefore displayed zero passed for the failed suite. The raw log establishes **2 passed / 1 failed**. A reporting-only correction now counts failed-suite results, rejects contradictory failure summaries even with exit 0, and recognizes uppercase Windows `IMAGEVERSION`. It does not change runtime behavior, deadlines, retries or mark the fault fixed. Standard CI106 and Managed Package17 passed on the observed feature; they do not override this independent failure.

## First slice: observation and controlled cases

Timeout messages retain existing codes/deadlines and add `phase`, `stdout_bytes` and `stderr_bytes`. `phase=process_exit` means the direct child was not observed exited; `input_completion` means input-worker completion was not observed after child exit; `pipe_eof` means an output reader is unfinished after child exit/input completion. Counts are observed bytes, not document content. These are asynchronous snapshots, not lock-owner diagnoses. Full valid JSON alone is not process/pipe completion.

Native CLI cases cover:

1. A complete valid response while the parent remains alive: must timeout, not accept a response early.
2. A completed parent with an output-pipe-owning descendant: must timeout with a pipe phase rather than wait forever.
3. A fragmented valid response: must collect the complete response normally.
4. A quiet descendant with a test-owned working directory: inspect followed by immediate deactivate/remove must clean the version.
5. Windows-only external directory occupancy: hold a test-created directory handle without delete sharing, verify the real public removal command fails closed and preserves installation metadata, release that test handle, then verify removal succeeds.

The finite-lived helper exits on its own after 20s if cleanup regresses; negative tests still require completion within 8s with the unchanged 1s requested deadline. They run under the existing parallel test execution. The inherited-pipe fixture does not wait for the descendant's user code to signal readiness: handle inheritance is established by successful spawn, and a readiness wait would conflate cold startup with pipe completion.

These tests exercise the public CLI/installer and existing process control. The two strict one-second response/EOF probes preflight the exact installed native fixture once before measurement; otherwise a cold-start delay can prevent the intended response state from being reached at all. This preflight is never applied to the existing cold-start/real-package cycles, never retries a failed case, and does not extend any engine deadline. They do not inspect or unlock unrelated applications. Cross-platform directory removal alone is not proof of all descendant termination; Windows occupancy is a separate test.

## Independent Windows diagnostic gate

`.github/workflows/windows-lifecycle.yml` builds the locked private engine and a developer candidate, then invokes `tools/run_lifecycle_probe.py` against the extracted executable. Each of three passes runs all native cases and all three real-package tests, including ten full install/activate/use/deactivate/remove cycles. Thus a complete Windows run has six successful suite steps, 15 native case executions, and 30 repeated real lifecycle cycles, in addition to the read-only and independent pixel checks.

Any failed suite/timeout/zero-test run stops that invocation. It is not retried. `report.json` and per-step original logs are uploaded with `if: always()`; an earlier failed run remains evidence even when another run later passes. The independent gate does not replace the original three-platform standard/package gates. Reports and candidate metadata always retain `public_release_ready: false` and `root_cause_fixed: false`.

A timeout of the outer Cargo command is separately recorded as `outer_command_timeout`, not an engine30s timeout. A workflow setup failure before the probe is a setup failure with no executed-test receipt. Job cancellation/power loss are not claimed to yield complete diagnostics.

## Follow-up: scoped occupancy evidence

[Windows occupancy capture](windows-occupancy-capture.md) adds opt-in owned-Job snapshots and a test-harness hook that samples resources after a failed removal but before test-root teardown. The collector retains bounded file-user/process observations, explicit directory-handle blind spots, and PID + creation-time correlation. It does not alter native process-control order or retry/timeout policy, and does not close any historical root cause. The added observer has timing overhead; uninstrumented package acceptance remains separate.

## Unimplemented native-control follow-up

Review identified two implementation gaps worth controlled reproduction: the process currently starts before assignment to its private Windows Job, and cleanup issues termination then waits for the direct child/readers without explicitly observing the whole Job becoming empty. **This slice does not change process creation, Job assignment, termination or cleanup scheduling.** A native-control patch remains separate work; no claim is made that either gap caused the recorded OS5 or30s timeout.

Next acceptance should establish ownership before engine code executes, explicit bounded process-tree/reader cleanup outcome, and failure propagation before successful artifact publication. Verify both the success and error paths, retained pipe handles, non-pipe descendants, and persistent external file occupancy. Do not weaken existing deadlines, disable concurrency, add blanket retries, or automatically terminate a suspected external handle owner.

Primary API references for the investigation:
- https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-assignprocesstojobobject
- https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-terminatejobobject
- https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess

See [acceptance checklist](../testing/pr26-windows-lifecycle.md) and [v0.1 release boundary](../releasing/v0.1-readiness.md).
