"use strict";

const {test}=require("node:test");
const assert=require("node:assert/strict");
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const {spawnSync}=require("node:child_process");
const {newRun,attempt,recovery,syncDir,filesystemInfo,SCRATCH}=require("./m4c12_recovery_fs_gate.cjs");
const {ORIGINAL_SHA,CANDIDATE_SHA}=require("./m4c11_safe_output_spike.cjs");
const RECEIPT=require(path.resolve(__dirname,"../../../../docs/data/m4c12-recovery-filesystem-v1.json"));
const SCRIPT=path.join(__dirname,"m4c12_recovery_fs_gate.cjs");
const sha=b=>crypto.createHash("sha256").update(b).digest("hex");
const output=d=>path.join(d,"renamed-research.psd");
const source=d=>path.join(d,"pinned-source.psd");
const txn=d=>path.join(d,fs.readdirSync(d).filter(x=>x.startsWith(".txn-"))[0]);
function withRun(fn){
  const d=newRun();
  try{return fn(d);}finally{fs.rmSync(d,{force:true,recursive:true});}
}
function reject(code,fn){assert.throws(fn,e=>e.code===code,"expected "+code);}

test("four research cutpoints classify safely without re-publishing or deleting outputs",()=>{
  for(const [cut,status,classification] of [
    ["after_stage","interrupted_after_stage_before_link","staged_only_manual_disposition_required"],
    ["after_link","interrupted_after_link_before_parent_fsync","published_candidate_without_receipt_reinspect_only"],
    ["after_directory_fsync","interrupted_after_parent_fsync_before_receipt","published_candidate_without_receipt_reinspect_only"],
    [null,"observed_verified_after_parent_fsync","verified_receipt_observed"],
  ]){
    withRun(d=>{
      const a=attempt(d,{interruptAt:cut});
      assert.equal(a.state,status);
      const r=recovery(d);
      assert.equal(r.classification,classification);
      assert.equal(r.public_write_authorized,false);
      assert.equal(r.automatic_republication_attempted,false);
      assert.equal(r.automatic_output_deletion_attempted,false);
      assert.equal(r.previous_crash_durability_proven,false);
      assert.equal(sha(fs.readFileSync(source(d))),ORIGINAL_SHA);
      if(cut==="after_stage")assert.equal(fs.existsSync(output(d)),false);
      else assert.equal(sha(fs.readFileSync(output(d))),CANDIDATE_SHA);
    });
  }
});
test("real SIGKILL of isolated child process is observed and reconciled read-only",()=>{
  for(const [command,classification,hasOutput] of [
    ["--sigkill-after-stage","staged_only_manual_disposition_required",false],
    ["--sigkill-after-link","published_candidate_without_receipt_reinspect_only",true]
  ]){
    withRun(dir=>{
      const child=spawnSync(process.execPath,[SCRIPT,command,dir],{
        encoding:"utf8",timeout:30000,maxBuffer:32768
      });
      assert.equal(child.error,undefined);
      assert.equal(child.signal,"SIGKILL");
      assert.equal(child.status,null);
      const result=recovery(dir);
      assert.equal(result.classification,classification);
      assert.equal(fs.existsSync(output(dir)),hasOutput);
      assert.equal(result.previous_crash_durability_proven,false);
      assert.equal(result.automatic_output_deletion_attempted,false);
      assert.equal(result.automatic_republication_attempted,false);
    });
  }
});
test("foreign output racing between intent and link is left untouched",()=>{
  withRun(d=>{
    const competitor=Buffer.from("rival output is not ours");
    reject("OUTPUT_CONFLICT",()=>attempt(d,{
      beforeLink({output:dest}){fs.writeFileSync(dest,competitor,{flag:"wx"});}
    }));
    assert.deepEqual(fs.readFileSync(output(d)),competitor);
    assert.equal(recovery(d).classification,"output_conflict_do_not_touch");
  });
});
test("unsupported cross-device or hard-link filesystems fail closed without copying fallback",()=>{
  for(const errno of ["EXDEV","ENOTSUP","EOPNOTSUPP"]){
    withRun(d=>{
      reject("FILESYSTEM_UNSUPPORTED",()=>attempt(d,{simulateLinkError:errno}));
      assert.equal(fs.existsSync(output(d)),false);
      assert.equal(recovery(d).classification,"staged_only_manual_disposition_required");
    });
  }
});
test("post-link directory sync failure retains output but NEVER claims verified durability",()=>{
  withRun(d=>{
    reject("DURABILITY_UNVERIFIED",()=>attempt(d,{simulateDirectoryFsyncFailure:true}));
    assert.equal(sha(fs.readFileSync(output(d))),CANDIDATE_SHA);
    const r=recovery(d);
    assert.equal(r.classification,"published_candidate_without_receipt_reinspect_only");
    assert.equal(r.existing_verified_receipt,false);
    assert.equal(r.previous_crash_durability_proven,false);
  });
});
test("source bytes and identity replacement before publication are rejected",()=>{
  withRun(d=>{
    reject("SOURCE_VERSION_CONFLICT",()=>attempt(d,{
      beforeLink({source:file}){fs.appendFileSync(file,Buffer.from([0xab]));}
    }));
    assert.equal(fs.existsSync(output(d)),false);
    assert.equal(recovery(d).classification,"source_drift_refuse_automatic_resume");
  });
  withRun(d=>{
    reject("SOURCE_VERSION_CONFLICT",()=>attempt(d,{
      beforeLink({source:file}) {
        const next=file+".replacement";
        fs.copyFileSync(file,next,fs.constants.COPYFILE_EXCL);
        fs.renameSync(next,file);
      }
    }));
    assert.equal(recovery(d).classification,"source_drift_refuse_automatic_resume");
  });
});
test("source changes after stage and before recovery cannot be silently resumed",()=>{
  withRun(d=>{
    attempt(d,{interruptAt:"after_stage"});
    const original=fs.readFileSync(source(d));
    fs.writeFileSync(source(d),Buffer.concat([original,Buffer.from([0xca])]));
    assert.equal(recovery(d).classification,"source_drift_refuse_automatic_resume");
    assert.equal(fs.existsSync(output(d)),false);
  });
});
test("tampered stage or intent is flagged and never promoted",()=>{
  withRun(d=>{
    attempt(d,{interruptAt:"after_stage"});
    fs.appendFileSync(path.join(txn(d),"candidate.psd"),Buffer.from([0x01]));
    reject("RECOVERY_AMBIGUOUS",()=>recovery(d));
  });
  withRun(d=>{
    attempt(d,{interruptAt:"after_stage"});
    const intent=path.join(txn(d),"intent.json");
    const data=JSON.parse(fs.readFileSync(intent,"utf8"));
    data.source_sha256="bad source";
    fs.writeFileSync(intent,JSON.stringify(data));
    reject("RECOVERY_AMBIGUOUS",()=>recovery(d));
  });
});
test("already published output with mutated bytes or symlink never gets overwritten/deleted",()=>{
  withRun(d=>{
    attempt(d,{interruptAt:"after_link"});
    // A hard link shares one inode with the private stage: in-place writes
    // corrupt both. Recovery must fail safely rather than auto-resume.
    fs.writeFileSync(output(d),Buffer.from("changed after first link"));
    reject("RECOVERY_AMBIGUOUS",()=>recovery(d));
    assert.equal(fs.readFileSync(output(d),"utf8"),"changed after first link");
  });
  withRun(d=>{
    attempt(d,{interruptAt:"after_stage"});
    fs.symlinkSync(source(d),output(d));
    assert.equal(recovery(d).classification,"output_conflict_do_not_touch");
    assert.ok(fs.lstatSync(output(d)).isSymbolicLink());
  });
});
test("research directory scopes and source symlink are refused",()=>{
  withRun(d=>{
    reject("RESEARCH_SCOPE_DENIED",()=>attempt(path.dirname(d)));
    const original=path.join(d,"real.psd");
    fs.renameSync(source(d),original);
    fs.symlinkSync(original,source(d));
    reject("UNSAFE_FILE",()=>attempt(d));
  });
  const d=path.join(SCRATCH,"run-link");
  fs.symlinkSync(path.dirname(SCRATCH),d);
  try{reject("RESEARCH_SCOPE_DENIED",()=>attempt(d));}
  finally{fs.unlinkSync(d);}
});
test("tampered transaction output after a recorded receipt is treated as conflicting data",()=>{
  withRun(d=>{
    const verified=attempt(d);
    assert.equal(verified.state,"observed_verified_after_parent_fsync");
    const r=recovery(d);
    assert.equal(r.existing_verified_receipt,true);
    assert.equal(r.classification,"verified_receipt_observed");
    // Atomic pathname replacement keeps the private staging inode intact,
    // unlike in-place writes through either hardlink alias.
    const rival=path.join(d,"rival.psd");
    fs.writeFileSync(rival,Buffer.from("not a candidate"),{flag:"wx"});
    fs.renameSync(rival,output(d));
    const changed=recovery(d);
    assert.equal(changed.existing_verified_receipt,true);
    assert.equal(changed.classification,"output_conflict_do_not_touch");
    assert.equal(changed.output_inode_matches_private_staging,false);
    assert.equal(fs.readFileSync(output(d),"utf8"),"not a candidate");
  });
});
test("racing publisher with byte-identical but separate inode is never credited as our publication",()=>{
  withRun(d=>{
    reject("OUTPUT_CONFLICT",()=>attempt(d,{
      beforeLink({output:dest}){
        fs.writeFileSync(dest,fs.readFileSync(
          path.join(txn(d),"candidate.psd")),{flag:"wx"});
      }
    }));
    const resumed=recovery(d);
    assert.equal(resumed.observed_output_sha256,CANDIDATE_SHA);
    assert.equal(resumed.output_inode_matches_private_staging,false);
    assert.equal(resumed.classification,"output_conflict_do_not_touch");
  });
});
test("directory fsync capability is observed, not assumed universal or power-loss certified",()=>{
  withRun(d=>{
    const state=filesystemInfo(d);
    assert.equal(state.observed_parent_directory_fsync,true);
    assert.equal(typeof state.filesystem_device_id,"string");
    assert.ok(state.statfs_block_size>0);
    reject("DURABILITY_UNVERIFIED",()=>syncDir(d,{simulateFailure:true}));
  });
});
test("committed research snapshot stays explicitly non-production",()=>{
  assert.equal(RECEIPT.milestone,"M4c-12");
  assert.equal(RECEIPT.public_write_authorized,false);
  assert.equal(RECEIPT.summary.deterministic_cutpoints_tested,4);
  assert.equal(RECEIPT.summary.power_failure_durability_verified,false);
  assert.equal(RECEIPT.summary.supported_customer_filesystems_certified,0);
  assert.ok(RECEIPT.cases.every(x=>!x.recovery.automatic_output_deletion_attempted));
});
