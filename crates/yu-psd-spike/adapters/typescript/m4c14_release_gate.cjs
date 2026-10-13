"use strict";

// M4c-14 PSD MUTATION release review. Fail closed even if all
// self-reported devices are observed. This module only emits a
// research-readiness decision, NEVER changes public engine capabilities.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const assert=require("node:assert/strict");
const {check:checkDevice}=require("./m4c14_device_probe.cjs");
const ROOT=path.resolve(__dirname,"../../../../");
const BASE_SHA="c6f80b4beb6b51ec64904405942b43c5ec312426";
const DEVICE_FILE=path.join(ROOT,"docs/data/m4c14-local-device-v1.json");
const DECISION_FILE=path.join(ROOT,"docs/data/m4c14-psd-write-release-decision-v1.json");
const DEVICE_SCRIPT=path.join(__dirname,"m4c14_device_probe.cjs");
const M4C13=require(path.join(ROOT,"docs/data/m4c13-filesystem-matrix-v1.json"));
const M4C12=require(path.join(ROOT,"docs/data/m4c12-recovery-filesystem-v1.json"));
const M4C11=require(path.join(ROOT,"docs/data/m4c11-metadata-semantic-gate-v1.json"));
const M4C7E=require(path.join(ROOT,"docs/data/m4c7e-release-decision-v1.json"));
const sha=b=>crypto.createHash("sha256").update(b).digest("hex");
const PLATFORM_IDS=["darwin","win32","linux"];
const REQUIRED_TESTS=[
  "file.create_exclusive","file.fsync",
  "hardlink.same_device_inode","hardlink.no_clobber",
  "directory.fsync.syscall"
];
function refusal(message){
  const err=new Error(message);err.code="M4C14_EVIDENCE_REFUSED";throw err;
}
function assessCandidateDevice(candidate,expectedPlatform,knownScriptSha){
  if(!PLATFORM_IDS.includes(expectedPlatform))refusal("invalid platform requirement");
  if(candidate===null || candidate===undefined)
    return {platform:expectedPlatform,status:"PENDING_REAL_DEVICE_RECEIPT",
      claimed_receipt_present:false,independently_attested:false,
      accepted_for_release:false};
  checkDevice(candidate);
  if(candidate.host.platform!==expectedPlatform)
    refusal("device platform does not match required slot");
  if(candidate.source_baseline_sha!==BASE_SHA)
    refusal("wrong source revision for device experiment");
  if(candidate.probe_script_sha256!==knownScriptSha)
    refusal("device probe implementation hash differs from research contract");
  if(JSON.stringify(candidate.test_evaluated)!==JSON.stringify(REQUIRED_TESTS))
    refusal("required actual test suite not recorded");
  if(candidate.private_test_scope!=="new ignored target/m4c14-device-probe/run-* only")
    refusal("test scope not approved");
  if(candidate.hardware_attested!==false||
     candidate.power_loss_durability_certified!==false||
     candidate.production_write_authorized!==false)
    refusal("device receipt illegally upgrades authorization or attestation");
  const observed=candidate.observations;
  const supported=observed.hard_link_succeeded===true&&
    observed.hard_link_same_inode===true&&
    observed.source_destination_sha256_equal===true&&
    observed.second_link_existing_destination_denied===true&&
    observed.source_file_fsync_succeeded===true&&
    observed.parent_directory_fsync?.syscall_succeeded===true;
  return {
    platform:expectedPlatform,claimed_receipt_present:true,
    status:supported?
      "LOCAL_SYSCALLS_OBSERVED_INDEPENDENT_REVIEW_REQUIRED":
      "DEVICE_CAPABILITY_NOT_ESTABLISHED",
    independently_attested:false,
    accepted_for_release:false,
    checks_observed:supported,
    filesystem_opaque_type:candidate.actual_filesystem.statfs_type,
    source_revision:BASE_SHA,
    probe_sha256:knownScriptSha,
    pending:"independent real device provenance + filesystem-specific acceptance + power-loss & metadata review"
  };
}
function enforceExistingDenials(){
  if(M4C7E.public_psd_write_authorized!==false||
    M4C7E.decision!=="PSD_PRODUCTION_MUTATION_BLOCKED")
    refusal("M4c-7e production denial changed");
  if(M4C13.summary.real_windows_linux_device_receipts!==0 ||
    M4C13.summary.physical_power_cut_tests!==0 ||
    M4C13.summary.production_writes_authorized!==0)
    refusal("old M4c-13 platform receipts unexpectedly upgraded");
  if(M4C12.summary.power_failure_durability_verified!==false ||
    M4C12.summary.production_writes_authorized!==0)
    refusal("old M4c-12 durability denial changed");
  if(M4C11.summary.cases_with_unknown_semantics!==2 ||
    M4C11.summary.public_writes_authorized!==0)
    refusal("unknown PSD metadata prematurely waived");
}
function evaluate({localReceipt,additionalDeviceReceipts={}}){
  enforceExistingDenials();
  const hash=sha(fs.readFileSync(DEVICE_SCRIPT));
  if(!localReceipt)refusal("missing real local device receipt");
  if(localReceipt.host.platform!=="darwin")
    refusal("this decision snapshot requires locally measured macOS APFS baseline");
  // This is only a local, self-reported receipt, NOT authenticated
  // independent authority for Windows, Linux or external devices.
  const mac=assessCandidateDevice(localReceipt,"darwin",hash);
  const platformReceipts={
    darwin:mac,
    win32:assessCandidateDevice(additionalDeviceReceipts.win32??null,"win32",hash),
    linux:assessCandidateDevice(additionalDeviceReceipts.linux??null,"linux",hash)
  };
  const allClaimed=Object.values(platformReceipts).every(x=>x.claimed_receipt_present);
  const blockers=[
    {id:"device_windows_real",status:platformReceipts.win32.status},
    {id:"device_linux_real",status:platformReceipts.linux.status},
    {id:"fs_cross_platform_verified",status:"BLOCKED_MULTIPLE_REAL_FILESYSTEM_DEVICE_ACCEPTANCES_MISSING"},
    {id:"crash_power_loss_and_directory_durability",
      status:"BLOCKED_REAL_POWER_LOSS_PROOF_NOT_AVAILABLE"},
    {id:"full_psd_metadata_feature_fidelity",
      status:"BLOCKED_EXIF_XMP_SHMD_AND_UNKNOWN_SEMANTICS"},
    {id:"source_concurrency_and_customer_output_recovery",
      status:"BLOCKED_PRODUCTION_TRANSACTION_UNVERIFIED"},
    {id:"security_release_decision",status:"BLOCKED_MANUAL_RELEASE_GATE_NOT_ACCEPTED"}
  ];
  return {
    schema_version:"1",milestone:"M4c-14",
    operation:"PSD mutation release decision and cross-platform device evidence review",
    source_baseline_sha:BASE_SHA,
    public_psd_write_authorized:false,
    status:"RESEARCH_HANDOFF_COMPLETE_PSD_PRODUCTION_WRITE_BLOCKED",
    platform_receipts:platformReceipts,
    gates:{
      research_only_local_mac_filesystem_calls_observed:
        mac.checks_observed&&mac.claimed_receipt_present,
      all_required_platform_receipts_submitted:allClaimed,
      independent_device_provenance_review_accepted:false,
      actual_power_loss_durability_accepted:false,
      arbitrary_psd_exif_xmp_shmd_fidelity_accepted:false,
      production_output_source_concurrency_accepted:false,
      manual_production_release_accepted:false
    },
    outstanding:blockers,
    summary:{
      verified_self_reported_local_mac_platform_receipts:mac.checks_observed?1:0,
      windows_linux_real_runner_receipts_accepted:0,
      real_power_loss_test_receipts:0,
      unresolved_metadata_feature_categories:["EXIF","XMP","shmd/cust","unknown resources"],
      unaccepted_release_gates:blockers.length,
      production_writes_authorized:0
    },
    decision:"PSD_PRODUCTION_MUTATION_BLOCKED",
    warning:"Self-reported OS/filesystem receipts must be independently witnessed and reviewed; no receipt by itself authorizes PSD writing."
  };
}
function checkDecision(r){
  assert.equal(r.schema_version,"1");
  assert.equal(r.milestone,"M4c-14");
  assert.equal(r.source_baseline_sha,BASE_SHA);
  assert.equal(r.public_psd_write_authorized,false);
  assert.equal(r.decision,"PSD_PRODUCTION_MUTATION_BLOCKED");
  assert.equal(r.summary.production_writes_authorized,0);
  assert.equal(r.summary.windows_linux_real_runner_receipts_accepted,0);
  assert.equal(r.gates.manual_production_release_accepted,false);
  assert.equal(r.gates.arbitrary_psd_exif_xmp_shmd_fidelity_accepted,false);
  assert.ok(r.outstanding.length>=5);
  assert.ok(Object.values(r.platform_receipts).every(x=>x.accepted_for_release===false));
}
if(require.main===module){
  const mode=process.argv[2];
  if(mode==="--write"&&process.argv.length===3){
    const local=JSON.parse(fs.readFileSync(DEVICE_FILE,"utf8"));
    const decision=evaluate({localReceipt:local});
    checkDecision(decision);
    fs.writeFileSync(DECISION_FILE,JSON.stringify(decision,null,2)+"\n");
    console.log(JSON.stringify({mode,decision:decision.decision,
      local_mac_receipts:decision.summary.verified_self_reported_local_mac_platform_receipts,
      windows:decision.platform_receipts.win32.status,
      linux:decision.platform_receipts.linux.status,
      blocked_gates:decision.outstanding.length}));
  }else if(mode==="--check"&&process.argv.length===3){
    const data=JSON.parse(fs.readFileSync(DECISION_FILE,"utf8"));
    checkDecision(data);
    console.log(JSON.stringify({mode,offline:true,decision:data.decision,
      remaining_gates:data.summary.unaccepted_release_gates}));
  }else{
    process.stderr.write("usage: node m4c14_release_gate.cjs --write|--check\n");
    process.exitCode=2;
  }
}
module.exports={BASE_SHA,assessCandidateDevice,evaluate,checkDecision,enforceExistingDenials};
