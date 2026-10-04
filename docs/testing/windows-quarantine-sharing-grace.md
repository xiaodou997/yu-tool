# Windows quarantine sharing grace acceptance

## Required controls

- [ ] portable fmt/check/Clippy/workspace tests pass;
- [ ] sharing errors 5 / 32 / 33 remain the only retryable Windows rename errors;
- [ ] unrelated errors still fail on the first attempt;
- [ ] a nested regular-file holder that remains active beyond 2 seconds but releases inside the 2.5-second window allows the same atomic directory rename to succeed;
- [ ] a holder that remains active through the full production window still returns failure and preserves the source version;
- [ ] no copy/delete fallback, external unlock or unrelated-process termination is introduced.

## Hosted Windows gates

- [ ] Windows Lifecycle resource calibration passes;
- [ ] creation-time Job and bounded transport controls pass;
- [ ] three diagnostic repetitions pass without failed-case retries;
- [ ] Managed Package Windows held-file preservation control passes;
- [ ] debug and extracted-candidate ten-cycle lifecycle passes;
- [ ] isolated candidate acceptance passes.

Historical RC Freeze run 36822712886 remains failed evidence and must not be replaced by a rerun of the same failed case.

## Delivery

Successful validation permits integrating the bounded mitigation and restarting PR #38 from the new main. It does not itself authorize v0.1 publication or close Issue #27.
