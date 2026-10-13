"use strict";

const {test}=require("node:test");
const assert=require("node:assert/strict");
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const {check:checkDevice,actualFilesystemKind}=
  require("./m4c14_device_probe.cjs");
const {assessCandidateDevice,evaluate,checkDecision,BASE_SHA}=
  require("./m4c14_release_gate.cjs");
const ROOT=path.resolve(__dirname,"../../../../");
const LOCAL=require(path.join(ROOT,"docs/data/m4c14-local-device-v1.json"));
const RELEASE=require(path.join(ROOT,"docs/data/m4c14-psd-write-release-decision-v1.json"));
const sha=b=>crypto.createHash("sha256").update(b).digest("hex");
const scriptHash=sha(fs.readFileSync(path.join(__dirname,"m4c14_device_probe.cjs")));
const clone=x=>JSON.parse(JSON.stringify(x));

test("real local macOS device receipt reports bounded fs syscall observations but NOT authority",()=>{
  checkDevice(LOCAL);
  assert.equal(LOCAL.source_baseline_sha,BASE_SHA);
  assert.equal(LOCAL.host.platform,"darwin");
  assert.equal(LOCAL.host.arch,"arm64");
  assert.equal(LOCAL.host.node_version,"22.23.3");
  assert.equal(LOCAL.probe_script_sha256,scriptHash);
  assert.equal(LOCAL.observations.hard_link_succeeded,true);
  assert.equal(LOCAL.observations.hard_link_same_inode,true);
  assert.equal(LOCAL.observations.second_link_existing_destination_denied,true);
  assert.equal(LOCAL.observations.source_file_fsync_succeeded,true);
  assert.equal(LOCAL.observations.parent_directory_fsync.syscall_succeeded,true);
  assert.equal(LOCAL.hardware_attested,false);
  assert.equal(LOCAL.power_loss_durability_certified,false);
  assert.equal(LOCAL.production_write_authorized,false);
});
test("device statfs type is opaque and cannot be falsely named APFS, NTFS or ext4",()=>{
  const result=actualFilesystemKind({type:1234,bsize:4096});
  assert.deepEqual(result,{statfs_type:"1234",statfs_block_size:4096});
  assert.equal(Object.hasOwn(result,"filesystem_name"),false);
  assert.equal(Object.hasOwn(LOCAL.actual_filesystem,"filesystem_name"),false);
});
test("release decision records Windows Linux as PENDING without fabricated runtime receipts",()=>{
  checkDecision(RELEASE);
  assert.equal(RELEASE.decision,"PSD_PRODUCTION_MUTATION_BLOCKED");
  assert.equal(RELEASE.platform_receipts.win32.status,"PENDING_REAL_DEVICE_RECEIPT");
  assert.equal(RELEASE.platform_receipts.linux.status,"PENDING_REAL_DEVICE_RECEIPT");
  assert.equal(RELEASE.platform_receipts.win32.claimed_receipt_present,false);
  assert.equal(RELEASE.platform_receipts.linux.claimed_receipt_present,false);
  assert.equal(RELEASE.gates.all_required_platform_receipts_submitted,false);
  assert.equal(RELEASE.summary.windows_linux_real_runner_receipts_accepted,0);
  assert.equal(RELEASE.summary.real_power_loss_test_receipts,0);
  assert.equal(RELEASE.summary.unaccepted_release_gates,7);
  assert.equal(RELEASE.summary.production_writes_authorized,0);
});
test("only the exact device probe source revision and implementation can be considered for review",()=>{
  const a=clone(LOCAL);a.source_baseline_sha="0".repeat(40);
  assert.throws(()=>assessCandidateDevice(a,"darwin",scriptHash));
  const b=clone(LOCAL);b.probe_script_sha256="f".repeat(64);
  assert.throws(()=>assessCandidateDevice(b,"darwin",scriptHash),/implementation hash/);
  const c=clone(LOCAL);c.executing_checkout_sha="bad";
  assert.throws(()=>assessCandidateDevice(c,"darwin",scriptHash));
});
test("a platform slot rejects wrong OS and unsupported OS claims",()=>{
  assert.throws(()=>assessCandidateDevice(LOCAL,"win32",scriptHash),
    /platform does not match/);
  assert.throws(()=>assessCandidateDevice(LOCAL,"plan9",scriptHash),
    /invalid platform/);
});
test("missing or abbreviated required real syscall test set is rejected",()=>{
  const a=clone(LOCAL);a.test_evaluated.pop();
  assert.throws(()=>assessCandidateDevice(a,"darwin",scriptHash),
    /test suite not recorded/);
  const b=clone(LOCAL);b.private_test_scope="other-volume";
  assert.throws(()=>assessCandidateDevice(b,"darwin",scriptHash),
    /scope not approved/);
});
test("forgeable self-reported hardware attestation/authorization cannot become approved device evidence",()=>{
  for(const key of ["hardware_attested","power_loss_durability_certified",
    "production_write_authorized"]){
    const a=clone(LOCAL);a[key]=true;
    assert.throws(()=>assessCandidateDevice(a,"darwin",scriptHash));
  }
  assert.equal(RELEASE.gates.independent_device_provenance_review_accepted,false);
  assert.equal(RELEASE.gates.manual_production_release_accepted,false);
});
test("device observed unsupported hard link or failed directory sync cannot count as acceptance",()=>{
  const link=clone(LOCAL);
  link.observations.hard_link_succeeded=false;
  link.observations.hard_link_same_inode=false;
  link.observations.hard_link_error_code="ENOTSUP";
  assert.equal(assessCandidateDevice(link,"darwin",scriptHash).status,
    "DEVICE_CAPABILITY_NOT_ESTABLISHED");
  const sync=clone(LOCAL);
  sync.observations.parent_directory_fsync={syscall_succeeded:false,error_code:"EINVAL"};
  assert.equal(assessCandidateDevice(sync,"darwin",scriptHash).status,
    "DEVICE_CAPABILITY_NOT_ESTABLISHED");
});
test("synthetic Windows/Linux-shaped RECEIPTS in a UNIT TEST never grant a production release",()=>{
  // These are deliberately invented, in-memory NEGATIVE fixtures. They
  // are not written to a device receipt file and are NOT real device tests.
  const mockWin=clone(LOCAL);mockWin.host.platform="win32";
  const mockLinux=clone(LOCAL);mockLinux.host.platform="linux";
  const result=evaluate({localReceipt:LOCAL,additionalDeviceReceipts:{
    win32:mockWin,linux:mockLinux
  }});
  assert.equal(result.gates.all_required_platform_receipts_submitted,true);
  assert.equal(result.gates.independent_device_provenance_review_accepted,false);
  assert.equal(result.gates.manual_production_release_accepted,false);
  assert.equal(result.decision,"PSD_PRODUCTION_MUTATION_BLOCKED");
  assert.equal(result.summary.windows_linux_real_runner_receipts_accepted,0);
  assert.equal(result.platform_receipts.win32.accepted_for_release,false);
  assert.equal(result.platform_receipts.linux.accepted_for_release,false);
});
test("missing local device receipt cannot be fabricated by an aggregate gate",()=>{
  assert.throws(()=>evaluate({localReceipt:null}),/missing real local device/);
  const actual=evaluate({localReceipt:LOCAL});
  assert.equal(actual.gates.research_only_local_mac_filesystem_calls_observed,true);
  assert.equal(actual.gates.actual_power_loss_durability_accepted,false);
  assert.equal(actual.gates.arbitrary_psd_exif_xmp_shmd_fidelity_accepted,false);
  assert.equal(actual.gates.production_output_source_concurrency_accepted,false);
});
test("prior frozen PSD research denials remain intact and do not get a release from FS receipts",()=>{
  const m4c13=require(path.join(ROOT,"docs/data/m4c13-filesystem-matrix-v1.json"));
  const metadata=require(path.join(ROOT,"docs/data/m4c11-metadata-semantic-gate-v1.json"));
  const old=require(path.join(ROOT,"docs/data/m4c7e-release-decision-v1.json"));
  assert.equal(m4c13.summary.real_windows_linux_device_receipts,0);
  assert.equal(m4c13.summary.physical_power_cut_tests,0);
  assert.equal(metadata.summary.cases_with_unknown_semantics,2);
  assert.equal(old.decision,"PSD_PRODUCTION_MUTATION_BLOCKED");
  assert.equal(RELEASE.gates.arbitrary_psd_exif_xmp_shmd_fidelity_accepted,false);
  assert.ok(RELEASE.summary.unresolved_metadata_feature_categories.includes("shmd/cust"));
});
