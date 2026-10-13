"use strict";

// M4c-8 controlled byte patch + native Photoshop save/reopen experiment.
// Only one exact, already-committed RGB 8-bit PSD. No user paths are accepted.
// All PSD outputs are new scratch files under ignored target/. Public
// mutation authorization is categorically false even when tests pass.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const assert=require("node:assert/strict");
const {spawnSync}=require("node:child_process");
const {scan}=require("./m4c3_raw_block_inventory.cjs");
const {decodeLuni,utf16be}=require("./m4c5_byte_patch_spike.cjs");
const {classifyPhotoshopReadiness}=require("./m4c7c_native_readiness.cjs");
const {readPhotoshopLayers}=require("./m4c6_photoshop_readonly.cjs");
const {independentRead}=require("./m4c6_dual_name_fidelity.cjs");
const {compareSavedPair}=require("./m4c7c_saved_pair_gate.cjs");

const ROOT=path.resolve(__dirname,"../../../../");
const INPUT=path.join(ROOT,"fixtures/psd/benchmark/upstream/psd-tools/background-red-opacity-80.psd");
const ORIGINAL_SHA="391f4c009b9bd4cc5fdffb3799c3476f7d690874e6ca458463ab7fe3b5739aee";
const TARGET_RAW_INDEX=0;
const OLD_NAME="Layer 1",NEW_NAME="Layer A";
const DEST=path.join(ROOT,"target/m4c8-pixel-research");
const REPORT=path.join(ROOT,"docs/data/m4c8-pixel-photoshop-fidelity-v1.json");
const sha=x=>crypto.createHash("sha256").update(x).digest("hex");
function refuse(message){
  const e=new Error(message);e.code="RESEARCH_ONLY_REFUSAL";throw e;
}
function legacyRegion(source,rec,version){
  const start=rec.offset+16+2+rec.channel_count*(version===2?10:6)+12;
  if(start+12>source.length)refuse("invalid layer record extra section");
  let p=start+4;
  const mask=source.readUInt32BE(p);p+=4+mask;
  const blend=source.readUInt32BE(p);p+=4+blend;
  if(p+1>source.length)refuse("missing legacy Pascal name");
  const size=source[p],nameStart=p+1;
  if(nameStart+size>source.length)refuse("invalid legacy Pascal bounds");
  return {start:nameStart,end:nameStart+size,name:source.toString("latin1",nameStart,nameStart+size)};
}
function patchKnownPixel(source){
  if(!Buffer.isBuffer(source)||sha(source)!==ORIGINAL_SHA)refuse("wrong exact source SHA");
  const inv=scan(source);
  if(inv.header.format!=="psd"||inv.header.bit_depth!==8||inv.header.color_mode!==3||
    inv.layer_records.length!==2)refuse("unsupported source format/bit depth/layer count");
  const rec=inv.layer_records[TARGET_RAW_INDEX],other=inv.layer_records[1];
  if(!rec||rec.legacy_name!==OLD_NAME||other.legacy_name!=="Layer 0"||
    rec.channel_count!==4)refuse("pixel record identity changed");
  const target=inv.tagged_blocks.filter(x=>x.scope==="layer"&&
    x.layer_index===TARGET_RAW_INDEX&&x.key==="luni");
  if(target.length!==1)refuse("target Unicode-name block not unique");
  const raw=decodeLuni(source,target[0]);
  if(raw.value!==OLD_NAME||raw.units!==7||utf16be(NEW_NAME).length!==(raw.end-raw.start))
    refuse("nonmatching Unicode name or length");
  const legacy=legacyRegion(source,rec,inv.header.version);
  if(legacy.name!==OLD_NAME||legacy.end-legacy.start!==7)
    refuse("legacy name mismatches Unicode target");
  const out=Buffer.from(source);
  utf16be(NEW_NAME).copy(out,raw.start);
  out.write(NEW_NAME,legacy.start,"ascii");
  const touched=[{start:raw.start,end:raw.end},{start:legacy.start,end:legacy.end}];
  let byteChanges=0;
  for(let i=0;i<source.length;i++)if(source[i]!==out[i]){
    byteChanges++;
    if(!touched.some(r=>i>=r.start&&i<r.end))refuse("non-name source byte altered");
  }
  if(!byteChanges)refuse("no actual modification");
  const edited=scan(out);
  assert.equal(edited.source.bytes,inv.source.bytes);
  assert.deepEqual(edited.header,inv.header);
  assert.deepEqual(edited.image_resources,inv.image_resources);
  assert.deepEqual(edited.merged_image,inv.merged_image);
  assert.deepEqual(edited.layer_records.map(x=>x.channels),inv.layer_records.map(x=>x.channels));
  assert.equal(edited.layer_records[0].legacy_name,NEW_NAME);
  assert.equal(decodeLuni(out,edited.tagged_blocks.find(x=>
    x.scope==="layer"&&x.layer_index===0&&x.key==="luni")).value,NEW_NAME);
  return {bytes:out,sha256:sha(out),changed_bytes:byteChanges,touched_ranges:touched};
}
function requireNoOutput(file){
  try{fs.lstatSync(file);refuse("refuse existing output or symlink");}
  catch(e){if(e.code!=="ENOENT")throw e;}
}
function nativeSaveCopy(input,output){
  if(path.dirname(input)!==path.dirname(output)||
     !input.startsWith(DEST+path.sep)||!output.startsWith(DEST+path.sep))
    refuse("only isolated scratch run allowed");
  requireNoOutput(output);
  const state=classifyPhotoshopReadiness();
  if(!state.ready)refuse("Photoshop not ready with zero documents: "+state.status);
  const script=[
    "(function(){var d=null,old=app.displayDialogs;try{",
    "app.displayDialogs=DialogModes.NO;",
    "d=app.open(new File("+JSON.stringify(input)+"));",
    "var opts=new PhotoshopSaveOptions();opts.layers=true;opts.embedColorProfile=true;",
    "d.saveAs(new File("+JSON.stringify(output)+"),opts,true,Extension.LOWERCASE);",
    "return 'saved';",
    "}finally{if(d)d.close(SaveOptions.DONOTSAVECHANGES);app.displayDialogs=old;}})()",
  ].join("");
  const osa='tell application id "com.adobe.Photoshop" to do javascript '+JSON.stringify(script);
  const result=spawnSync("/usr/bin/osascript",["-e",osa],{
    encoding:"utf8",timeout:90000,maxBuffer:1024*1024,cwd:ROOT});
  if(result.error||result.status!==0||result.stdout.trim()!=="saved")
    refuse("Photoshop saveAs failed: "+
      String(result.error?.message||result.stderr||result.stdout).slice(0,330));
  if(!fs.lstatSync(output).isFile())refuse("saved output is not regular file");
  return {bytes:fs.statSync(output).size,sha256:sha(fs.readFileSync(output))};
}
function namesOnly(before,after){
  if(before.length!==after.length)return false;
  return before.every((x,i)=>x.kind===after[i].kind&&
    after[i].name===(x.name===OLD_NAME?NEW_NAME:x.name));
}
function build(){
  if(process.platform!=="darwin"||process.versions.node!=="22.23.3")
    refuse("requires macOS and pinned Node 22.23.3");
  const source=fs.readFileSync(INPUT);
  const candidate=patchKnownPixel(source);
  const state=classifyPhotoshopReadiness();
  if(!state.ready)return {
    schema_version:"1",milestone:"M4c-8",status:"native_preflight_blocked",
    native_preflight:state,public_write_authorized:false,
    summary:{photoshop_save_pairs:0,production_write_authorizations:0},cases:[]
  };
  fs.mkdirSync(DEST,{recursive:true});
  const dir=fs.mkdtempSync(path.join(DEST,"run-"));
  const originalPath=path.join(dir,"pixel-original.psd");
  const patchedPath=path.join(dir,"pixel-patched.psd");
  const savedOriginalPath=path.join(dir,"pixel-original-saved.psd");
  const savedPatchedPath=path.join(dir,"pixel-patched-saved.psd");
  fs.writeFileSync(originalPath,source,{flag:"wx"});
  fs.writeFileSync(patchedPath,candidate.bytes,{flag:"wx"});
  const result={fixture:"bench-baseline-rgb",kind:"pixel",
    source_sha256:ORIGINAL_SHA,candidate_sha256:candidate.sha256,
    byte_patch_changed_bytes:candidate.changed_bytes,
    src_bytes_equal_candidate_bytes:source.length===candidate.bytes.length,
    two_name_encodings_updated:true,
    control_saved:false,modified_saved:false,
    public_write_authorized:false};
  try{
    const nativeOld=readPhotoshopLayers(originalPath);
    const nativePatched=readPhotoshopLayers(patchedPath);
    const before=independentRead(originalPath),edited=independentRead(patchedPath);
    result.source_layers=nativeOld;
    result.candidate_layers=nativePatched;
    result.native_input_names_changed_only_target=namesOnly(nativeOld,nativePatched);
    if(!result.native_input_names_changed_only_target)refuse("native input edit not verified");
    result.saved_original=nativeSaveCopy(originalPath,savedOriginalPath);
    result.control_saved=true;
    result.saved_modified=nativeSaveCopy(patchedPath,savedPatchedPath);
    result.modified_saved=true;
    const nativeSavedOriginal=readPhotoshopLayers(savedOriginalPath);
    const nativeSavedPatched=readPhotoshopLayers(savedPatchedPath);
    const referenceSavedOriginal=independentRead(savedOriginalPath);
    const referenceSavedPatched=independentRead(savedPatchedPath);
    result.reopened_control_layers=nativeSavedOriginal;
    result.reopened_modified_layers=nativeSavedPatched;
    result.native_reopen_persists_only_target_name=
      JSON.stringify(nativeOld)===JSON.stringify(nativeSavedOriginal)&&
      JSON.stringify(nativePatched)===JSON.stringify(nativeSavedPatched);
    result.independent_saved_pixel_equal=
      referenceSavedOriginal.independent_preview.status==="rendered" &&
      referenceSavedOriginal.independent_preview.rgba_sha256===
      referenceSavedPatched.independent_preview.rgba_sha256;
    result.independent_saved_tree_only_name_changed=
      before.layers.length===edited.layers.length &&
      referenceSavedOriginal.layers.length===referenceSavedPatched.layers.length &&
      referenceSavedOriginal.layers.every((x,i)=>{
        const y=referenceSavedPatched.layers[i];
        return x.kind===y.kind&&y.name===(x.name===OLD_NAME?NEW_NAME:x.name);
      });
    const raw=compareSavedPair(fs.readFileSync(savedOriginalPath),
      fs.readFileSync(savedPatchedPath),{
        raw_index:TARGET_RAW_INDEX,expected_old_name:OLD_NAME,expected_new_name:NEW_NAME
      });
    result.saved_raw_pair={
      resources:raw.resources,tags:raw.tags,channels:raw.channels,
      merged_composite_equal:raw.merged_composite_equal,
      full_strict_byte_gate_passed:raw.strict_saved_pair_bytes_passed,
      output_byte_lengths:[result.saved_original.bytes,result.saved_modified.bytes],
    };
    result.native_name_reopen_research_passed=
      result.native_reopen_persists_only_target_name &&
      result.independent_saved_pixel_equal &&
      result.independent_saved_tree_only_name_changed;
    result.status=result.native_name_reopen_research_passed?
      "pixel_name_reopen_verified_only_not_writer_approval":
      "pixel_name_reopen_mismatch";
  }catch(e){
    result.status="native_experiment_failed_closed";
    result.error=String(e?.message||e).slice(0,400);
    result.saved_original_output_exists=fs.existsSync(savedOriginalPath);
    result.saved_modified_output_exists=fs.existsSync(savedPatchedPath);
  }
  assert.equal(sha(fs.readFileSync(INPUT)),ORIGINAL_SHA);
  assert.equal(sha(fs.readFileSync(originalPath)),ORIGINAL_SHA);
  assert.equal(sha(fs.readFileSync(patchedPath)),candidate.sha256);
  result.all_original_and_experiment_inputs_unchanged=true;
  return {
    schema_version:"1",milestone:"M4c-8",operation:"pixel_layer_rename_native_research",
    public_write_authorized:false,
    summary:{photoshop_save_pairs:result.control_saved&&result.modified_saved?1:0,
      native_pixel_name_reopen_verified:result.native_name_reopen_research_passed?1:0,
      strict_opaque_metadata_fidelity_certified:0,
      production_write_authorizations:0},
    cases:[result]
  };
}
function check(obj){
  assert.equal(obj.milestone,"M4c-8");
  assert.equal(obj.public_write_authorized,false);
  assert.equal(obj.summary.production_write_authorizations,0);
  assert.ok(obj.cases.every(c=>c.public_write_authorized===false));
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
  }else{
    process.stderr.write("usage: node m4c8_pixel_rename_research.cjs --write|--check\n");
    process.exitCode=2;
  }
}
module.exports={patchKnownPixel,legacyRegion,build,check,ORIGINAL_SHA,OLD_NAME,NEW_NAME};
