# Windows quarantine sharing settle grace

This release-blocking increment follows the spontaneous RC Freeze reproduction recorded in Issue #27 and PR #38.

## Evidence

During the failed Windows managed lifecycle remove:

- quarantine rename failed with OS error 5 after 75 attempts / 2002 ms;
- exact-directory sampling observed no directory user;
- in-flight regular-file sampling identity-confirmed one external process for `runtime/node.exe` from the first sample through about 2125 ms;
- the same PID + process creation FILETIME produced a positive singleton `runtime/node.exe` witness in all 15 positive samples;
- the relationship had disappeared by the immediate post-failure collector;
- Yu-owned Job cleanup was already complete.

Restart Manager does not expose the observed handle's DELETE-share flags, so this is strong temporal/file attribution rather than causal handle proof.

## Change

On Windows only, retain the same atomic directory rename and the same retryable sharing-class errors (5 / 32 / 33), but increase the bounded rename window from 2000 ms to **2500 ms**.

The extra 500 ms is an evidence-backed settle grace. It does not:

- retry unrelated errors;
- copy/delete the original version;
- unlock or terminate external processes;
- change the engine process timeout;
- serialize test execution;
- turn cleanup failure into success;
- change release-readiness flags.

A persistent holder still fails after the bounded window and the original installed version remains intact.

## Release boundary

This change is a mitigation for the observed external-file-holder timing window. It does not set `root_cause_fixed=true` merely because finite validation passes. The RC Freeze must restart from the eventual merged fix commit and pass exact-source gates.
