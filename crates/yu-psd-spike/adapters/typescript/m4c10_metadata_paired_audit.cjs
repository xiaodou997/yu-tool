"use strict";

// Read-only strict boundary analysis of two M4c-10 Photoshop saved pairs.
// Receipts are SHA-bound and incomplete metadata semantics remain BLOCKED.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const assert=require("node:assert/strict");
const {scan}=require("./m4c3_raw_block_inventory.cjs");
const {rawDelta}=require("./m4c7d_photoshop_delta_audit.cjs");
const {extractXmp}=require("./m4c9_metadata_semantics.cjs");
const {resourcePayload}=require("./m4c7d_metadata_forensics.cjs");
const ROOT=path.resolve(__dirname,"../../../../");
const TEMP=path.join(ROOT,"target/m4c10-multilingual-sync");
const PRIOR=require(path.join(ROOT,"docs/data/m4c10-multilingual-sync-v1.json"));
const OUT=path.join(ROOT,"docs/data/m4c10-metadata-paired-v1.json");
const sha=b=>crypto.createHash("sha256").update(b).digest("hex");
function refuse(s){throw Error("M4c-10 metadata audit refusal: "+s);}
function findRun() {
  const candidates=fs.readdirSync(TEMP).filter(x=>x.startsWith("run-"))
    .map(x=>path.join(TEMP,x));
  const matched=candidates.filter(dir=>PRIOR.cases.every(c=>{
    const p=path.join(dir,c.fixture+"-original-saved.psd"),q=path.join(dir,c.fixture+"-synced-saved.psd");
    return fs.existsSync(p)&&fs.existsSync(q)&&
      sha(fs.readFileSync(p))===c.saved_original.sha256 &&
      sha(fs.readFileSync(q))===c.saved_candidate.sha256;
  }));
  if(matched.length!==1)refuse("cannot bind unique existing Photoshop scratch saved pair");
  return matched[0];
}
function auditCase(dir,c) {
  const names=[c.fixture+"-original-saved.psd",c.fixture+"-synced-saved.psd"];
  const b=names.map(x=>{
    const file=path.join(dir,x),st=fs.lstatSync(file);
    if(!st.isFile()||st.isSymbolicLink()||st.size>8*1024*1024)
      refuse("untrusted saved input");
    return fs.readFileSync(file);
  });
  if(sha(b[0])!==c.saved_original.sha256||
      sha(b[1])!==c.saved_candidate.sha256)refuse("hash change since native save");
  const delta=rawDelta(b[0],b[1]);
  const inventory=b.map(scan);
  const xs=b.map((bytes,i)=>{
    const resource=resourcePayload(bytes,inventory[i],1060);
    return extractXmp(resource);
  });
  const xmpChanged=Object.keys(xs[0]).filter(k=>
    JSON.stringify(xs[0][k])!==JSON.stringify(xs[1][k]));
  const exif=b.map((bytes,i)=>resourcePayload(bytes,inventory[i],1058));
  const exifChanged=exif[0].length!==exif[1].length||
    !exif[0].equals(exif[1]);
  const tags=delta.additional_layer_information;
  const resources=delta.image_resources;
  const layers=inventory.map(i=>i.layer_records.length);
  return {
    fixture:c.fixture,control_sha256:c.saved_original.sha256,
    candidate_sha256:c.saved_candidate.sha256,
    saved_layer_counts:layers,
    resources,tags,
    raw_channel_delta:delta.channel_pixels,
    compressed_merged_image_equal:delta.compressed_composite_equal,
    XMP_changed_markers:xmpChanged,
    EXIF_bytes_equal:!exifChanged,
    raw_image_resource_changes_nonzero:
      resources.changed.length+resources.added.length+resources.removed.length>0,
    unknown_metadata_preservation_proven:false,
    production_write_authorized:false
  };
}
function build(){
  const dir=findRun(),cases=PRIOR.cases.map(x=>auditCase(dir,x));
  assert.equal(cases.length,2);
  return {
    schema_version:"1",milestone:"M4c-10",
    operation:"strict SHA-bound saved pair meta comparison read only",
    public_write_authorized:false,
    summary:{
      paired_native_multilingual_cases:cases.length,
      pairs_with_identical_raw_channels:cases.filter(x=>
        x.raw_channel_delta.changed.length===0 &&
        x.raw_channel_delta.added.length===0 &&
        x.raw_channel_delta.removed.length===0).length,
      pairs_with_identical_compressed_merged_image:cases.filter(x=>
        x.compressed_merged_image_equal).length,
      pairs_with_unresolved_metadata:cases.filter(x=>
        x.raw_image_resource_changes_nonzero ||
        x.tags.changed.length>0 ||
        x.tags.added.length>0 ||
        x.tags.removed.length>0).length,
      production_writes_authorized:0
    },cases,
    warning:"Photoshop saves may alter history/identity metadata. Neither XMP nor EXIF nor opaque shmd is globally safe to ignore."
  };
}
function check(report){
  assert.equal(report.public_write_authorized,false);
  assert.equal(report.summary.production_writes_authorized,0);
  assert.equal(report.summary.paired_native_multilingual_cases,2);
  assert.ok(report.cases.every(x=>!x.unknown_metadata_preservation_proven &&
    !x.production_write_authorized));
}
if(require.main===module){
  if(process.argv[2]==="--write"&&process.argv.length===3){
    const report=build();check(report);
    fs.writeFileSync(OUT,JSON.stringify(report,null,2)+"\n");
    console.log(JSON.stringify({mode:"--write",...report.summary}));
  }else if(process.argv[2]==="--check"&&process.argv.length===3){
    const report=JSON.parse(fs.readFileSync(OUT,"utf8"));check(report);
    console.log(JSON.stringify({mode:"--check",offline:true,...report.summary}));
  }else {
    process.stderr.write("usage: node m4c10_metadata_paired_audit.cjs --write|--check\n");
    process.exitCode=2;
  }
}
module.exports={auditCase,build,check};
