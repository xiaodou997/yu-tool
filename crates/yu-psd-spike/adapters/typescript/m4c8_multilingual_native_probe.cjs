"use strict";

// Photoshop-authoritative *fixture creation* research, never a YuTool writer.
// User PSDs are not accepted; Photoshop changes only scratch copies of one
// SHA-pinned source and saves to distinct new scratch PSD destinations.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const assert=require("node:assert/strict");
const {spawnSync}=require("node:child_process");
const {scan}=require("./m4c3_raw_block_inventory.cjs");
const {decodeLuni}=require("./m4c5_byte_patch_spike.cjs");
const {classifyPhotoshopReadiness}=require("./m4c7c_native_readiness.cjs");
const {readPhotoshopLayers}=require("./m4c6_photoshop_readonly.cjs");
const {independentRead}=require("./m4c6_dual_name_fidelity.cjs");
const {ORIGINAL_SHA}=require("./m4c8_pixel_rename_research.cjs");

const ROOT=path.resolve(__dirname,"../../../../");
const SRC=path.join(ROOT,"fixtures/psd/benchmark/upstream/psd-tools/background-red-opacity-80.psd");
const TEMP=path.join(ROOT,"target/m4c8-multilingual-native");
const REPORT=path.join(ROOT,"docs/data/m4c8-multilingual-photoshop-names-v1.json");
const CASES=[
  {id:"chinese",name:"测试图层"},
  {id:"cyrillic",name:"Слой"},
  {id:"duplicate",name:"Layer 0"},
];
const sha=b=>crypto.createHash("sha256").update(b).digest("hex");
function nativeRename(input,output,newName){
  if(path.dirname(input)!==path.dirname(output) ||
      !input.startsWith(TEMP+path.sep)||!output.startsWith(TEMP+path.sep))
    throw Error("unsafe Photoshop research paths");
  if(fs.existsSync(output))throw Error("research output exists");
  const state=classifyPhotoshopReadiness();
  if(!state.ready)throw Error("native Photoshop not idle: "+state.status);
  const script=[
    "(function(){var d=null,prior=app.displayDialogs;try{",
    "app.displayDialogs=DialogModes.NO;",
    "d=app.open(new File("+JSON.stringify(input)+"));",
    "var target=null;for(var i=0;i<d.layers.length;i++){",
    "var l=d.layers[i];if(l.name==='Layer 1' && !l.isBackgroundLayer){target=l;break;}}",
    "if(!target)throw new Error('required ordinary pixel layer missing');",
    "target.name="+JSON.stringify(newName)+";",
    "var options=new PhotoshopSaveOptions();options.layers=true;options.embedColorProfile=true;",
    "d.saveAs(new File("+JSON.stringify(output)+"),options,true,Extension.LOWERCASE);",
    "return 'saved';",
    "}finally{if(d)d.close(SaveOptions.DONOTSAVECHANGES);app.displayDialogs=prior;}})()",
  ].join("");
  const appScript='tell application id "com.adobe.Photoshop" to do javascript '+JSON.stringify(script);
  const response=spawnSync("/usr/bin/osascript",["-e",appScript],{
    encoding:"utf8",timeout:90000,maxBuffer:1024*1024,cwd:ROOT
  });
  if(response.error||response.status!==0||response.stdout.trim()!=="saved")
    throw Error("Photoshop native rename/save failed: "+
      (response.error?.message||response.stderr||response.stdout).slice(0,300));
  if(!fs.lstatSync(output).isFile())throw Error("Photoshop did not save regular PSD");
}
function rawNames(file){
  const b=fs.readFileSync(file),inv=scan(b);
  const results=[];
  for(const r of inv.layer_records){
    const tags=inv.tagged_blocks.filter(t=>t.scope==="layer"&&
      t.layer_index===r.index&&t.key==="luni");
    const unicode=tags.length===1?decodeLuni(b,tags[0]).value:null;
    results.push({raw_index:r.index,unicode,legacy_pascal:r.legacy_name,
      same_exact_encoding:unicode===r.legacy_name});
  }
  return results;
}
function build(){
  if(process.platform!=="darwin"||process.versions.node!=="22.23.3")
    throw Error("requires macOS Photoshop and pinned Node 22.23.3");
  const b=fs.readFileSync(SRC);
  assert.equal(sha(b),ORIGINAL_SHA);
  const state=classifyPhotoshopReadiness();
  if(!state.ready)throw Error("Photoshop native app not ready: "+state.status);
  fs.mkdirSync(TEMP,{recursive:true});
  const folder=fs.mkdtempSync(path.join(TEMP,"run-"));
  const results=[];
  for(const c of CASES){
    const input=path.join(folder,c.id+"-input.psd");
    const output=path.join(folder,c.id+"-native-saved.psd");
    fs.writeFileSync(input,b,{flag:"wx"});
    const entry={scenario:c.id,requested_name:c.name,
      source_sha256:ORIGINAL_SHA,
      operation:"Photoshop edits scratch copy then native saveAs, no YuTool PSD writer",
      native_save_completed:false,public_write_authorized:false};
    try{
      nativeRename(input,output,c.name);
      entry.native_save_completed=true;
      const layers=readPhotoshopLayers(output);
      const independent=independentRead(output);
      const raw=rawNames(output);
      entry.native_layers=layers;
      entry.independent_layers=independent.layers.map(x=>({name:x.name,kind:x.kind}));
      entry.raw_names=raw;
      entry.native_name_after_reopen_correct=layers.some(x=>x.name===c.name&&x.kind==="layer");
      entry.independent_unicode_name_correct=independent.layers.some(x=>x.name===c.name);
      entry.has_dual_name_difference=raw.some(x=>x.unicode===c.name&&
        !x.same_exact_encoding);
      entry.duplicate_names_visible=
        layers.filter(x=>x.name===c.name).length>1;
      entry.output_sha256=sha(fs.readFileSync(output));
      entry.status=entry.native_name_after_reopen_correct&&
        entry.independent_unicode_name_correct?
        "native_name_observation_passed":"native_name_mismatch";
    }catch(e){
      entry.status="native_case_failed_closed";
      entry.error=String(e?.message||e).slice(0,400);
      entry.scratch_output_exists=fs.existsSync(output);
    }
    assert.equal(sha(fs.readFileSync(input)),ORIGINAL_SHA);
    assert.equal(sha(fs.readFileSync(SRC)),ORIGINAL_SHA);
    entry.original_and_research_input_unchanged=true;
    results.push(entry);
  }
  return {
    schema_version:"1",milestone:"M4c-8",
    type:"Photoshop-native multilingual and duplicate layer-name compatibility research",
    public_write_authorized:false,
    summary:{scenarios:results.length,
      saved_and_reopened:results.filter(x=>x.native_save_completed&&
        x.native_name_after_reopen_correct).length,
      legacy_unicode_divergent:results.filter(x=>x.has_dual_name_difference).length,
      production_write_authorizations:0},
    cases:results
  };
}
function check(r){
  assert.equal(r.public_write_authorized,false);
  assert.equal(r.summary.production_write_authorizations,0);
  assert.equal(r.summary.scenarios,3);
  assert.ok(r.cases.every(x=>!x.public_write_authorized&&
    x.original_and_research_input_unchanged));
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
    process.stderr.write("usage: node m4c8_multilingual_native_probe.cjs --write|--check\n");
    process.exitCode=2;
  }
}
module.exports={nativeRename,rawNames,build,check};
