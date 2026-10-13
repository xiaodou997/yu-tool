"use strict";

// Research-only M4c-12 restart-boundary and filesystem probe.
// NEVER use for arbitrary PSD paths or as a public YuTool writer.
// No automatic recovery writes or deletion of published output.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const assert=require("node:assert/strict");
const {spawnSync}=require("node:child_process");
const {patchKnownPixel}=require("./m4c8_pixel_rename_research.cjs");
const {validateCandidate,BASE,ORIGINAL_SHA,CANDIDATE_SHA}=require("./m4c11_safe_output_spike.cjs");
const PRIOR_META=require("../../../../docs/data/m4c11-metadata-semantic-gate-v1.json");

const ROOT=path.resolve(__dirname,"../../../../");
const SCRATCH=path.join(ROOT,"target/m4c12-recovery");
const EVIDENCE=path.join(ROOT,"docs/data/m4c12-recovery-filesystem-v1.json");
const NAME="renamed-research.psd";
const SHA=b=>crypto.createHash("sha256").update(b).digest("hex");
function fail(code,message) {
  const err=new Error(message);err.code=code;throw err;
}
function regular(file){
  const s=fs.lstatSync(file);
  if(!s.isFile()||s.isSymbolicLink()||s.size>8*1024*1024)
    fail("UNSAFE_FILE","not an allowed regular PSD file");
  return s;
}
function noExisting(file) {
  try {fs.lstatSync(file);fail("OUTPUT_CONFLICT","destination already exists, including symlink");}
  catch(e){if(e.code!=="ENOENT")throw e;}
}
function safeDir(dir) {
  const d=path.resolve(dir);
  if(path.dirname(d)!==SCRATCH||!path.basename(d).startsWith("run-"))
    fail("RESEARCH_SCOPE_DENIED","only direct private run directories supported");
  for(const x of [SCRATCH,d]){
    const st=fs.lstatSync(x);
    if(st.isSymbolicLink()||!st.isDirectory())
      fail("RESEARCH_SCOPE_DENIED","symlink or non-directory boundary");
  }
  return d;
}
function syncDir(dir,{simulateFailure=false}={}) {
  if(simulateFailure)fail("DURABILITY_UNVERIFIED","simulated directory fsync not supported");
  let fd;
  try{
    fd=fs.openSync(dir,fs.constants.O_RDONLY);
    fs.fsyncSync(fd);
    return {directory_fsync_succeeded:true};
  }catch(e){
    fail("DURABILITY_UNVERIFIED","cannot fsync research directory: "+(e.code||"unknown"));
  }finally{if(fd!==undefined)fs.closeSync(fd);}
}
function writeSyncedNew(file,bytes) {
  let fd=fs.openSync(file,fs.constants.O_WRONLY|fs.constants.O_CREAT|fs.constants.O_EXCL,0o600);
  try{fs.writeFileSync(fd,bytes);fs.fsyncSync(fd);}
  finally{fs.closeSync(fd);}
}
function newRun(){
  fs.mkdirSync(SCRATCH,{recursive:true});
  const root=fs.lstatSync(SCRATCH);
  if(!root.isDirectory()||root.isSymbolicLink())
    fail("RESEARCH_SCOPE_DENIED","scratch root must not be a symlink");
  const dir=fs.mkdtempSync(path.join(SCRATCH,"run-"));
  fs.chmodSync(dir,0o700);
  syncDir(SCRATCH);
  const initial=fs.readFileSync(BASE);
  assert.equal(SHA(initial),ORIGINAL_SHA);
  writeSyncedNew(path.join(dir,"pinned-source.psd"),initial);
  syncDir(dir);
  return dir;
}
function sourceState(dir){
  const file=path.join(dir,"pinned-source.psd"),st=regular(file);
  const bytes=fs.readFileSync(file);
  if(SHA(bytes)!==ORIGINAL_SHA)
    fail("SOURCE_VERSION_CONFLICT","source no longer matches fixed M4c-8 fixture");
  return {file,dev:st.dev,ino:st.ino,size:st.size,
    sha256:SHA(bytes),bytes};
}
function recheckSource(source) {
  const stat=regular(source.file);
  if(stat.dev!==source.dev||stat.ino!==source.ino||
     stat.size!==source.size||SHA(fs.readFileSync(source.file))!==source.sha256)
    fail("SOURCE_VERSION_CONFLICT","source changed during publication");
}
function stageIntent(dir,source) {
  const txn=fs.mkdtempSync(path.join(dir,".txn-"));
  fs.chmodSync(txn,0o700);
  const candidate=patchKnownPixel(source.bytes);
  assert.equal(candidate.sha256,CANDIDATE_SHA);
  validateCandidate(candidate.bytes);
  const staged=path.join(txn,"candidate.psd");
  writeSyncedNew(staged,candidate.bytes);
  const intent={
    schema_version:"1",milestone:"M4c-12",
    operation:"research only exact SHA PSD output",
    source_sha256:ORIGINAL_SHA,candidate_sha256:CANDIDATE_SHA,
    source_device:String(source.dev),source_inode:String(source.ino),
    source_bytes:source.size,
    output_basename:NAME,public_write_authorized:false
  };
  writeSyncedNew(path.join(txn,"intent.json"),
    Buffer.from(JSON.stringify(intent,null,2)+"\n"));
  syncDir(txn);
  syncDir(dir);
  return {txn,staged,intent};
}
function attempt(dir,{interruptAt=null,simulateLinkError=null,
  simulateDirectoryFsyncFailure=false,beforeLink}={}){
  const run=safeDir(dir);
  const output=path.join(run,NAME);
  noExisting(output);
  const source=sourceState(run);
  const transaction=stageIntent(run,source);
  const result={milestone:"M4c-12",
    public_write_authorized:false,
    output_basename:NAME,
    transaction_basename:path.basename(transaction.txn),
    original_sha256:ORIGINAL_SHA,candidate_sha256:CANDIDATE_SHA};
  if(interruptAt==="after_stage")
    return {...result,state:"interrupted_after_stage_before_link",
      directory_fsync_after_publication:false};
  if(![null,"after_link","after_directory_fsync"].includes(interruptAt))
    fail("RESEARCH_SCOPE_DENIED","unrecognized interruption point");
  if(beforeLink!==undefined){
    if(typeof beforeLink!=="function")fail("RESEARCH_SCOPE_DENIED","test hook invalid");
    beforeLink({output,source:source.file});
  }
  recheckSource(source);
  try{
    if(simulateLinkError){
      if(!["EXDEV","ENOTSUP","EOPNOTSUPP"].includes(simulateLinkError))
        fail("RESEARCH_SCOPE_DENIED","unsupported fault injection");
      const e=new Error("simulated unsupported filesystem link");
      e.code=simulateLinkError;throw e;
    }
    fs.linkSync(transaction.staged,output);
  }catch(e){
    if(e.code==="EEXIST")fail("OUTPUT_CONFLICT","rival wrote output; never overwrite");
    if(["EXDEV","ENOTSUP","EOPNOTSUPP"].includes(e.code))
      fail("FILESYSTEM_UNSUPPORTED","same-volume hard link unsupported; no fallback write");
    throw e;
  }
  if(interruptAt==="after_link")return {...result,
    state:"interrupted_after_link_before_parent_fsync",
    directory_fsync_after_publication:false};
  syncDir(run,{simulateFailure:simulateDirectoryFsyncFailure});
  if(interruptAt==="after_directory_fsync")return {...result,
    state:"interrupted_after_parent_fsync_before_receipt",
    directory_fsync_after_publication:true};
  const published=fs.readFileSync(output);
  validateCandidate(published);
  if(SHA(published)!==CANDIDATE_SHA)
    fail("POST_PUBLISH_INCONCLUSIVE","target changed before verification");
  recheckSource(source);
  const receipt={...result,
    state:"observed_verified_after_parent_fsync",
    directory_fsync_after_publication:true,
    verified_output_sha256:SHA(published),
    power_loss_guarantee_proven:false,
    filesystem_portability_proven:false,
    metadata_fidelity_proven:false};
  writeSyncedNew(path.join(transaction.txn,"receipt.json"),
    Buffer.from(JSON.stringify(receipt,null,2)+"\n"));
  syncDir(transaction.txn);
  // Deliberately retain stage and receipt as research evidence; never
  // delete the published file or silently re-publish during recovery.
  return receipt;
}
function recovery(dir){
  const run=safeDir(dir);
  const txns=fs.readdirSync(run).filter(x=>x.startsWith(".txn-"));
  if(txns.length!==1)fail("RECOVERY_AMBIGUOUS","need exactly one research transaction");
  const transaction=path.join(run,txns[0]);
  const txnInfo=fs.lstatSync(transaction);
  if(txnInfo.isSymbolicLink()||!txnInfo.isDirectory())
    fail("RECOVERY_AMBIGUOUS","unsafe transaction directory");
  const intentPath=path.join(transaction,"intent.json");
  regular(intentPath);
  const intent=JSON.parse(fs.readFileSync(intentPath,"utf8"));
  if(intent.schema_version!=="1"||intent.milestone!=="M4c-12"||
    intent.public_write_authorized!==false||
    intent.source_sha256!==ORIGINAL_SHA||intent.candidate_sha256!==CANDIDATE_SHA||
    intent.output_basename!==NAME||
    !/^\d+$/.test(intent.source_device)||!/^\d+$/.test(intent.source_inode)||
    !Number.isSafeInteger(intent.source_bytes))
    fail("RECOVERY_AMBIGUOUS","invalid frozen transaction intent");
  const staged=path.join(transaction,"candidate.psd");
  const stagedStat=regular(staged);
  if(SHA(fs.readFileSync(staged))!==CANDIDATE_SHA)
    fail("RECOVERY_AMBIGUOUS","staged candidate tampered; do not retry");
  const source=path.join(run,"pinned-source.psd");
  let sourceMatches=false;
  try{
    const st=regular(source);
    sourceMatches=SHA(fs.readFileSync(source))===ORIGINAL_SHA &&
      String(st.dev)===intent.source_device &&
      String(st.ino)===intent.source_inode &&
      st.size===intent.source_bytes;
  }catch{}
  const output=path.join(run,NAME);
  let publishedStatus="absent",observedHash=null;
  let matchingPublishedHardlink=false;
  try {
    const info=fs.lstatSync(output);
    if(!info.isFile()||info.isSymbolicLink()||info.size>8*1024*1024)
      publishedStatus="untrusted_or_competing_output";
    else{
      observedHash=SHA(fs.readFileSync(output));
      matchingPublishedHardlink=info.dev===stagedStat.dev&&info.ino===stagedStat.ino;
      publishedStatus=observedHash===CANDIDATE_SHA && matchingPublishedHardlink?
        "matching_candidate_observed":"untrusted_or_competing_output";
    }
  }catch(e){if(e.code!=="ENOENT")throw e;}
  const receiptPath=path.join(transaction,"receipt.json");
  let verifiedReceipt=false;
  try{
    regular(receiptPath);
    const receipt=JSON.parse(fs.readFileSync(receiptPath,"utf8"));
    verifiedReceipt=receipt.state==="observed_verified_after_parent_fsync" &&
      receipt.verified_output_sha256===CANDIDATE_SHA &&
      receipt.candidate_sha256===CANDIDATE_SHA &&
      receipt.original_sha256===ORIGINAL_SHA &&
      receipt.directory_fsync_after_publication===true &&
      receipt.public_write_authorized===false;
  }catch(e){if(e.code!=="ENOENT")throw e;}
  return {
    milestone:"M4c-12",public_write_authorized:false,
    source_sha256_still_matches:sourceMatches,
    staged_candidate_sha256_matches:true,
    published_status:publishedStatus,
    output_inode_matches_private_staging:matchingPublishedHardlink,
    observed_output_sha256:observedHash,
    existing_verified_receipt:verifiedReceipt,
    classification:!sourceMatches?"source_drift_refuse_automatic_resume":
      publishedStatus==="absent"?"staged_only_manual_disposition_required":
      publishedStatus!=="matching_candidate_observed"?
        "output_conflict_do_not_touch":
      verifiedReceipt?"verified_receipt_observed":
        "published_candidate_without_receipt_reinspect_only",
    automatic_republication_attempted:false,
    automatic_output_deletion_attempted:false,
    previous_crash_durability_proven:false
  };
}
function filesystemInfo(dir){
  const d=safeDir(dir);
  const st=fs.statfsSync(d);
  const volume=fs.statSync(d);
  const sync=syncDir(d);
  return {
    platform:process.platform,
    statfs_type:String(st.type),
    statfs_block_size:st.bsize,
    filesystem_device_id:String(volume.dev),
    observed_parent_directory_fsync:sync.directory_fsync_succeeded
  };
}
function build(){
  const cases=[];
  for(const cut of ["after_stage","after_link","after_directory_fsync",null]){
    const dir=newRun();
    const outcome=attempt(dir,{interruptAt:cut});
    const observed=recovery(dir);
    cases.push({simulated_cutpoint:cut||"normal_complete",
      attempted_state:outcome.state,recovery:observed});
  }
  const childCrashes=[];
  for(const [command,expected] of [
    ["--sigkill-after-stage","staged_only_manual_disposition_required"],
    ["--sigkill-after-link","published_candidate_without_receipt_reinspect_only"]
  ]){
    const dir=newRun();
    const child=spawnSync(process.execPath,[__filename,command,dir],{
      encoding:"utf8",timeout:30000,maxBuffer:32768
    });
    if(child.error||child.status!==null||child.signal!=="SIGKILL")
      fail("RESEARCH_PROCESS_INTERRUPTION_FAILED","the controlled child did not terminate by SIGKILL");
    const observed=recovery(dir);
    if(observed.classification!==expected)
      fail("RECOVERY_AMBIGUOUS","child interruption not classified as expected");
    childCrashes.push({terminator:"SIGKILL_of_disposable_child_only",
      phase:command,recovered_classification:observed.classification,
      source_sha256_still_matches:observed.source_sha256_still_matches,
      previous_crash_durability_proven:false});
  }
  assert.equal(PRIOR_META.summary.paired_multilingual_cases,2);
  assert.equal(PRIOR_META.summary.cases_with_unknown_semantics,2);
  const result={
    schema_version:"1",milestone:"M4c-12",
    status:"research_recovery_classification_implemented_not_durability_certified",
    public_write_authorized:false,
    summary:{
      deterministic_cutpoints_tested:cases.length,
      real_child_SIGKILL_cutpoints_tested:childCrashes.length,
      recovery_read_only_cases:cases.length,
      production_writes_authorized:0,
      power_failure_durability_verified:false,
      supported_customer_filesystems_certified:0
    },
    observed_filesystem:filesystemInfo(newRun()),
    metadata_retention_policy:{
      exact_known_source_patch_preserves_all_non_name_bytes:true,
      saved_photoshop_pairs_referenced:PRIOR_META.summary.paired_multilingual_cases,
      EXIF_DateTime_changes_identified_not_ignored:true,
      XMP_document_identity_history_fields_not_ignored:true,
      shmd_and_unknown_tagged_blocks_not_ignored:true,
      metadata_auto_normalization_or_exclusion_rules:0,
      broad_semantic_equivalence_certified:false
    },
    cases,real_child_SIGKILL_runs:childCrashes
  };
  return result;
}
function check(r){
  assert.equal(r.milestone,"M4c-12");
  assert.equal(r.public_write_authorized,false);
  assert.equal(r.summary.deterministic_cutpoints_tested,4);
  assert.equal(r.summary.real_child_SIGKILL_cutpoints_tested,2);
  assert.equal(r.summary.production_writes_authorized,0);
  assert.equal(r.summary.power_failure_durability_verified,false);
  assert.ok(r.cases.every(x=>!x.recovery.automatic_republication_attempted &&
    !x.recovery.automatic_output_deletion_attempted));
  assert.equal(r.real_child_SIGKILL_runs.length,2);
  assert.ok(r.real_child_SIGKILL_runs.every(x=>
    x.terminator==="SIGKILL_of_disposable_child_only" &&
    x.source_sha256_still_matches &&
    x.previous_crash_durability_proven===false));
  assert.equal(r.metadata_retention_policy.metadata_auto_normalization_or_exclusion_rules,0);
}
if(require.main===module){
  if(process.argv[2]==="--write"&&process.argv.length===3){
    const r=build();check(r);fs.writeFileSync(EVIDENCE,JSON.stringify(r,null,2)+"\n");
    console.log(JSON.stringify({mode:"--write",...r.summary}));
  }else if(process.argv[2]==="--check"&&process.argv.length===3){
    const r=JSON.parse(fs.readFileSync(EVIDENCE,"utf8"));check(r);
    console.log(JSON.stringify({mode:"--check",offline:true,...r.summary}));
  }else if(["--sigkill-after-stage","--sigkill-after-link"].includes(process.argv[2])&&
      process.argv.length===4){
    // Terminate only the purpose-spawned disposable child process.
    const cut=process.argv[2]==="--sigkill-after-stage"?"after_stage":"after_link";
    attempt(process.argv[3],{interruptAt:cut});
    process.kill(process.pid,"SIGKILL");
  }else{
    process.stderr.write("usage: node m4c12_recovery_fs_gate.cjs --write|--check\n");
    process.exitCode=2;
  }
}
module.exports={SCRATCH,newRun,attempt,recovery,filesystemInfo,syncDir,check,build};
