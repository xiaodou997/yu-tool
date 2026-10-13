"use strict";

// M4c-11 — tightly bounded *research-only* PSD publication experiment.
// This must not be wired to the public yu CLI or external PSD paths.
// The only accepted bytes are the exact SHA-pinned normal-pixel M4c-8
// source and its deterministic two-name, same-width experimental patch.
// A same-filesystem hard link is an atomic no-overwrite publication
// primitive. It does NOT by itself prove a production PSD writer is safe.
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const assert = require("node:assert/strict");
const {scan} = require("./m4c3_raw_block_inventory.cjs");
const {patchKnownPixel,ORIGINAL_SHA} = require("./m4c8_pixel_rename_research.cjs");
const KNOWN = require("../../../../docs/data/m4c8-pixel-photoshop-fidelity-v1.json");

const ROOT = path.resolve(__dirname,"../../../../");
const SCRATCH = path.join(ROOT,"target/m4c11-safe-output");
const BASE = path.join(ROOT,
  "fixtures/psd/benchmark/upstream/psd-tools/background-red-opacity-80.psd");
const REPORT = path.join(ROOT,"docs/data/m4c11-safe-output-v1.json");
const sha = bytes=>crypto.createHash("sha256").update(bytes).digest("hex");
const CANDIDATE_SHA = KNOWN.cases[0].candidate_sha256;

