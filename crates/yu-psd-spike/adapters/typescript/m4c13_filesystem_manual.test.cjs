"use strict";

const {test}=require("node:test");
const assert=require("node:assert/strict");
const fs=require("node:fs");
const path=require("node:path");
const {inspectMounts,check:checkMatrix}=require("./m4c13_filesystem_matrix.cjs");
const {classify,inspect,check:checkManual}=require("./m4c13_manual_recovery.cjs");
const {newRun,attempt,recovery}=require("./m4c12_recovery_fs_gate.cjs");
const ROOT=path.resolve(__dirname,"../../../../");
const MATRIX=require(path.join(ROOT,"docs/data/m4c13-filesystem-matrix-v1.json"));
const MANUAL=require(path.join(ROOT,"docs/data/m4c13-manual-recovery-v1.json"));

function inRun(fn){
  const dir=newRun();
  try{return fn(dir);}finally{fs.rmSync(dir,{recursive:true,force:true});}
}
function target(dir){return path.join(dir,"renamed-research.psd");}
function txn(dir){return path.join(dir,fs.readdirSync(dir).find(x=>x.startsWith(".txn-")));}

test("machine snapshot confirms real APFS research test without implying power-loss survival",()=>{
  checkMatrix(MATRIX);
  assert.equal(MATRIX.summary.local_apfs_filesystems_with_observed_hard_link_and_sync,1);
  const a=MATRIX.volume_cases.find(x=>x.class==="macos_local_apfs");
  assert.equal(a.evidence_kind,"real_local_write_test");
  assert.equal(a.actual_capabilities.filesystem,"apfs");
  assert.equal(a.actual_capabilities.hard_link_creation_observed,true);
  assert.equal(a.actual_capabilities.hard_link_inode_identity_observed,true);
  assert.equal(a.actual_capabilities.existing_target_hardlink_refused,true);
  assert.equal(a.actual_capabilities.directory_fsync_returned_success,true);
  assert.equal(a.actual_capabilities.true_durability_proven,false);
  assert.equal(MATRIX.summary.production_writes_authorized,0);
});
test("NTFS and NFS mounts are discovered without writing to remote or read-only volumes",()=>{
  const mounts=inspectMounts(
    "/dev/disk3s5 on /System/Volumes/Data (apfs, local, journaled)\n"+
    "/dev/disk6s1 on /Volumes/Disk (ntfs, local, read-only, noowners)\n"+
    "OrbStack:/OrbStack on /Users/test/OrbStack (nfs, nodev, mounted by test)\n"+
    "devfs on /dev (devfs, local)\n"
  );
  assert.deepEqual(mounts.map(x=>x.filesystem),["apfs","ntfs","nfs"]);
  assert.equal(mounts[1].explicitly_read_only,true);
  assert.equal(mounts[2].explicitly_local,false);
  assert.ok(mounts.every(x=>x.volume_write_tested===false&&
    x.crash_durability_certified===false));
  const ntfs=MATRIX.volume_cases.find(x=>x.class==="macos_mounted_ntfs");
  const nfs=MATRIX.volume_cases.find(x=>x.class==="macos_mounted_nfs");
  assert.equal(ntfs.volume_write_tested,false);
  assert.equal(nfs.volume_write_tested,false);
  assert.equal(MATRIX.summary.NTFS_and_NFS_write_tests,0);
});
test("Windows and Linux results remain absent, not synthetic successful devices",()=>{
  assert.equal(MATRIX.summary.real_windows_linux_device_receipts,0);
  assert.equal(MATRIX.summary.physical_power_cut_tests,0);
  for(const name of ["windows_ntfs","linux_local_and_network"]){
    const x=MATRIX.volume_cases.find(y=>y.class===name);
    assert.equal(x.evidence_kind,"no_host_test_receipt");
    assert.equal(x.volume_write_tested,false);
    assert.equal(x.state,"not_tested");
  }
});
test("manual recovery four known checkpoint classes remain read-only and have no success claim for missing receipts",()=>{
  checkManual(MANUAL);
  assert.equal(MANUAL.summary.read_only_manual_dispositions,4);
  assert.equal(MANUAL.summary.missing_receipt_explicit_manual_holds,2);
  assert.ok(MANUAL.cases.every(x=>x.manual_recovery.no_auto_republish &&
    x.manual_recovery.no_auto_delete));
  assert.equal(MANUAL.summary.production_psd_writes_authorized,0);
  for(const row of MANUAL.cases){
    if(["after_link","after_directory_fsync"].includes(row.test_cutpoint))
      assert.equal(row.manual_recovery.decision,"HOLD_PUBLISHED_NO_VERIFIED_RECEIPT");
  }
});
test("normal completion classified as already-verified, not a new publish request",()=>{
  inRun(dir=>{
    attempt(dir);
    const before=fs.readFileSync(target(dir));
    const result=inspect(dir);
    assert.equal(result.decision,"OBSERVED_COMPLETED_RESEARCH");
    assert.equal(result.publisher_receipt_previously_present,true);
    assert.equal(result.private_staging_and_published_inode_match,true);
    assert.deepEqual(fs.readFileSync(target(dir)),before);
    assert.equal(result.no_auto_republish,true);
  });
});
test("missing receipt retains matching output without generating a new receipt",()=>{
  inRun(dir=>{
    attempt(dir,{interruptAt:"after_link"});
    const before=fs.readFileSync(target(dir));
    const t=txn(dir);
    assert.equal(fs.existsSync(path.join(t,"receipt.json")),false);
    const result=inspect(dir);
    assert.equal(result.decision,"HOLD_PUBLISHED_NO_VERIFIED_RECEIPT");
    assert.equal(result.publisher_receipt_previously_present,false);
    assert.equal(result.actual_power_loss_durability_proven,false);
    assert.equal(fs.existsSync(path.join(t,"receipt.json")),false);
    assert.deepEqual(fs.readFileSync(target(dir)),before);
  });
});
test("staged-only state retains all bytes and never auto-publishes",()=>{
  inRun(dir=>{
    attempt(dir,{interruptAt:"after_stage"});
    const original=fs.readFileSync(path.join(txn(dir),"candidate.psd"));
    assert.equal(inspect(dir).decision,"HOLD_STAGED_ONLY");
    assert.equal(fs.existsSync(target(dir)),false);
    assert.deepEqual(fs.readFileSync(path.join(txn(dir),"candidate.psd")),original);
  });
});
test("post-stage source byte mutation or inode replacement always stops",()=>{
  inRun(dir=>{
    attempt(dir,{interruptAt:"after_stage"});
    fs.appendFileSync(path.join(dir,"pinned-source.psd"),Buffer.from([1]));
    assert.equal(inspect(dir).decision,"STOP_SOURCE_VERSION_CHANGED");
    assert.equal(fs.existsSync(target(dir)),false);
  });
  inRun(dir=>{
    attempt(dir,{interruptAt:"after_stage"});
    const src=path.join(dir,"pinned-source.psd");
    const other=path.join(dir,"same-bytes.psd");
    fs.copyFileSync(src,other);
    fs.renameSync(other,src);
    assert.equal(inspect(dir).decision,"STOP_SOURCE_VERSION_CHANGED");
  });
});
test("racing different-inode same SHA-256 output is not credited as our publication",()=>{
  inRun(dir=>{
    attempt(dir,{interruptAt:"after_stage"});
    const payload=fs.readFileSync(path.join(txn(dir),"candidate.psd"));
    fs.writeFileSync(target(dir),payload,{flag:"wx"});
    const state=recovery(dir);
    assert.equal(state.output_inode_matches_private_staging,false);
    assert.equal(inspect(dir).decision,"STOP_UNTRUSTED_OUTPUT");
    assert.deepEqual(fs.readFileSync(target(dir)),payload);
  });
});
test("symlink output, tampered stage, unknown transaction intent are refused without changes",()=>{
  inRun(dir=>{
    attempt(dir,{interruptAt:"after_stage"});
    fs.symlinkSync(path.join(dir,"pinned-source.psd"),target(dir));
    assert.equal(inspect(dir).decision,"STOP_UNTRUSTED_OUTPUT");
    assert.ok(fs.lstatSync(target(dir)).isSymbolicLink());
  });
  inRun(dir=>{
    attempt(dir,{interruptAt:"after_stage"});
    fs.appendFileSync(path.join(txn(dir),"candidate.psd"),Buffer.from([9]));
    assert.equal(inspect(dir).decision,"STOP_INVALID_TRANSACTION_EVIDENCE");
  });
  inRun(dir=>{
    attempt(dir,{interruptAt:"after_stage"});
    const file=path.join(txn(dir),"intent.json");
    const intent=JSON.parse(fs.readFileSync(file,"utf8"));
    intent.source_sha256="changed";
    fs.writeFileSync(file,JSON.stringify(intent));
    assert.equal(inspect(dir).decision,"STOP_INVALID_TRANSACTION_EVIDENCE");
  });
});
test("a verified old receipt does not override a current output conflict",()=>{
  inRun(dir=>{
    attempt(dir);
    const replacement=path.join(dir,"new-content.psd");
    fs.writeFileSync(replacement,Buffer.from("replacement"));
    fs.renameSync(replacement,target(dir));
    const state=inspect(dir);
    assert.equal(state.decision,"STOP_UNTRUSTED_OUTPUT");
    assert.equal(state.publisher_receipt_previously_present,true);
  });
});
test("recovery classifier refuses invalid summaries, foreign paths and unknown categories",()=>{
  assert.throws(()=>classify({milestone:"other"}),/untrusted/);
  assert.throws(()=>classify({
    milestone:"M4c-12",public_write_authorized:false,
    automatic_republication_attempted:false,
    automatic_output_deletion_attempted:false,
    classification:"surprise"
  }),/unrecognized/);
  assert.equal(inspect("/tmp/somebody-else-recovery").decision,
    "STOP_INVALID_TRANSACTION_EVIDENCE");
});
