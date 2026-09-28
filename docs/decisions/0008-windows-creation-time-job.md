# ADR 0008: Establish Windows engine Job ownership during process creation

Status: proposed for PR #29, pending review and actual Windows acceptance.

## Context

PR #28 makes completed cleanup failures visible but intentionally leaves startup-before-Job-assignment unchanged. The pinned stable Rust Command attribute-spawn API is still experimental. Moving to nightly, relying on undocumented resume APIs, or treating a post-spawn assignment as equivalent would add different risks.

## Decision

Use a small private stable-Rust Windows startup wrapper around documented CreateProcessW extended attributes. Create an unnamed kill-on-close Job first, provide JOB_LIST and an explicit three-standard-stream HANDLE_LIST, and fail closed when creation/attributes/permissions fail. Preserve explicit executable/working-directory selection, literal package arguments and Node environment sanitization. No shell or uncontained fallback is introduced.

Retain the current transport, synchronous I/O and #28 cleanup-result contract. The minimal native Child interface supplies owned handles, wait/try_wait/kill/status and stream endpoints; it does not define a new cleanup deadline. Unix stays on std Command. The new wrapper uses existing windows-sys version/features, not a new engine or heavy process framework.

## Consequences and acceptance

Windows-specific argument/environment serialization and handle lifetimes become our responsibility and need portable serialization tests plus real Windows startup controls. Test membership in a uniquely named private Job, not merely the runner's Job. Compare a controlled old-order case; verify immediate descendants, refused assignment permissions, literal argv/Unicode cwd and restricted handle inheritance. Re-run actual private-engine and extracted-CLI gates.

The APIs require Windows 10/Server 2016 or later; this decision is not minimum-OS certification or release approval. It does not address malicious engine escape, unrelated creators using unrestricted handle inheritance, eventual Job emptiness, external occupancy or historical OS5 attribution. Bounded process waiting and I/O cancellation remain a separate implementation/review. Issue #27, immutable evidence and public_release_ready=false remain in force.

See [scope and references](../reliability/windows-creation-job.md) and [test checklist](../testing/pr29-windows-creation-job.md).
