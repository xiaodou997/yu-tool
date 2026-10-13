"use strict";

// M4c-10 — pinned, RESEARCH-ONLY same-width legacy+luni byte patch.
// This deliberately has no CLI for customer paths, no generic locale codec,
// no production publication, and no PSD source replacement.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const assert=require("node:assert/strict");
const {scan}=require("./m4c3_raw_block_inventory.cjs");
const {decodeLuni,utf16be}=require("./m4c5_byte_patch_spike.cjs");
const {legacyRegion}=require("./m4c8_pixel_rename_research.cjs");
const {layerEntries}=require("./m4c9_legacy_lyid_research.cjs");
const {classifyPhotoshopReadiness}=require("./m4c7c_native_readiness.cjs");
const {readPhotoshopLayers}=require("./m4c6_photoshop_readonly.cjs");
const {independentRead}=require("./m4c6_dual_name_fidelity.cjs");
const {spawnSync}=require("node:child_process");
const ROOT=path.resolve(__dirname,"../../../../");
const PREVIOUS=require(path.join(ROOT,"docs/data/m4c8-multilingual-photoshop-names-v1.json"));
const OUTPUT=path.join(ROOT,"docs/data/m4c10-multilingual-sync-v1.json");
const TEMP=path.join(ROOT,"target/m4c10-multilingual-sync");
const BASE=process.env.M4C10_M4C8_SOURCE_RUN ||
  "/Users/luoxiaodou/workspace/yu-workspace/.webcodex-worktrees/yu-tool-pr22-72bbe9b7/target/m4c8-multilingual-native/run-whZZDB";
