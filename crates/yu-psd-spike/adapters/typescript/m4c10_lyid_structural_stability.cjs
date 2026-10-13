"use strict";

// M4c-10: Photoshop-native reorder/group layer-id persistence research.
// The only input is a SHA-pinned, pre-existing Photoshop-generated duplicate
// name PSD, copied into ignored target/. No mutation of user PSD or public CLI.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const assert=require("node:assert/strict");
const {spawnSync}=require("node:child_process");
const {scan}=require("./m4c3_raw_block_inventory.cjs");
const {classifyPhotoshopReadiness}=require("./m4c7c_native_readiness.cjs");
const PRIOR=require(path.resolve(__dirname,
  "../../../../docs/data/m4c8-multilingual-photoshop-names-v1.json"));
const ROOT=path.resolve(__dirname,"../../../../");
const SOURCE=process.env.M4C10_M4C8_DUPLICATE_SOURCE ||
  "/Users/luoxiaodou/workspace/yu-workspace/.webcodex-worktrees/yu-tool-pr22-72bbe9b7/target/m4c8-multilingual-native/run-whZZDB/duplicate-native-saved.psd";
const TEMP=path.join(ROOT,"target/m4c10-lyid-structure");
const REPORT=path.join(ROOT,"docs/data/m4c10-lyid-reorder-group-v1.json");
const sha=b=>crypto.createHash("sha256").update(b).digest("hex");
function fail(reason){const e=new Error(reason);e.code="M4C10_RESEARCH_REFUSED";throw e;}
function extractRawIds(bytes){
  const inventory=scan(bytes),out=[];
  for(const r of inventory.layer_records){
    const tags=inventory.tagged_blocks.filter(t=>
      t.scope==="layer"&&t.layer_index===r.index&&t.key==="lyid");
    if(tags.length>1)fail("duplicate raw layer ID records");
    if(tags.length===1){
      if(tags[0].length!==4)fail("unexpected lyid width");
      out.push({raw_index:r.index,
        photoshop_lyid:bytes.readUInt32BE(tags[0].offset+8+tags[0].length_width),
        legacy_name:r.legacy_name});
    }
  }
  if(new Set(out.map(x=>x.photoshop_lyid)).size!==out.length)
    fail("repeated lyid values in raw PSD");
  return out;
}
function photoshopScript(input,output,action){
  if(!["reorder","group"].includes(action))fail("unsupported structure study");
  const js=[
    "(function(){var d=null,old=app.displayDialogs;try{",
    "app.displayDialogs=DialogModes.NO;",
    "d=app.open(new File("+JSON.stringify(input)+"));",
    "function find(layers,id){for(var i=0;i<layers.length;i++){",
    "var x=layers[i];if(x.id===id)return x;",
    "if(x.typename==='LayerSet'){var a=find(x.layers,id);if(a)return a;}",
    "}return null;}",
    "var selected=find(d.layers,6);var other=find(d.layers,2);",
    "if(!selected||!other||selected.name!=='Layer 0'||other.name!=='Layer 0')",
    "throw new Error('pinned duplicate target mismatch');",
    ...(action==="reorder"?
      ["selected.move(other,ElementPlacement.PLACEBEFORE);"]:
      ["var group=d.layerSets.add();group.name='M4c10 Research Group';",
       "selected.move(group,ElementPlacement.INSIDE);"]),
    "if(!find(d.layers,6)||!find(d.layers,2))",
    "throw new Error('lyid missing after transform');",
    "var opt=new PhotoshopSaveOptions();opt.layers=true;opt.embedColorProfile=true;",
    "d.saveAs(new File("+JSON.stringify(output)+"),opt,true,Extension.LOWERCASE);",
    "return 'saved';",
    "}finally{if(d)d.close(SaveOptions.DONOTSAVECHANGES);app.displayDialogs=old;}})()"
  ].join("");
  const osa='tell application id "com.adobe.Photoshop" to do javascript '+JSON.stringify(js);
  const r=spawnSync("/usr/bin/osascript",["-e",osa],{
    cwd:ROOT,encoding:"utf8",timeout:90000,maxBuffer:1024*1024
  });
  if(r.error||r.status!==0||r.stdout.trim()!=="saved")
    fail("Photoshop native operation failed: "+
      String(r.error?.message||r.stderr||r.stdout).slice(0,300));
}
function nativeRead(file){
  const script=[
    "(function(){var d=null,prev=app.displayDialogs;try{",
    "app.displayDialogs=DialogModes.NO;",
    "d=app.open(new File("+JSON.stringify(file)+"));",
    "var items=[];function visit(layers,depth){for(var i=0;i<layers.length;i++){",
    "var x=layers[i];items.push(String(x.id)+':'+String(depth)+':'+",
    "(x.typename==='LayerSet'?'G:':'L:')+encodeURIComponent(x.name));",
    "if(x.typename==='LayerSet')visit(x.layers,depth+1);",
    "}}visit(d.layers,0);return items.join('|');",
    "}finally{if(d)d.close(SaveOptions.DONOTSAVECHANGES);app.displayDialogs=prev;}})()"
  ].join("");
  const osa='tell application id "com.adobe.Photoshop" to do javascript '+JSON.stringify(script);
  const r=spawnSync("/usr/bin/osascript",["-e",osa],{
    cwd:ROOT,encoding:"utf8",timeout:60000,maxBuffer:1024*1024
  });
  if(r.error||r.status!==0)fail("Photoshop read failed: "+
    String(r.error?.message||r.stderr||r.stdout).slice(0,280));
  return r.stdout.trim().split("|").filter(Boolean).map(x=>{
    const m=x.match(/^(\d+):(\d+):([GL]):(.*)$/);
    if(!m)fail("unexpected native layer read structure");
    return {photoshop_lyid:Number(m[1]),depth:Number(m[2]),group:m[3]==="G",
      name:decodeURIComponent(m[4])};
  });
}
function build(){
  if(process.platform!=="darwin"||process.versions.node!=="22.23.3")
    fail("pinned Node 22 and macOS required");
  if(path.basename(SOURCE)!=="duplicate-native-saved.psd" ||
      !path.basename(path.dirname(SOURCE)).startsWith("run-") ||
      path.basename(path.dirname(path.dirname(SOURCE)))!=="m4c8-multilingual-native")
    fail("only previous M4c-8 Photoshop scratch fixture may be used");
  const recorded=PRIOR.cases.find(c=>c.scenario==="duplicate");
  const stat=fs.lstatSync(SOURCE);
  if(!stat.isFile()||stat.isSymbolicLink()||stat.size>8*1024*1024)
    fail("invalid SHA-bound native scratch fixture");
  const source=fs.readFileSync(SOURCE);
  if(sha(source)!==recorded.output_sha256)
    fail("unrecognized Photoshop saved source");
  const before=extractRawIds(source);
  if(!before.some(x=>x.photoshop_lyid===6)||!before.some(x=>x.photoshop_lyid===2))
    fail("frozen ID missing");
  const ready=classifyPhotoshopReadiness();
  if(!ready.ready)fail("Photoshop not in empty workspace: "+ready.status);
  fs.mkdirSync(TEMP,{recursive:true});
  const dir=fs.mkdtempSync(path.join(TEMP,"run-"));
  const cases=[];
  for(const action of ["reorder","group"]){
    const input=path.join(dir,action+"-input.psd"),
      output=path.join(dir,action+"-saved.psd");
    if(fs.existsSync(input)||fs.existsSync(output))fail("existing scratch output");
    fs.writeFileSync(input,source,{flag:"wx"});
    photoshopScript(input,output,action);
    const saved=fs.readFileSync(output),raw=extractRawIds(saved);
    const native=nativeRead(output);
    const selected=native.find(x=>x.photoshop_lyid===6),
      other=native.find(x=>x.photoshop_lyid===2);
    const IDsCorrect=!!selected&&!!other &&
      selected.name==="Layer 0"&&other.name==="Layer 0"&&
      raw.some(x=>x.photoshop_lyid===6)&&raw.some(x=>x.photoshop_lyid===2);
    const structureCorrect=action==="reorder"?
      native.findIndex(x=>x.photoshop_lyid===6)<
      native.findIndex(x=>x.photoshop_lyid===2):
      selected.depth>other.depth && native.some(x=>x.group&&
        x.name==="M4c10 Research Group");
    if(!IDsCorrect||!structureCorrect)fail("native IDs or structure changed unexpectedly");
    assert.equal(sha(fs.readFileSync(input)),recorded.output_sha256);
    assert.equal(sha(fs.readFileSync(SOURCE)),recorded.output_sha256);
    cases.push({action,
      source_sha256:recorded.output_sha256,
      output_sha256:sha(saved),
      Photoshop_lyids_6_and_2_preserved:IDsCorrect,
      expected_native_structure_observed:structureCorrect,
      native_layers:native,raw_lyid_records:raw,
      source_and_input_unchanged:true,
      public_write_authorized:false});
  }
  return {schema_version:"1",milestone:"M4c-10",
    operation:"Photoshop native scratch only lyid reorder and group stability",
    public_write_authorized:false,
    summary:{native_structural_operations_tested:cases.length,
      operations_preserving_target_lyids:cases.filter(x=>
        x.Photoshop_lyids_6_and_2_preserved&&
        x.expected_native_structure_observed).length,
      production_writes_authorized:0},
    cases};
}
function check(r){
  assert.equal(r.public_write_authorized,false);
  assert.equal(r.summary.native_structural_operations_tested,2);
  assert.equal(r.summary.production_writes_authorized,0);
  assert.ok(r.cases.every(x=>x.source_and_input_unchanged&&
    x.public_write_authorized===false));
}
if(require.main===module){
  if(process.argv[2]==="--write"&&process.argv.length===3){
    const data=build();check(data);
    fs.writeFileSync(REPORT,JSON.stringify(data,null,2)+"\n");
    console.log(JSON.stringify({mode:"--write",...data.summary}));
  }else if(process.argv[2]==="--check"&&process.argv.length===3){
    const data=JSON.parse(fs.readFileSync(REPORT,"utf8"));check(data);
    console.log(JSON.stringify({mode:"--check",offline:true,...data.summary}));
  }else{process.stderr.write("usage: node m4c10_lyid_structural_stability.cjs --write|--check\n");process.exitCode=2;}
}
module.exports={extractRawIds,build,check};
