"use strict";

// M4c-14: portable, explicitly invoked device filesystem experiment.
// It never opens PSDs, user files, Photoshop or other applications.
// The ONLY writable path is a new private folder under target/.
// Self-reported device receipts are evidence candidates, not remotely
// authenticated device identity or a PSD production release.
const fs=require("node:fs");
const os=require("node:os");
const path=require("node:path");
const crypto=require("node:crypto");
const cp=require("node:child_process");
const assert=require("node:assert/strict");

const ROOT=path.resolve(__dirname,"../../../../");
const BASE=path.join(ROOT,"target/m4c14-device-probe");
const REC=path.join(ROOT,"docs/data/m4c14-local-device-v1.json");
const CONTRACT_BASE_SHA="c6f80b4beb6b51ec64904405942b43c5ec312426";
const sha=data=>crypto.createHash("sha256").update(data).digest("hex");

function boundary() {
  const target=path.join(ROOT,"target");
  for(const dir of [ROOT,target,BASE]){
    if(!fs.existsSync(dir)) {
      if(dir===ROOT)throw Error("repository root missing");
      fs.mkdirSync(dir);
    }
    const stat=fs.lstatSync(dir);
    if(stat.isSymbolicLink()||!stat.isDirectory())
      throw Error("research directory symlink/non-directory refused");
  }
}
function syncedFile(file,data){
  const fd=fs.openSync(file,fs.constants.O_EXCL|fs.constants.O_CREAT|
    fs.constants.O_WRONLY,0o600);
  try{fs.writeFileSync(fd,data);fs.fsyncSync(fd);}
  finally{fs.closeSync(fd);}
}
function tryDirectorySync(dir){
  let fd;
  try{
    fd=fs.openSync(dir,fs.constants.O_RDONLY);
    fs.fsyncSync(fd);
    return {syscall_succeeded:true,error_code:null};
  }catch(e){
    return {syscall_succeeded:false,error_code:String(e.code||"UNKNOWN")};
  }finally{if(fd!==undefined)fs.closeSync(fd);}
}
function pinnedHead() {
  const result=cp.spawnSync("git",["rev-parse","HEAD"],{
    encoding:"utf8",cwd:ROOT,timeout:4500,maxBuffer:1024
  });
  if(result.error||result.status!==0)throw Error("no git source revision");
  const head=result.stdout.trim();
  if(!/^[0-9a-f]{40}$/.test(head))throw Error("bad git SHA");
  return head;
}
function actualFilesystemKind(st) {
  // statfs type is an opaque platform-dependent integer. Do not map
  // numeric codes into ext4, XFS, NTFS, APFS, etc without native
  // independent evidence for this exact mounted volume.
  return {statfs_type:String(st.type),statfs_block_size:st.bsize};
}
function perform() {
  if(!["darwin","win32","linux"].includes(process.platform))
    throw Error("OS not in the candidate acceptance matrix");
  if(process.versions.node!=="22.23.3")
    throw Error("research node version must be exactly 22.23.3");
  boundary();
  const dir=fs.mkdtempSync(path.join(BASE,"run-"));
  fs.chmodSync(dir,0o700);
  const original=path.join(dir,"private-source.bin");
  const dest=path.join(dir,"private-hardlink.bin");
  const value=Buffer.from("M4c14 fixed isolated filesystem probe\n");
  syncedFile(original,value);
  let linkError=null;
  try{fs.linkSync(original,dest);}
  catch(e){linkError=String(e.code||"UNKNOWN");}
  const a=fs.statSync(original),b=linkError?null:fs.statSync(dest);
  const sameIdentity=!!b&&a.dev===b.dev&&a.ino===b.ino&&a.ino!==0;
  const sameBytes=!!b&&sha(fs.readFileSync(original))===sha(fs.readFileSync(dest));
  let clobberDenied=false;
  if(b){
    try{fs.linkSync(original,dest);}
    catch(e){clobberDenied=e.code==="EEXIST";}
  }
  const sync=tryDirectorySync(dir);
  const statfs=fs.statfsSync(dir);
  return {
    schema_version:"1",milestone:"M4c-14",
    evidence_type:"local_device_executed_probe_unattested",
    source_baseline_sha:CONTRACT_BASE_SHA,
    executing_checkout_sha:pinnedHead(),
    probe_script_sha256:sha(fs.readFileSync(__filename)),
    host:{platform:process.platform,arch:process.arch,
      node_version:process.versions.node,
      os_release:os.release()},
    private_test_scope:"new ignored target/m4c14-device-probe/run-* only",
    actual_filesystem:actualFilesystemKind(statfs),
    observations:{
      exclusive_private_file_created:true,
      source_file_fsync_succeeded:true,
      hard_link_succeeded:!!b,
      hard_link_error_code:linkError,
      hard_link_same_inode:!!sameIdentity,
      source_destination_sha256_equal:sameBytes,
      second_link_existing_destination_denied:clobberDenied,
      parent_directory_fsync:sync,
      source_file_size:a.size,
      destination_file_size:b?.size??null
    },
    test_evaluated:["file.create_exclusive","file.fsync",
      "hardlink.same_device_inode","hardlink.no_clobber",
      "directory.fsync.syscall"],
    not_evaluated:["PSD Photoshop save/reopen","actual power loss",
      "storage controller flush","different filesystem/device",
      "filesystem vendor/product claims","source concurrent writer",
      "cross-platform acceptance","public PSD mutation"],
    hardware_attested:false,
    power_loss_durability_certified:false,
    production_write_authorized:false,
    note:"A successful syscall is not proof of crash durability or device authenticity."
  };
}
function check(r) {
  assert.equal(r.schema_version,"1");
  assert.equal(r.milestone,"M4c-14");
  assert.equal(r.evidence_type,"local_device_executed_probe_unattested");
  assert.ok(["darwin","win32","linux"].includes(r.host.platform));
  assert.equal(r.host.node_version,"22.23.3");
  assert.match(r.source_baseline_sha,/^[0-9a-f]{40}$/);
  assert.equal(r.source_baseline_sha,CONTRACT_BASE_SHA);
  assert.match(r.executing_checkout_sha,/^[0-9a-f]{40}$/);
  assert.match(r.probe_script_sha256,/^[0-9a-f]{64}$/);
  assert.equal(r.observations.exclusive_private_file_created,true);
  assert.equal(r.hardware_attested,false);
  assert.equal(r.power_loss_durability_certified,false);
  assert.equal(r.production_write_authorized,false);
}
if(require.main===module) {
  const mode=process.argv[2];
  if(!["--write","--check"].includes(mode)||
    (mode==="--write"&&process.argv.length!==3)||
    (mode==="--check"&&![3,4].includes(process.argv.length))){
    process.stderr.write("usage: node m4c14_device_probe.cjs --write|--check [RECEIPT_FILE]\n");
    process.exitCode=2;
  }else if(mode==="--write"){
    const result=perform();check(result);
    // Commit a bounded snapshot only for the existing macOS research
    // runner. A future Windows/Linux physical device writes its own
    // receipt to ignored target instead of silently overwriting Mac
    // evidence in docs/data. Every receipt remains self-reported.
    const out=process.platform==="darwin"?REC:
      path.join(BASE,"device-receipt-"+process.platform+"-"+
        crypto.randomUUID()+".json");
    fs.writeFileSync(out,JSON.stringify(result,null,2)+"\n",
      process.platform==="darwin"?undefined:{flag:"wx"});
    console.log(JSON.stringify({mode,host:result.host,
      receipt_path:path.relative(ROOT,out),
      directory_fsync:result.observations.parent_directory_fsync,
      hard_link_same_inode:result.observations.hard_link_same_inode,
      hardware_attested:result.hardware_attested,
      production_write_authorized:result.production_write_authorized}));
  }else{
    const source=process.argv[3]?path.resolve(process.argv[3]):REC;
    const result=JSON.parse(fs.readFileSync(source,"utf8"));check(result);
    console.log(JSON.stringify({mode,offline:true,platform:result.host.platform,
      production_write_authorized:false}));
  }
}
module.exports={perform,check,actualFilesystemKind,tryDirectorySync};