const sha=b=>crypto.createHash("sha256").update(b).digest("hex");
const encoder=new TextDecoder("gb18030",{fatal:true});
const CASES=[
  {id:"chinese",old:"测试图层",next:"实验图层",oldGb:"b2e2cad4cdbcb2e3",nextGb:"cab5d1e9cdbcb2e3"},
  {id:"cyrillic",old:"Слой",next:"Тест",oldGb:"a7b3a7dda7e0a7db",nextGb:"a7b4a7d6a7e3a7e4"}
];
function refuse(reason){const e=new Error(reason);e.code="M4C10_REFUSED";throw e;}
function scratchNativeSave(input,output){
  if(path.dirname(input)!==path.dirname(output) ||
    !input.startsWith(TEMP+path.sep)||!output.startsWith(TEMP+path.sep))
    refuse("saveAs restricted to ignored scratch directory");
  if(fs.existsSync(output))refuse("no overwrite of Photoshop output");
  const state=classifyPhotoshopReadiness();
  if(!state.ready)refuse("Photoshop must have zero documents: "+state.status);
  const script=[
    "(function(){var d=null,prev=app.displayDialogs;try{",
    "app.displayDialogs=DialogModes.NO;d=app.open(new File("+JSON.stringify(input)+"));",
    "var opt=new PhotoshopSaveOptions();opt.layers=true;opt.embedColorProfile=true;",
    "d.saveAs(new File("+JSON.stringify(output)+"),opt,true,Extension.LOWERCASE);",
    "return 'saved';",
    "}finally{if(d)d.close(SaveOptions.DONOTSAVECHANGES);app.displayDialogs=prev;}})()"
  ].join("");
  const osa='tell application id "com.adobe.Photoshop" to do javascript '+JSON.stringify(script);
  const r=spawnSync("/usr/bin/osascript",["-e",osa],{
    encoding:"utf8",cwd:ROOT,timeout:90000,maxBuffer:1024*1024
  });
  if(r.error||r.status!==0||r.stdout.trim()!=="saved")
    refuse("Photoshop scratch save failed: "+String(r.error?.message||r.stderr||r.stdout).slice(0,300));
  return {bytes:fs.statSync(output).size,sha256:sha(fs.readFileSync(output))};
}
function patchSynced(bytes,expectedSha,c){
  if(!Buffer.isBuffer(bytes)||sha(bytes)!==expectedSha)refuse("unrecognized native Photoshop source SHA");
  const before=scan(bytes);
  if(before.header.format!=="psd"||before.header.bit_depth!==8 ||
    before.header.color_mode!==3||before.layer_records.length!==2)
    refuse("PSD scope violation");
  const old=layerEntries(bytes);
  if(old[0].photoshop_lyid!==6||old[1].photoshop_lyid!==2 ||
    old[0].unicode_name!==c.old||old[1].unicode_name!=="Layer 0")
    refuse("lyid/Unicode target mismatch");
  if(old[0].legacy_raw_hex!==c.oldGb||old[0].legacy_as_gb18030!==c.old)
    refuse("unknown legacy encoding or source name");
  if(encoder.decode(Buffer.from(c.nextGb,"hex"))!==c.next ||
    encoder.decode(Buffer.from(c.oldGb,"hex"))!==c.old)
    refuse("codec proof mismatch");
  const block=before.tagged_blocks.filter(t=>t.scope==="layer"&&
    t.layer_index===0&&t.key==="luni");
  if(block.length!==1)refuse("luni not unique");
  const luni=decodeLuni(bytes,block[0]);
  const nextUnicode=utf16be(c.next);
  const nextLegacy=Buffer.from(c.nextGb,"hex");
  const legacy=legacyRegion(bytes,before.layer_records[0],before.header.version);
  if(luni.value!==c.old||nextUnicode.length!==(luni.end-luni.start) ||
    legacy.end-legacy.start!==nextLegacy.length ||
    bytes.subarray(legacy.start,legacy.end).toString("hex")!==c.oldGb)
    refuse("name length or raw byte binding mismatch");
  const edited=Buffer.from(bytes);
  nextUnicode.copy(edited,luni.start);
  nextLegacy.copy(edited,legacy.start);
  const allowed=[{start:luni.start,end:luni.end},{start:legacy.start,end:legacy.end}];
  let changed=0;
  for(let i=0;i<bytes.length;i++)if(bytes[i]!==edited[i]){
    changed++;
    if(!allowed.some(x=>i>=x.start&&i<x.end))refuse("non-name bytes modified");
  }
  if(!changed)refuse("no rename performed");
  const after=scan(edited),verified=layerEntries(edited);
  assert.equal(after.source.bytes,before.source.bytes);
  assert.deepEqual(after.image_resources,before.image_resources);
  assert.deepEqual(after.merged_image,before.merged_image);
  assert.deepEqual(after.layer_records.map(r=>r.channels),before.layer_records.map(r=>r.channels));
  if(verified[0].unicode_name!==c.next||verified[0].legacy_as_gb18030!==c.next ||
    verified[0].photoshop_lyid!==6||verified[1].photoshop_lyid!==2)
    refuse("postpatch two-name/lyid check failed");
  return {bytes:edited,sha256:sha(edited),changed_bytes:changed,
    legacy_and_unicode_name_synchronized:true,lyid:6};
}
function build(){
  if(process.platform!=="darwin"||process.versions.node!=="22.23.3")
    refuse("requires pinned Node22 and macOS");
  if(!path.basename(BASE).startsWith("run-") ||
      path.basename(path.dirname(BASE))!=="m4c8-multilingual-native")
    refuse("research source must be prior M4c-8 isolated scratch run");
  const ready=classifyPhotoshopReadiness();
  if(!ready.ready)refuse("native editor unavailable or user documents open");
  const nativeSource=BASE;
  fs.mkdirSync(TEMP,{recursive:true});
  const run=fs.mkdtempSync(path.join(TEMP,"run-"));
  const results=[];
  for(const c of CASES){
    const receipt=PREVIOUS.cases.find(x=>x.scenario===c.id);
    if(!receipt||!receipt.output_sha256)refuse("missing prior source receipt");
    const src=path.join(nativeSource,c.id+"-native-saved.psd");
    const info=fs.lstatSync(src);
    if(!info.isFile()||info.isSymbolicLink()||info.size>8*1024*1024)
      refuse("invalid source scratch file");
    const b=fs.readFileSync(src),p=patchSynced(b,receipt.output_sha256,c);
    const untouched=path.join(run,c.id+"-original.psd"),
      altered=path.join(run,c.id+"-synced.psd"),
      savedOriginal=path.join(run,c.id+"-original-saved.psd"),
      savedEdited=path.join(run,c.id+"-synced-saved.psd");
    fs.writeFileSync(untouched,b,{flag:"wx"});
    fs.writeFileSync(altered,p.bytes,{flag:"wx"});
    const nativeOriginal=readPhotoshopLayers(untouched);
    const nativeEdited=readPhotoshopLayers(altered);
    const control=scratchNativeSave(untouched,savedOriginal);
    const candidate=scratchNativeSave(altered,savedEdited);
    const nativeControlReopened=readPhotoshopLayers(savedOriginal);
    const nativeCandidateReopened=readPhotoshopLayers(savedEdited);
    const refControl=independentRead(savedOriginal),refEdited=independentRead(savedEdited);
    const after=layerEntries(fs.readFileSync(savedEdited));
    const nativeCorrect=nativeOriginal.some(x=>x.name===c.old)&&
      nativeEdited.some(x=>x.name===c.next)&&
      nativeControlReopened.some(x=>x.name===c.old)&&
      nativeCandidateReopened.some(x=>x.name===c.next);
    const identityCorrect=after[0].photoshop_lyid===6&&
      after[0].unicode_name===c.next&&
      after[0].legacy_as_gb18030===c.next&&
      after[1].photoshop_lyid===2&&after[1].unicode_name==="Layer 0";
    const pixelsEqual=refControl.independent_preview.status==="rendered"&&
      refControl.independent_preview.rgba_sha256===
      refEdited.independent_preview.rgba_sha256;
    if(!nativeCorrect||!identityCorrect||!pixelsEqual)
      refuse("native or independent saved fixture mismatch");
    assert.equal(sha(fs.readFileSync(src)),receipt.output_sha256);
    assert.equal(sha(fs.readFileSync(untouched)),receipt.output_sha256);
    assert.equal(sha(fs.readFileSync(altered)),p.sha256);
    results.push({
      fixture:c.id,source_sha256:receipt.output_sha256,
      candidate_sha256:p.sha256,
      requested_name:c.next,original_name:c.old,
      changed_name_bytes:p.changed_bytes,
      UTF16BE_and_GB18030_synchronized:true,
      saved_original:control,saved_candidate:candidate,
      photoshop_original_names:nativeOriginal,
      photoshop_candidate_names:nativeEdited,
      photoshop_reopened_candidate_names:nativeCandidateReopened,
      native_name_reopen_verified:nativeCorrect,
      saved_lyid_and_dual_name_verified:identityCorrect,
      independent_saved_composite_equal:pixelsEqual,
      full_opaque_metadata_semantics_certified:false,
      source_and_inputs_unchanged:true,
      public_write_authorized:false
    });
  }
  return {schema_version:"1",milestone:"M4c-10",
    public_write_authorized:false,
    summary:{paired_native_runs:results.length,
      synchronized_multilingual_names_reopened:results.filter(x=>
        x.native_name_reopen_verified&&x.saved_lyid_and_dual_name_verified).length,
      metadata_fidelity_certifications:0,production_writes_authorized:0},
    cases:results};
}
function check(obj){
  assert.equal(obj.milestone,"M4c-10");
  assert.equal(obj.public_write_authorized,false);
  assert.equal(obj.summary.production_writes_authorized,0);
  assert.equal(obj.summary.paired_native_runs,2);
  assert.ok(obj.cases.every(x=>x.public_write_authorized===false));
}
if(require.main===module){
  const mode=process.argv[2];
  if(mode==="--write"&&process.argv.length===3){
    const data=build();check(data);
    fs.writeFileSync(OUTPUT,JSON.stringify(data,null,2)+"\n");
    console.log(JSON.stringify({mode,...data.summary}));
  }else if(mode==="--check"&&process.argv.length===3){
    const data=JSON.parse(fs.readFileSync(OUTPUT,"utf8"));check(data);
    console.log(JSON.stringify({mode,offline:true,...data.summary}));
  }else{
    process.stderr.write("usage: node m4c10_multilingual_dual_sync.cjs --write|--check\n");
    process.exitCode=2;
  }
}
module.exports={patchSynced,check,build,CASES};
