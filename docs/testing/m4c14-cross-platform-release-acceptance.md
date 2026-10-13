# M4c-14 — Cross-Platform Device Acceptance and PSD Release Review

**Parent:** `develop@c6f80b4beb6b51ec64904405942b43c5ec312426`

**Frozen `main`:** `5526dc4465c4e79ad7f2757d06b3b1693642b8af`

**Result:** research handoff prepared; PSD production mutation **BLOCKED**.

## Device evidence collected in this stage

- [x] Runner fleet checked: **one** connected online Runner, macOS arm64.
- [x] Local device Node.js 22.23.3 filesystem experiment executed in
      private `target/m4c14-device-probe/run-*` only.
- [x] Local hard-link/no-clobber, inode, file fsync and directory fsync
      syscall behavior **observed**.
- [x] Exact checkout source SHA and probe-script SHA-256 recorded.
- [x] Windows native NTFS receipt: **PENDING**, no accessible Runner.
- [x] Linux native filesystem receipt: **PENDING**, no accessible Runner.
- [x] Real storage power loss: **NOT TESTED**.
- [x] macOS read-only NTFS and mounted NFS: **no write attempted**.
- [x] No self-reported receipt treated as independent device proof.

## Device-handoff runbook (only when a real device is available)

Use an isolated checkout of the **same reviewed M4c-14 source commit**,
the pinned Node.js **22.23.3**, and a local disk with a disposable
research directory. Do not pass any user PSD. The probe writes only a
small non-PSD test file under the checkout's ignored `target/`.

```text
# On the actual Windows or Linux host:
git rev-parse HEAD
node --version   # must be v22.23.3
node crates/yu-psd-spike/adapters/typescript/m4c14_device_probe.cjs --write

# Copy the exact receipt_path reported above into an independently
# reviewed evidence package, retaining its hash and source revision.
node crates/yu-psd-spike/adapters/typescript/m4c14_device_probe.cjs \
  --check <the-exact-receipt-path>
```

**Required human verification** of each future receipt:
actual device OS, architecture, checkout commit, probe implementation
hash, volume/filesystem identity, file/link/no-clobber errors, parent
directory sync result, lack of customer data writes and independently
verified origin of the receipt. `statfs` numeric type alone is not
a filesystem name; use native OS evidence where necessary.

Failed hard-link or directory-sync observations are **failure
evidence**, not a reason to substitute a non-atomic copy/rename.
Each tested filesystem needs its own acceptance decision.

Do not perform real destructive power-cut tests on working computers,
external user volumes or shared storage. Any power-failure
qualification requires separately provisioned disposable test
storage and an explicit safety plan.

## Release decision tests (do not enable writing)

```bash
NODE22=target/m4c14-node22/node_modules/node/bin/node

# macOS research host only, records an actual bounded local syscall run:
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c14_device_probe.cjs --write

# Reconcile known missing device / metadata evidence into a BLOCKED
# machine decision, independent of hypothetical future receipts:
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c14_release_gate.cjs --write

# Offline-only checks:
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c14_device_probe.cjs --check
"$NODE22" crates/yu-psd-spike/adapters/typescript/m4c14_release_gate.cjs --check

"$NODE22" --test \
  crates/yu-psd-spike/adapters/typescript/m4c14_device_release.test.cjs
```

- [x] Device/release negative suite: **11 passed, 0 failed**.
- [x] Full M4c-3 through M4c-14 pinned Node.js 22.23.3 PSD
      research suite: **120 passed, 0 failed**.
- [x] Rust `cargo test --workspace --locked -- --test-threads=1`:
      **174 passed, 0 failed** (local macOS).
- [x] Clippy `cargo clippy --workspace --all-targets --locked -- -D warnings`:
      passed.
- [x] Rustfmt, JavaScript syntax, source contract hashes,
      offline device/release evidence and Git whitespace checks:
      passed.

## Unaccepted product gates

- [ ] Native Windows and Linux independently witnessed device runs.
- [ ] Supported filesystem matrix beyond the one local macOS device.
- [ ] Actual power-loss durability and source-version concurrency.
- [ ] Full PSD metadata/Photoshop semantics and preservation of
      unknown resources.
- [ ] Explicit release approval and a separate production PSD writer.

**`PSD_PRODUCTION_MUTATION_BLOCKED` remains mandatory.**

See [ADR 0030](../decisions/0030-m4c14-cross-platform-psd-write-release-gate.md),
[macOS device receipt](../data/m4c14-local-device-v1.json) and
[machine-readable release decision](../data/m4c14-psd-write-release-decision-v1.json).
