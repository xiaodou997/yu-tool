"use strict";

// M4c-7e: broaden independent Photoshop *read-only* observations only.
// New fixtures are not mutation inputs, writer allow-list entries, or
// permission to save. Photoshop opens disposable source copies and closes
// without saving. No Photoshop saveAs is performed by this script.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const assert=require("node:assert/strict");
const {classifyPhotoshopReadiness}=
  require("./m4c7c_native_readiness.cjs");
const {readPhotoshopLayers}=
  require("./m4c6_photoshop_readonly.cjs");
const {independentRead}=
  require("./m4c6_dual_name_fidelity.cjs");
const PROFILE=require(path.resolve(__dirname,
  "../../../../docs/data/psd-raw-block-inventory-m4c3-v1.json"));
const ROOT=path.resolve(__dirname,"../../../../");
const TEMP=path.join(ROOT,"target/m4c7e-native-readonly");
const REPORT=path.join(ROOT,"docs/data/m4c7e-native-readonly-samples-v1.json");
const FROZEN=[
  "bench-baseline-rgb", // pixel class, no PSD mutation permission
  "simple-pixel-layers-psd", // non-ASCII Unicode / native Background
  "text-layer", // unsupported Type layer
];
const sha=b=>crypto.createHash("sha256").update(b).digest("hex");

function reportOne(fixture,dir) {
  const item=PROFILE.cases.find(x=>x.fixture===fixture);
  if(!item||item.outcome!=="structurally_parsed")throw Error("unknown frozen source");
  const input=path.join(ROOT,item.path),bytes=fs.readFileSync(input);
  assert.equal(sha(bytes),item.sha256);
  const copy=path.join(dir,fixture+"."+item.format);
  fs.writeFileSync(copy,bytes,{flag:"wx"});
  let result={fixture,source_sha256:item.sha256,
    public_write_authorized:false,read_only:true,photoshop_save_attempted:false};
  try {
    const photoshop=readPhotoshopLayers(copy);
    const reference=independentRead(copy);
    result={
      ...result,status:"photoshop_read_only_inspected",
      native_photoshop_layers:photoshop,
      psd_tools_logical_layers:reference.layers.map(x=>({
        name:x.name,kind:x.kind
      })),
      native_layer_count:photoshop.length,
      reference_layer_count:reference.layers.length,
      native_background_count:photoshop.filter(x=>x.kind==="background").length,
      native_and_reference_layer_count_equal:photoshop.length===reference.layers.length
    };
  }catch(err){
    result.status="native_read_refused_or_failed";
    result.error=String(err?.message||err).slice(0,300);
  }
  assert.equal(sha(fs.readFileSync(copy)),item.sha256);
  assert.equal(sha(fs.readFileSync(input)),item.sha256);
  result.source_and_scratch_unchanged=true;
  return result;
}
function buildReport(){
  const readiness=classifyPhotoshopReadiness();
  if(!readiness.ready) {
    return {schema_version:"1",milestone:"M4c-7e",
      status:"blocked_native_read_only_preflight",native_preflight:readiness,
      public_write_authorized:false,
      summary:{requested_readonly_fixtures:FROZEN.length,inspected:0,
        production_writes_authorized:0},cases:[]};
  }
  fs.mkdirSync(TEMP,{recursive:true});
  const dir=fs.mkdtempSync(path.join(TEMP,"run-"));
  const cases=FROZEN.map(id=>reportOne(id,dir));
  return {
    schema_version:"1",milestone:"M4c-7e",
    operation:"Photoshop read-only scratch fixture inspection",
    public_write_authorized:false,native_preflight:readiness,
    summary:{requested_readonly_fixtures:cases.length,
      inspected:cases.filter(x=>x.status==="photoshop_read_only_inspected").length,
      source_copies_unchanged:cases.filter(x=>x.source_and_scratch_unchanged).length,
      production_writes_authorized:0},
    cases
  };
}
function check(report){
  assert.equal(report.milestone,"M4c-7e");
  assert.equal(report.public_write_authorized,false);
  assert.equal(report.summary.production_writes_authorized,0);
  assert.ok(report.cases.every(x=>x.public_write_authorized===false &&
    x.photoshop_save_attempted===false && x.source_and_scratch_unchanged));
}
if(require.main===module){
  const mode=process.argv[2];
  if(mode==="--write"&&process.argv.length===3){
    const r=buildReport();check(r);
    fs.writeFileSync(REPORT,JSON.stringify(r,null,2)+"\n");
    console.log(JSON.stringify({mode,...r.summary}));
  }else if(mode==="--check"&&process.argv.length===3){
    const r=JSON.parse(fs.readFileSync(REPORT,"utf8"));
    check(r);
    console.log(JSON.stringify({mode,offline:true,...r.summary}));
  }else{
    process.stderr.write("usage: node m4c7e_native_readonly_sampling.cjs --write|--check\n");
    process.exitCode=2;
  }
}
module.exports={buildReport,reportOne,check};
