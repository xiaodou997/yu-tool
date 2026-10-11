# ADR 0020 — M4c-7c Native Photoshop Readiness and Saved-Pair Byte Gate

**Status:** Guardrails implemented; native Photoshop save/reopen **BLOCKED**

**Date:** 2026-10-11

**Base:** `develop@930e1d83068926130a22274f618390e70c5f52f8`

**v0.1 protected:** `main@5526dc4465c4e79ad7f2757d06b3b1693642b8af`

## Question and observed result

Can the only eligible M4c-7a sample, the exact-hash nested group
`Group 1 → Group 2`, pass a paired *native Photoshop* saveAs/reopen
experiment with the unchanged original saved as a control?

**Not on this device in the current native application state.** Adobe
Photoshop 2026 is running, but a bounded AppleScript document-count query
continues to time out. The System Events application-process lookup
works, while a window inspection via System Events reports that
`osascript` lacks accessibility access (macOS error `-1728`).
Accessibility denial prevents confirming whether Photoshop has a blocking
dialog; it is **not proven** to be the cause of the Apple Events timeout.

**No Photoshop save was attempted in this guarded run.** The M4c-7c
[blocked receipt](../data/m4c7c-photoshop-control-v2.json) records
`native_preflight.status=apple_events_unresponsive`,
`save_command_attempted=false` and
`paired_research_save_reopen_passed=0`. It supersedes neither the
historical M4c-7 receipt nor the four-fixture M4c-6 read-only evidence.

## Engineering changes

### Native preflight

[`m4c7c_native_readiness.cjs`](../../crates/yu-psd-spike/adapters/typescript/m4c7c_native_readiness.cjs)
checks that the Photoshop 2026 process is already running before issuing
an Apple Event. A bounded document-count query classifies:

- application not running;
- Apple Events unresponsive / automation denied / unexpected error;
- application responsive with **existing documents open** (must refuse);
- application responsive with **zero documents** (research may proceed).

It does **not** launch, restart, force-quit, foreground, click in, or
change permissions for Photoshop. Its query has a bounded timeout.
The paired save probe repeats the preflight directly before each saveAs
operation, beyond the initial check.

### Stronger paired-output verification

[`m4c7c_saved_pair_gate.cjs`](../../crates/yu-psd-spike/adapters/typescript/m4c7c_saved_pair_gate.cjs)
independently compares two **hypothetical successfully saved** PSD/PSB
buffers: the Photoshop-saved unchanged control and the Photoshop-saved
name-patched candidate. This is separate from Photoshop's own name
inspection and the independent `psd-tools` pixel comparison.

The strict comparator requires:

1. same document header, geometry and raw logical record count;
2. identical Image Resources and compressed merged composite;
3. identical layer channel bytes and non-target tagged metadata;
4. exactly one target `luni` difference and one matching ASCII Pascal
   name difference, with all other raw layer properties unchanged;
5. exact equality of file length and **every byte outside the identified
   target Unicode/legacy name ranges**.

A saved Photoshop document may legitimately undergo re-encoding or
metadata normalization; this research gate intentionally **does not
assume such changes are safe**. If independent control/candidate saves
cannot meet strict byte preservation, the operation is rejected pending
a narrower format-specific explanation and independent editor evidence.

Synthetic control/patched-fixture tests demonstrate successful
classification of only name-byte changes and rejection of unexpected
resource, channel and other metadata modifications. These tests do
**not substitute** for a real Photoshop saved-pair receipt.

### Publication boundary

The experiment accepts only the M4c-7a exact reviewed group fixture.
Its inputs and potential outputs are isolated in fresh ignored
`target/` run directories. Output existence is checked before native
saveAs, and the experiment does not replace a source or publish a PSD.
The absence check alone is **not atomic compare-and-swap**, so the
experiment must never be repurposed as a production writer.

Native saveAs timeouts after the command has started remain
**inconclusive**, since the application could finish work after the
caller returns. The receipt now records whether a save command was
attempted and whether outputs exist on a failure path. No automatic
promotion of a partial receipt is allowed.

## Exit decision

M4c-7c **remains BLOCKED**. No user action is automatically performed
to resolve macOS Accessibility/Automation permissions or force-close
Photoshop. The next native acceptance attempt requires a responsive
Photoshop 2026 session with no open documents and an explicitly
re-invoked scratch-only experiment.

Even a fully successful saved-pair study does **not** authorize a
public `yu psd rename` command without an additional production
release decision, feature-level allow-list, source-version binding,
staged no-clobber publication and a verified output receipt.

See [local acceptance record](../testing/m4c7c-native-gate-and-saved-pair.md).
