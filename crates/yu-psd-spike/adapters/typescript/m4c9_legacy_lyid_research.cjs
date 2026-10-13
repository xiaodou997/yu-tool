"use strict";

// M4c-9: inspect Photoshop-generated multi-language legacy Pascal strings,
// test native stable layer IDs when user-visible names collide, and run one
// SHA-bound scratch-only Photoshop save/reopen of a dual-name patch.
// NO general PSD writer or user-file publication is authorized.
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const { scan } = require("./m4c3_raw_block_inventory.cjs");
const { decodeLuni, utf16be } = require("./m4c5_byte_patch_spike.cjs");
const { legacyRegion } = require("./m4c8_pixel_rename_research.cjs");
const { classifyPhotoshopReadiness } = require("./m4c7c_native_readiness.cjs");
const { independentRead } = require("./m4c6_dual_name_fidelity.cjs");
const { compareSavedPair } = require("./m4c7c_saved_pair_gate.cjs");

const ROOT=path.resolve(__dirname,"../../../../");
const SCRATCH=path.join(ROOT,"target/m4c9-multilingual-lyid");
const OUTPUT=path.join(ROOT,"docs/data/m4c9-encoding-lyid-v1.json");
const KNOWN=require(path.join(ROOT,"docs/data/m4c8-multilingual-photoshop-names-v1.json"));
const IDs=["chinese","cyrillic","duplicate"];
const SHA=x=>crypto.createHash("sha256").update(x).digest("hex");
const utf8=b=>new TextDecoder("gb18030",{fatal:true}).decode(b);
function deny(message){
  const e=new Error(message);e.code="M4C9_RESEARCH_REFUSAL";throw e;
}
function layerEntries(buf) {
  const inv=scan(buf);
  if(inv.header.format!=="psd"||inv.header.bit_depth!==8||
      inv.header.color_mode!==3||inv.layer_records.length!==2)deny("unapproved PSD shape");
  const entries=inv.layer_records.map(r=>{
    const tags=inv.tagged_blocks.filter(t=>t.scope==="layer"&&t.layer_index===r.index);
    const unicode=tags.filter(t=>t.key==="luni");
    const ids=tags.filter(t=>t.key==="lyid");
    if(unicode.length!==1||ids.length!==1||ids[0].length!==4) {
      deny("ambiguous or absent luni/lyid raw identity");
    }
    const id=buf.readUInt32BE(ids[0].offset+8+ids[0].length_width);
    if(id===0)deny("zero layer identity");
    const name=decodeLuni(buf,unicode[0]).value;
    const oldBytes=Buffer.from(r.legacy_name,"latin1");
    let decoded;
    try{decoded=utf8(oldBytes);}catch{decoded=null;}
    return {
      raw_index:r.index,photoshop_lyid:id,unicode_name:name,
      legacy_raw_hex:oldBytes.toString("hex"),
      legacy_as_gb18030:decoded,
      legacy_gb18030_matches_unicode:decoded===name,
      legacy_encoding_certified_for_arbitrary_PSD:false
    };
  });
  if(new Set(entries.map(x=>x.photoshop_lyid)).size!==entries.length)
    deny("duplicate Photoshop lyid values");
  return entries;
}
function strictDuplicatePatch(original,expectedSha) {
  if(SHA(original)!==expectedSha)deny("wrong M4c-8 native duplicate source hash");
  const originalLayers=layerEntries(original);
  if(originalLayers.length!==2 ||
    originalLayers[0].photoshop_lyid!==6 ||
    originalLayers[1].photoshop_lyid!==2 ||
    originalLayers.some(x=>x.unicode_name!=="Layer 0"||
      x.legacy_as_gb18030!=="Layer 0"))deny("not the expected Photoshop duplicate-name document");
  const inv=scan(original);
  const targetId=6;
  const target=originalLayers.find(x=>x.photoshop_lyid===targetId);
  if(!target || target.raw_index!==0)deny("target lyid to raw record mapping changed");
  const tags=inv.tagged_blocks.filter(x=>x.scope==="layer"&&
    x.layer_index===target.raw_index&&x.key==="luni");
  const unicode=decodeLuni(original,tags[0]);
  const encoded=utf16be("Layer A");
  if(encoded.length!==unicode.end-unicode.start)deny("Unicode length changed");
  const legacy=legacyRegion(original,inv.layer_records[target.raw_index],1);
  if(legacy.name!=="Layer 0"||legacy.end-legacy.start!==7)deny("legacy mismatch");
  const patched=Buffer.from(original);
  encoded.copy(patched,unicode.start);
  patched.write("Layer A",legacy.start,"ascii");
  const regions=[{start:unicode.start,end:unicode.end},
    {start:legacy.start,end:legacy.end}];
  let count=0;
  for(let i=0;i<original.length;i++)if(original[i]!==patched[i]){
    count++;
    if(!regions.some(x=>i>=x.start&&i<x.end))deny("non-name byte patched");
  }
  const after=layerEntries(patched);
  if(after[0].photoshop_lyid!==6||after[1].photoshop_lyid!==2||
    after[0].unicode_name!=="Layer A"||after[1].unicode_name!=="Layer 0"||
    after[0].legacy_as_gb18030!=="Layer A")deny("identity or dual-name postcheck failed");
  assert.deepEqual(scan(patched).layer_records.map(x=>x.channels),
    inv.layer_records.map(x=>x.channels));
  assert.deepEqual(scan(patched).merged_image,inv.merged_image);
  assert.deepEqual(scan(patched).image_resources,inv.image_resources);
  return {bytes:patched,sha256:SHA(patched),changed_bytes:count,
    target_lyid:targetId,non_target_lyid:2,
    original_raw_index:target.raw_index};
}
function nativeObserve(file,saveAs) {
  const ready=classifyPhotoshopReadiness();
  if(!ready.ready)deny("Photoshop must be idle: "+ready.status);
  const script=[
    "(function(){var d=null,prev=app.displayDialogs;try{",
    "app.displayDialogs=DialogModes.NO;",
    "d=app.open(new File("+JSON.stringify(file)+"));",
    "var list=[];for(var i=0;i<d.layers.length;i++){var l=d.layers[i];",
    "list.push(String(l.id)+':'+encodeURIComponent(l.name));}",
    ...(saveAs?[
      "var opts=new PhotoshopSaveOptions();opts.layers=true;opts.embedColorProfile=true;",
      "d.saveAs(new File("+JSON.stringify(saveAs)+"),opts,true,Extension.LOWERCASE);",
    ]:[]),
    "return list.join('|');",
    "}finally{if(d)d.close(SaveOptions.DONOTSAVECHANGES);app.displayDialogs=prev;}})()"
  ].join("");
  const osa='tell application id "com.adobe.Photoshop" to do javascript '+
    JSON.stringify(script);
  const r=spawnSync("/usr/bin/osascript",["-e",osa],{
    encoding:"utf8",cwd:ROOT,timeout:90000,maxBuffer:1024*1024
  });
  if(r.error||r.status!==0)deny("Photoshop native read failed: "+
    String(r.error?.message||r.stderr).slice(0,350));
  return r.stdout.trim().split("|").filter(Boolean).map(x=>{
    const idx=x.indexOf(":");
    if(idx<0)deny("invalid native lyid observation");
    const id=Number(x.slice(0,idx));
    if(!Number.isSafeInteger(id)||id<=0)deny("invalid native Photoshop id");
    return {photoshop_layer_id:id,name:decodeURIComponent(x.slice(idx+1))};
  });
}
function capture(from) {
  if(process.platform!=="darwin"||process.versions.node!=="22.23.3")
    deny("requires macOS and pinned Node 22.23.3");
  const source=path.resolve(from);
  if(!path.basename(source).startsWith("run-") ||
    path.basename(path.dirname(source))!=="m4c8-multilingual-native")
    deny("must reference M4c-8 controlled Photoshop scratch run");
  fs.mkdirSync(SCRATCH,{recursive:true});
  const folder=fs.mkdtempSync(path.join(SCRATCH,"run-"));
  const specimens=[];
  for(const id of IDs){
    const ref=KNOWN.cases.find(c=>c.scenario===id);
    if(!ref||!ref.output_sha256)deny("missing Photoshop fixture receipt");
    const input=path.join(source,id+"-native-saved.psd");
    const info=fs.lstatSync(input);
    if(!info.isFile()||info.isSymbolicLink()||info.size>8*1024*1024)
      deny("unsafe fixture source");
    const b=fs.readFileSync(input);
    if(SHA(b)!==ref.output_sha256)deny("SHA-bound source mismatch");
    const output=path.join(folder,id+"-source.psd");
    fs.writeFileSync(output,b,{flag:"wx"});
    specimens.push({fixture:id,source_sha256:ref.output_sha256,
      raw_layers:layerEntries(b),source_unchanged:SHA(fs.readFileSync(input))===ref.output_sha256,
      copied_scratch_unchanged:SHA(fs.readFileSync(output))===ref.output_sha256,
      production_write_authorized:false});
  }
  const chinese=specimens.find(x=>x.fixture==="chinese").raw_layers;
  const cyrillic=specimens.find(x=>x.fixture==="cyrillic").raw_layers;
  const duplicate=specimens.find(x=>x.fixture==="duplicate").raw_layers;
  if(!chinese.some(x=>x.unicode_name==="测试图层"&&x.legacy_gb18030_matches_unicode) ||
     !cyrillic.some(x=>x.unicode_name==="Слой"&&x.legacy_gb18030_matches_unicode))
    deny("legacy character mapping failed");
  if(duplicate.some(x=>x.unicode_name!=="Layer 0") ||
     JSON.stringify(duplicate.map(x=>x.photoshop_lyid))!==JSON.stringify([6,2]))
    deny("duplicate layer IDs not pinned");
  const dupPath=path.join(folder,"duplicate-source.psd");
  const dup=fs.readFileSync(dupPath);
  const patched=strictDuplicatePatch(dup,KNOWN.cases.find(x=>
    x.scenario==="duplicate").output_sha256);
  const input=path.join(folder,"duplicate-lyid-patched.psd");
  const saved=path.join(folder,"duplicate-lyid-saved.psd");
  if(fs.existsSync(input)||fs.existsSync(saved))deny("refuse to overwrite scratch");
  fs.writeFileSync(input,patched.bytes,{flag:"wx"});
  const nativeBefore=nativeObserve(dupPath);
  const nativePatched=nativeObserve(input,saved);
  const nativeReopened=nativeObserve(saved);
  const savedData=fs.readFileSync(saved);
  const nativeIdBindingOk=
    nativeBefore.some(x=>x.photoshop_layer_id===6&&x.name==="Layer 0")&&
    nativeBefore.some(x=>x.photoshop_layer_id===2&&x.name==="Layer 0")&&
    nativePatched.some(x=>x.photoshop_layer_id===6&&x.name==="Layer A")&&
    nativePatched.some(x=>x.photoshop_layer_id===2&&x.name==="Layer 0")&&
    nativeReopened.some(x=>x.photoshop_layer_id===6&&x.name==="Layer A")&&
    nativeReopened.some(x=>x.photoshop_layer_id===2&&x.name==="Layer 0");
  const independent=independentRead(saved);
  const afterRaw=layerEntries(savedData);
  const postSavedIdsOk=afterRaw.some(x=>
    x.photoshop_lyid===6&&x.unicode_name==="Layer A")&&
    afterRaw.some(x=>x.photoshop_lyid===2&&x.unicode_name==="Layer 0");
  if(!nativeIdBindingOk||!postSavedIdsOk)deny("stable native layer identity not verified");
  assert.equal(SHA(fs.readFileSync(dupPath)),KNOWN.cases.find(x=>
    x.scenario==="duplicate").output_sha256);
  assert.equal(SHA(fs.readFileSync(input)),patched.sha256);
  const result={
    schema_version:"1",milestone:"M4c-9",
    operation:"scratch only Photoshop legacy encoding and lyid identity study",
    public_write_authorized:false,
    summary:{language_fixtures:2,
      language_legacy_gb18030_equals_luni:2,
      duplicate_layer_ids_unique:2,
      native_photoshop_lyid_rename_reopen_verified:nativeIdBindingOk?1:0,
      production_writes_authorized:0},
    cases:specimens,
    duplicate_identity_experiment:{
      source_sha256:KNOWN.cases.find(x=>x.scenario==="duplicate").output_sha256,
      exact_stable_lyid_requested:6,
      non_target_lyid:2,
      source_layer_names_ambiguous:true,
      touched_original_bytes:patched.changed_bytes,
      candidate_sha256:patched.sha256,
      saved_copy_sha256:SHA(savedData),
      photoshop_before:nativeBefore,
      photoshop_after_patch:nativePatched,
      photoshop_saved_reopened:nativeReopened,
      psd_tools_saved_layer_names:independent.layers.map(x=>x.name),
      saved_raw_layer_ids:afterRaw.map(x=>({id:x.photoshop_lyid,name:x.unicode_name})),
      native_id_binding_verified:nativeIdBindingOk,
      no_write_authorization:true,
    },
    remaining_uncertainty:[
      "GB18030 matches observed Photoshop locale fixtures only, not a general PSD locale guarantee",
      "lyid stable on these exact Photoshop saves but must not be treated as globally unique across documents",
      "any opaque metadata changed by native Photoshop save is not certified preserved"
    ]
  };
  return result;
}
function check(r) {
  assert.equal(r.milestone,"M4c-9");
  assert.equal(r.public_write_authorized,false);
  assert.equal(r.summary.production_writes_authorized,0);
  assert.equal(r.summary.language_legacy_gb18030_equals_luni,2);
  assert.equal(r.summary.native_photoshop_lyid_rename_reopen_verified,1);
  assert.ok(r.cases.every(x=>x.production_write_authorized===false &&
    x.source_unchanged&&x.copied_scratch_unchanged));
}
if(require.main===module){
  const mode=process.argv[2];
  if(mode==="--write"&&process.argv.length===4){
    const report=capture(process.argv[3]);check(report);
    fs.writeFileSync(OUTPUT,JSON.stringify(report,null,2)+"\n");
    console.log(JSON.stringify({mode,...report.summary}));
  }else if(mode==="--check"&&process.argv.length===3){
    const report=JSON.parse(fs.readFileSync(OUTPUT,"utf8"));check(report);
    console.log(JSON.stringify({mode,offline:true,...report.summary}));
  }else {
    process.stderr.write("usage: node m4c9_legacy_lyid_research.cjs --write PREVIOUS_M4C8_SCRATCH_RUN|--check\n");
    process.exitCode=2;
  }
}
module.exports={layerEntries,strictDuplicatePatch,check,capture};