function refuse(code,message){
  const error = new Error(message);
  error.code = code;
  throw error;
}
function requireRegularNoSymlink(file){
  const st=fs.lstatSync(file);
  if(!st.isFile()||st.isSymbolicLink()||st.size>8*1024*1024)
    refuse("RESEARCH_UNSAFE_SOURCE","source must be a small ordinary file, not symlink");
  return st;
}
function checkAbsent(file){
  try{fs.lstatSync(file);refuse("OUTPUT_CONFLICT","output already exists, including symlink");}
  catch(e){if(e.code!=="ENOENT")throw e;}
}
function validateOutputDir(dir){
  const resolved=path.resolve(dir);
  if(path.dirname(resolved)!==SCRATCH ||
    !path.basename(resolved).startsWith("run-"))
    refuse("RESEARCH_SCOPE_DENIED","only new target/m4c11-safe-output/run-* allowed");
  const base=fs.lstatSync(SCRATCH);
  if(!base.isDirectory()||base.isSymbolicLink())
    refuse("RESEARCH_SCOPE_DENIED","research scratch root cannot be a symlink");
  const info=fs.lstatSync(resolved);
  if(!info.isDirectory()||info.isSymbolicLink())
    refuse("RESEARCH_SCOPE_DENIED","output folder must be a real directory");
  return resolved;
}
function validateCandidate(data){
  if(!Buffer.isBuffer(data)||sha(data)!==CANDIDATE_SHA)
    refuse("CANDIDATE_INVALID","only the pinned research candidate is accepted");
  const inv=scan(data);
  if(inv.header.format!=="psd"||inv.header.bit_depth!==8||
     inv.header.color_mode!==3||inv.layer_records.length!==2 ||
     inv.layer_records[0].legacy_name!=="Layer A")
    refuse("CANDIDATE_INVALID","candidate PSD verification failed");
  return {sha256:sha(data),bytes:data.length,layer_records:2};
}
function validateSource(sourceFile) {
  const file=path.resolve(sourceFile);
  const allowed=file===BASE || (file.startsWith(SCRATCH+path.sep) &&
    path.basename(file)==="pinned-source.psd" &&
    path.basename(path.dirname(file)).startsWith("run-") &&
    path.dirname(path.dirname(file))===SCRATCH);
  if(!allowed)refuse("RESEARCH_SCOPE_DENIED","source path outside exact fixture and scratch");
  const st=requireRegularNoSymlink(file);
  const bytes=fs.readFileSync(file);
  if(sha(bytes)!==ORIGINAL_SHA)
    refuse("SOURCE_VERSION_CONFLICT","source SHA-256 differs from frozen fixture");
  return {file,bytes,device:st.dev,inode:st.ino,size:st.size,sha256:sha(bytes)};
}
function recheckSource(v){
  const st=requireRegularNoSymlink(v.file);
  if(st.dev!==v.device||st.ino!==v.inode||st.size!==v.size ||
      sha(fs.readFileSync(v.file))!==v.sha256)
    refuse("SOURCE_VERSION_CONFLICT","source replaced or modified before publication");
}
function publishPinnedResearch({sourceFile,outputDir,outputName="renamed-research.psd",beforeLink}){
  if(outputName!=="renamed-research.psd")
    refuse("RESEARCH_SCOPE_DENIED","only fixed research output name is allowed");
  const directory=validateOutputDir(outputDir);
  const output=path.join(directory,outputName);
  checkAbsent(output);
  const source=validateSource(sourceFile);
  const patch=patchKnownPixel(source.bytes);
  const candidate=validateCandidate(patch.bytes);
  assert.equal(candidate.sha256,patch.sha256);
  const tempFolder=fs.mkdtempSync(path.join(directory,".private-stage-"));
  fs.chmodSync(tempFolder,0o700);
  const staged=path.join(tempFolder,"candidate.psd");
  try{
    const fd=fs.openSync(staged,fs.constants.O_CREAT|fs.constants.O_EXCL|fs.constants.O_WRONLY,0o600);
    try{
      fs.writeFileSync(fd,patch.bytes);
      fs.fsyncSync(fd);
    }finally{fs.closeSync(fd);}
    validateCandidate(fs.readFileSync(staged));
    // Test-only hook can model racing publication or source mutation.
    if(beforeLink!==undefined){
      if(typeof beforeLink!=="function")
        refuse("RESEARCH_SCOPE_DENIED","invalid test hook");
      beforeLink({source:source.file,output});
    }
    recheckSource(source);
    // Do not replace an existing file. The hard link is the atomic
    // no-clobber operation; an earlier lstat check is NOT sufficient.
    try{fs.linkSync(staged,output);}
    catch(e){
      if(e.code==="EEXIST")
        refuse("OUTPUT_CONFLICT","racing publisher won; not overwritten");
      refuse("PUBLICATION_UNSUPPORTED","atomic hard-link publication unavailable: "+e.code);
    }
    const result=validateCandidate(fs.readFileSync(output));
    if(result.sha256!==candidate.sha256)
      refuse("POST_PUBLISH_INCONCLUSIVE","published bytes changed after linking");
    const still=fs.readFileSync(source.file);
    if(sha(still)!==ORIGINAL_SHA)
      refuse("POST_PUBLISH_INCONCLUSIVE","source changed after publication");
    return {
      milestone:"M4c-11",kind:"research_only_nonclobber_receipt",
      status:"verified_private_scratch_published",
      original_sha256:ORIGINAL_SHA,
      output_sha256:result.sha256,
      output_bytes:result.bytes,
      candidate_name:"Layer A",
      source_version_rechecked:true,
      same_filesystem_atomic_hard_link:true,
      output_exists_verified:true,
      native_photoshop_save_not_performed:true,
      full_opaque_metadata_fidelity_certified:false,
      public_write_authorized:false
    };
  }finally{
    // The staged hard link is private; removing it never removes a
    // successfully published target inode. Never delete a published
    // candidate even if post-publication verification was inconclusive.
    fs.rmSync(tempFolder,{recursive:true,force:true});
  }
}
function build(){
  const original=fs.readFileSync(BASE);
  assert.equal(sha(original),ORIGINAL_SHA);
  fs.mkdirSync(SCRATCH,{recursive:true});
  const dir=fs.mkdtempSync(path.join(SCRATCH,"run-"));
  fs.chmodSync(dir,0o700);
  const source=path.join(dir,"pinned-source.psd");
  fs.writeFileSync(source,original,{flag:"wx"});
  const verified=publishPinnedResearch({sourceFile:source,outputDir:dir});
  assert.equal(sha(fs.readFileSync(source)),ORIGINAL_SHA);
  assert.equal(sha(fs.readFileSync(BASE)),ORIGINAL_SHA);
  return {
    schema_version:"1",milestone:"M4c-11",
    public_write_authorized:false,
    scope:"known 8-bit RGB pixel PSD, research output only",
    summary:{private_staged_published:1,sha_bound_verifications:1,
      no_clobber_violations:0,production_writes_authorized:0},
    cases:[verified]
  };
}
function check(r){
  assert.equal(r.milestone,"M4c-11");
  assert.equal(r.public_write_authorized,false);
  assert.equal(r.summary.production_writes_authorized,0);
  assert.equal(r.summary.private_staged_published,1);
  assert.ok(r.cases.every(x=>x.public_write_authorized===false &&
    x.full_opaque_metadata_fidelity_certified===false));
}
if(require.main===module){
  if(process.argv[2]==="--write"&&process.argv.length===3){
    const r=build();check(r);
    fs.writeFileSync(REPORT,JSON.stringify(r,null,2)+"\n");
    console.log(JSON.stringify({mode:"--write",...r.summary}));
  }else if(process.argv[2]==="--check"&&process.argv.length===3){
    const r=JSON.parse(fs.readFileSync(REPORT,"utf8"));check(r);
    console.log(JSON.stringify({mode:"--check",offline:true,...r.summary}));
  }else {
    process.stderr.write("usage: node m4c11_safe_output_spike.cjs --write|--check\n");
    process.exitCode=2;
  }
}
module.exports={publishPinnedResearch,validateCandidate,validateSource,build,check,
  ROOT,SCRATCH,BASE,ORIGINAL_SHA,CANDIDATE_SHA};
