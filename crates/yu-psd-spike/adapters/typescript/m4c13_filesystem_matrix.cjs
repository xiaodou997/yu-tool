"use strict";

// M4c-13: A narrowly scoped REAL filesystem capability experiment.
// Only the existing ignored YuTool research scratch directory is written.
// Removable, NFS, NTFS, Windows and Linux volumes are inventory-only;
// the presence of a mount never establishes write or crash durability.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const {spawnSync}=require("node:child_process");
const {SCRATCH,newRun,syncDir}=require("./m4c12_recovery_fs_gate.cjs");
const ROOT=path.resolve(__dirname,"../../../../");
const REPORT=path.join(ROOT,"docs/data/m4c13-filesystem-matrix-v1.json");
const sha=bytes=>crypto.createHash("sha256").update(bytes).digest("hex");
function inspectMounts(output) {
  if(typeof output!=="string"||output.length>1_000_000)
    throw Error("unbounded mount inventory");
  const results=[];
  for(const line of output.split("\n")){
    const m=line.match(/\son\s(.+?)\s\(([^)]+)\)$/);
    if(!m)continue;
    const flags=m[2].split(",").map(x=>x.trim());
    const kind=flags[0].toLowerCase();
    if(!["apfs","ntfs","nfs"].includes(kind))continue;
    results.push({filesystem:kind,mount_path:m[1],
      explicitly_read_only:flags.includes("read-only"),
      explicitly_local:flags.includes("local"),
      volume_write_tested:false,
      crash_durability_certified:false});
  }
  return results;
}
function mountMatrix() {
  if(process.platform!=="darwin")
    throw Error("no native macOS filesystem host present; cannot make actual claim");
  const result=spawnSync("/sbin/mount",[],{
    encoding:"utf8",timeout:5000,maxBuffer:1024*1024
  });
  if(result.error||result.status!==0)
    throw Error("cannot read native mount inventory");
  return inspectMounts(result.stdout);
}
function probePrivateApfsScratch() {
  const dir=newRun();
  const before=fs.statfsSync(dir),disk=fs.statSync(dir);
  const mounts=mountMatrix();
  const dataApfs=mounts.some(m=>m.filesystem==="apfs"&&m.mount_path==="/System/Volumes/Data");
  // The host's source scratch directory sits on macOS Data APFS.
  // Absence of that mount evidence is a refusal, not an assumption.
  if(!dataApfs)throw Error("local macOS Data APFS mount not established");
  const src=path.join(dir,"fs-probe-original.bin");
  const dest=path.join(dir,"fs-probe-public-link.bin");
  const message=Buffer.from("yu-m4c13-explicit-research-filesystem-probe\n");
  const fd=fs.openSync(src,fs.constants.O_CREAT|fs.constants.O_EXCL|fs.constants.O_WRONLY,0o600);
  try{fs.writeFileSync(fd,message);fs.fsyncSync(fd);}finally{fs.closeSync(fd);}
  fs.linkSync(src,dest);
  const a=fs.statSync(src),b=fs.statSync(dest);
  if(a.dev!==b.dev||a.ino!==b.ino ||
     sha(fs.readFileSync(src))!==sha(fs.readFileSync(dest)))
    throw Error("hard-link content/inode identity not observed");
  let noClobber=false;
  try{fs.linkSync(src,dest);}catch(e){noClobber=e.code==="EEXIST";}
  if(!noClobber)throw Error("atomic no-clobber duplicate link was not refused");
  syncDir(dir);
  return {
    filesystem:"apfs",
    scope:"same local mounted writable APFS Data volume, ignored target scratch only",
    filesystem_device:String(disk.dev),
    statfs_type:String(before.type),
    statfs_block_size:before.bsize,
    hard_link_creation_observed:true,
    hard_link_inode_identity_observed:true,
    existing_target_hardlink_refused:true,
    file_fsync_returned_success:true,
    directory_fsync_returned_success:true,
    real_power_loss_test_completed:false,
    true_durability_proven:false,
    other_filesystems_tested_for_writes:false
  };
}
function build(){
  const mounts=mountMatrix();
  const actual=probePrivateApfsScratch();
  const ntfs=mounts.filter(m=>m.filesystem==="ntfs");
  const nfs=mounts.filter(m=>m.filesystem==="nfs");
  const volumeCases=[
    {class:"macos_local_apfs",evidence_kind:"real_local_write_test",
      state:"tested_local_single_volume",volume_write_tested:true,
      actual_capabilities:actual},
    {class:"macos_mounted_ntfs",evidence_kind:"mount_inventory_only",
      state:ntfs.length?ntfs.some(x=>x.explicitly_read_only)?
        "read_only_mount_skip_writes":"mounted_not_authorized_for_writes":
        "mount_absent_not_tested",
      mount_count:ntfs.length,volume_write_tested:false},
    {class:"macos_mounted_nfs",evidence_kind:"mount_inventory_only",
      state:nfs.length?"mounted_no_test_authorization":"mount_absent_not_tested",
      mount_count:nfs.length,volume_write_tested:false},
    {class:"windows_ntfs",evidence_kind:"no_host_test_receipt",
      state:"not_tested",volume_write_tested:false},
    {class:"linux_local_and_network",evidence_kind:"no_host_test_receipt",
      state:"not_tested",volume_write_tested:false},
  ];
  return {
    schema_version:"1",milestone:"M4c-13",
    operation:"real macOS filesystem probe plus inventory-only untested volumes",
    public_write_authorized:false,
    summary:{
      local_apfs_filesystems_with_observed_hard_link_and_sync:1,
      real_windows_linux_device_receipts:0,
      NTFS_and_NFS_write_tests:0,
      physical_power_cut_tests:0,
      production_writes_authorized:0
    },
    caveats:[
      "statfs and successful file/directory fsync report syscall outcomes, not post-power-loss persistence",
      "read-only NTFS and mounted NFS not written to; do not claim cross-filesystem compatibility",
      "real Windows and Linux receipts absent, not simulated passes",
      "the private hard-link experiment cannot authorize a general PSD writer"
    ],volume_cases:volumeCases
  };
}
function check(report){
  if(report.milestone!=="M4c-13"||report.public_write_authorized!==false||
     report.summary.production_writes_authorized!==0||
     report.summary.real_windows_linux_device_receipts!==0||
     report.summary.physical_power_cut_tests!==0||
     report.summary.NTFS_and_NFS_write_tests!==0)
    throw Error("M4c-13 snapshot must not claim untested durability or write approval");
  const apfs=report.volume_cases.find(x=>x.class==="macos_local_apfs");
  if(!apfs?.actual_capabilities?.hard_link_inode_identity_observed ||
    apfs.actual_capabilities.true_durability_proven!==false)
    throw Error("M4c-13 local APFS probe receipt invalid");
}
if(require.main===module){
  const mode=process.argv[2];
  if(mode==="--write"&&process.argv.length===3){
    const result=build();check(result);
    fs.writeFileSync(REPORT,JSON.stringify(result,null,2)+"\n");
    console.log(JSON.stringify({mode,...result.summary}));
  }else if(mode==="--check"&&process.argv.length===3){
    const result=JSON.parse(fs.readFileSync(REPORT,"utf8"));check(result);
    console.log(JSON.stringify({mode,offline:true,...result.summary}));
  }else {
    process.stderr.write("usage: node m4c13_filesystem_matrix.cjs --write|--check\n");
    process.exitCode=2;
  }
}
module.exports={inspectMounts,mountMatrix,probePrivateApfsScratch,build,check};
