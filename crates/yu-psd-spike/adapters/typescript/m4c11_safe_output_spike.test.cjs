"use strict";

const {test}=require("node:test");
const assert=require("node:assert/strict");
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const {
  publishPinnedResearch,validateCandidate,validateSource,
  SCRATCH,BASE,ORIGINAL_SHA,CANDIDATE_SHA
}=require("./m4c11_safe_output_spike.cjs");
const {patchKnownPixel}=require("./m4c8_pixel_rename_research.cjs");
const sha=x=>crypto.createHash("sha256").update(x).digest("hex");

function fixture() {
  fs.mkdirSync(SCRATCH,{recursive:true});
  const dir=fs.mkdtempSync(path.join(SCRATCH,"run-"));
  fs.chmodSync(dir,0o700);
  const source=path.join(dir,"pinned-source.psd");
  fs.copyFileSync(BASE,source,fs.constants.COPYFILE_EXCL);
  return {dir,source,output:path.join(dir,"renamed-research.psd"),
    cleanup:()=>fs.rmSync(dir,{force:true,recursive:true})};
}
function rejected(code,run){
  assert.throws(run,error=>error.code===code,
    "expected research refusal "+code);
}
test("exact source SHA produces a valid staged, verified, no-clobber scratch PSD receipt",()=>{
  const f=fixture();
  try{
    const r=publishPinnedResearch({sourceFile:f.source,outputDir:f.dir});
    assert.equal(r.status,"verified_private_scratch_published");
    assert.equal(r.original_sha256,ORIGINAL_SHA);
    assert.equal(r.output_sha256,CANDIDATE_SHA);
    assert.equal(r.source_version_rechecked,true);
    assert.equal(r.same_filesystem_atomic_hard_link,true);
    assert.equal(r.public_write_authorized,false);
    assert.equal(r.full_opaque_metadata_fidelity_certified,false);
    assert.equal(r.native_photoshop_save_not_performed,true);
    assert.equal(sha(fs.readFileSync(f.output)),CANDIDATE_SHA);
    assert.equal(sha(fs.readFileSync(f.source)),ORIGINAL_SHA);
    assert.equal(fs.readdirSync(f.dir).filter(x=>x.startsWith(".private-stage-")).length,0);
    const verified=validateCandidate(fs.readFileSync(f.output));
    assert.equal(verified.sha256,CANDIDATE_SHA);
  }finally{f.cleanup();}
});
test("existing file and symlink output destination always survive unchanged",()=>{
  const f=fixture();
  try{
    const original=Buffer.from("do-not-overwrite");
    fs.writeFileSync(f.output,original,{flag:"wx"});
    rejected("OUTPUT_CONFLICT",()=>publishPinnedResearch({
      sourceFile:f.source,outputDir:f.dir}));
    assert.deepEqual(fs.readFileSync(f.output),original);
    fs.unlinkSync(f.output);
    fs.symlinkSync(f.source,f.output);
    rejected("OUTPUT_CONFLICT",()=>publishPinnedResearch({
      sourceFile:f.source,outputDir:f.dir}));
    assert.ok(fs.lstatSync(f.output).isSymbolicLink());
    assert.equal(sha(fs.readFileSync(f.source)),ORIGINAL_SHA);
  }finally{f.cleanup();}
});
test("racing publisher wins atomic link; stage cleans without removing rival output",()=>{
  const f=fixture();
  try{
    const other=Buffer.from("created-by-competitor");
    rejected("OUTPUT_CONFLICT",()=>publishPinnedResearch({
      sourceFile:f.source,outputDir:f.dir,
      beforeLink({output}){fs.writeFileSync(output,other,{flag:"wx"});}
    }));
    assert.deepEqual(fs.readFileSync(f.output),other);
    assert.equal(fs.readdirSync(f.dir).filter(x=>x.startsWith(".private-stage-")).length,0);
  }finally{f.cleanup();}
});
test("source version change between staging and publication is rejected",()=>{
  const f=fixture();
  try{
    rejected("SOURCE_VERSION_CONFLICT",()=>publishPinnedResearch({
      sourceFile:f.source,outputDir:f.dir,
      beforeLink({source}){fs.appendFileSync(source,Buffer.from([0x99]));}
    }));
    assert.equal(fs.existsSync(f.output),false);
    assert.equal(fs.readdirSync(f.dir).filter(x=>x.startsWith(".private-stage-")).length,0);
  }finally{f.cleanup();}
});
test("source identity replacement and symlinks are rejected independently of bytes",()=>{
  const f=fixture();
  try{
    rejected("SOURCE_VERSION_CONFLICT",()=>publishPinnedResearch({
      sourceFile:f.source,outputDir:f.dir,
      beforeLink({source}){
        const other=source+".replacement";
        fs.copyFileSync(source,other,fs.constants.COPYFILE_EXCL);
        fs.renameSync(other,source);
      }
    }));
    assert.equal(fs.existsSync(f.output),false);
    const real=path.join(f.dir,"pinned-real.psd");
    fs.renameSync(f.source,real);
    fs.symlinkSync(real,f.source);
    rejected("RESEARCH_UNSAFE_SOURCE",()=>publishPinnedResearch({
      sourceFile:f.source,outputDir:f.dir}));
  }finally{f.cleanup();}
});
test("wrong source hash, invalid candidate, unsafe paths and custom output name fail closed",()=>{
  const f=fixture();
  try{
    const bad=Buffer.from(fs.readFileSync(f.source));
    bad[bad.length-1]^=1;
    fs.writeFileSync(f.source,bad);
    rejected("SOURCE_VERSION_CONFLICT",()=>publishPinnedResearch({
      sourceFile:f.source,outputDir:f.dir}));
    rejected("CANDIDATE_INVALID",()=>validateCandidate(bad));
    rejected("RESEARCH_SCOPE_DENIED",()=>publishPinnedResearch({
      sourceFile:BASE,outputDir:f.dir,outputName:"customer.psd"}));
    rejected("RESEARCH_SCOPE_DENIED",()=>publishPinnedResearch({
      sourceFile:BASE,outputDir:path.dirname(SCRATCH)}));
    rejected("RESEARCH_SCOPE_DENIED",()=>validateSource(path.join(f.dir,"other.psd")));
    assert.equal(fs.existsSync(f.output),false);
  }finally{f.cleanup();}
});
test("source original/candidate remain the deterministic M4c-8 exact sample",()=>{
  const buffer=fs.readFileSync(BASE);
  assert.equal(sha(buffer),ORIGINAL_SHA);
  assert.equal(sha(patchKnownPixel(buffer).bytes),CANDIDATE_SHA);
});
