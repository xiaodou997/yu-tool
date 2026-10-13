"use strict";

// Research-only: Unicode name-only patch *on exact Photoshop-generated
// scratch files*. Deliberately leave legacy Pascal bytes intact to test
// what Photoshop reads and whether Photoshop normalizes them on save.
// Such patches are NOT safe for publication and remain denied.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const assert=require("node:assert/strict");
const {scan}=require("./m4c3_raw_block_inventory.cjs");
const {decodeLuni,utf16be}=require("./m4c5_byte_patch_spike.cjs");
const {classifyPhotoshopReadiness}=require("./m4c7c_native_readiness.cjs");
const {readPhotoshopLayers}=require("./m4c6_photoshop_readonly.cjs");
const {independentRead}=require("./m4c6_dual_name_fidelity.cjs");

const ROOT=path.resolve(__dirname,"../../../../");
const BASE=path.join(ROOT,"target/m4c8-multilingual-native");
const SOURCE_REPORT=path.join(ROOT,"docs/data/m4c8-multilingual-photoshop-names-v1.json");
const REPORT=path.join(ROOT,"docs/data/m4c8-luni-only-multilingual-v1.json");
const CASES=[
  {id:"chinese",old:"测试图层",now:"实验图层"},
  {id:"cyrillic",old:"Слой",now:"Тест"},
];
const sha=b=>crypto.createHash("sha256").update(b).digest("hex");
function findSourceFolder(report){
  const candidates=fs.readdirSync(BASE).filter(x=>x.startsWith("run-"))
    .map(x=>path.join(BASE,x));
  const matching=candidates.filter(dir=>report.cases.every(c=>{
    if(!c.output_sha256)return false;
    const file=path.join(dir,c.scenario+"-native-saved.psd");
    return fs.existsSync(file)&&sha(fs.readFileSync(file))===c.output_sha256;
  }));
  if(matching.length!==1)throw Error("no unique Photoshop-generated exact-hash scratch corpus");
  return matching[0];
}
function patchLuniOnly(source,expectedSha,oldName,newName){
  if(sha(source)!==expectedSha)throw Error("source hash does not match native evidence");
  const original=scan(source);
  if(original.header.format!=="psd"||original.header.bit_depth!==8||
     original.layer_records.length!==2)throw Error("unsupported scratch PSD");
  const t=original.tagged_blocks.filter(x=>x.scope==="layer"&&
    x.layer_index===0&&x.key==="luni");
  if(t.length!==1)throw Error("ambiguous Unicode name");
  const info=decodeLuni(source,t[0]),replacement=utf16be(newName);
  if(info.value!==oldName||replacement.length!==(info.end-info.start))
    throw Error("unsupported UTF-16 name length or old name");
  const result=Buffer.from(source);
  replacement.copy(result,info.start);
  for(let i=0;i<source.length;i++)if(source[i]!==result[i]&&
      (i<info.start||i>=info.end))throw Error("modified non-luni bytes");
  const checked=scan(result);
  assert.deepEqual(checked.image_resources,original.image_resources);
  assert.deepEqual(checked.layer_records.map(x=>x.channels),
    original.layer_records.map(x=>x.channels));
  assert.deepEqual(checked.merged_image,original.merged_image);
  assert.equal(checked.layer_records[0].legacy_name,
    original.layer_records[0].legacy_name);
  return {bytes:result,source_sha256:expectedSha,candidate_sha256:sha(result),
    legacy_pascal_untouched:true,original_raw_legacy:original.layer_records[0].legacy_name};
}
function build(){
  if(process.platform!=="darwin"||process.versions.node!=="22.23.3")
    throw Error("requires macOS Photoshop and Node 22.23.3");
  const state=classifyPhotoshopReadiness();
  if(!state.ready)throw Error("Photoshop not idle: "+state.status);
  const recorded=JSON.parse(fs.readFileSync(SOURCE_REPORT,"utf8"));
  const folder=findSourceFolder(recorded);
  const results=[];
  for(const c of CASES){
    const old=recorded.cases.find(x=>x.scenario===c.id);
    const f=path.join(folder,c.id+"-native-saved.psd");
    const source=fs.readFileSync(f);
    const patched=patchLuniOnly(source,old.output_sha256,c.old,c.now);
    const dest=path.join(folder,c.id+"-luni-only-research.psd");
    if(fs.existsSync(dest))throw Error("isolated result exists, refuse overwrite");
    fs.writeFileSync(dest,patched.bytes,{flag:"wx"});
    const native=readPhotoshopLayers(dest);
    const independent=independentRead(dest);
    assert.equal(sha(fs.readFileSync(f)),old.output_sha256);
    assert.equal(sha(fs.readFileSync(dest)),patched.candidate_sha256);
    results.push({
      scenario:c.id,old_name:c.old,new_name:c.now,
      unicode_patch_native_reopen_shows_new_name:native.some(x=>x.name===c.now),
      unicode_patch_psd_tools_shows_new_name:independent.layers.some(x=>x.name===c.now),
      legacy_pascal_untouched:true,
      old_legacy_pascal_bytes_latin1:patched.original_raw_legacy,
      source_sha256:patched.source_sha256,
      candidate_sha256:patched.candidate_sha256,
      native_layers:native,
      public_write_authorized:false,
      status:"research_only_legacy_divergent",
    });
  }
  return {schema_version:"1",milestone:"M4c-8",
    operation:"exact native fixture luni-only compatibility test",
    public_write_authorized:false,
    summary:{tested:results.length,photoshop_unicode_name_read_matches:
      results.filter(x=>x.unicode_patch_native_reopen_shows_new_name).length,
      unresolved_legacy_mismatch:results.length,production_writes_authorized:0},
    cases:results};
}
function check(r){
  assert.equal(r.summary.tested,2);
  assert.equal(r.public_write_authorized,false);
  assert.equal(r.summary.production_writes_authorized,0);
  assert.ok(r.cases.every(x=>x.public_write_authorized===false&&
    x.legacy_pascal_untouched));
}
if(require.main===module){
  const mode=process.argv[2];
  if(mode==="--write"&&process.argv.length===3){
    const r=build();check(r);
    fs.writeFileSync(REPORT,JSON.stringify(r,null,2)+"\n");
    console.log(JSON.stringify({mode,...r.summary}));
  }else if(mode==="--check"&&process.argv.length===3){
    const r=JSON.parse(fs.readFileSync(REPORT,"utf8"));check(r);
    console.log(JSON.stringify({mode,offline:true,...r.summary}));
  }else{
    process.stderr.write("usage: node m4c8_luni_legacy_divergence.cjs --write|--check\n");
    process.exitCode=2;
  }
}
module.exports={patchLuniOnly,build,check};
