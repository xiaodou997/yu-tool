"use strict";

// M4c-7b/c: research-only native Photoshop save-as and reopen acceptance.
// Source and patch are disposable copies in target/. Output paths are distinct,
// absent and inside the same new private run directory. No YuTool PSD write.

const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const { runCase } = require("./m4c5_byte_patch_spike.cjs");
const { readPhotoshopLayers } = require("./m4c6_photoshop_readonly.cjs");
const { independentRead } = require("./m4c6_dual_name_fidelity.cjs");
const { scan } = require("./m4c3_raw_block_inventory.cjs");
const { buildReport: buildScope } = require("./m4c7_scope_freeze.cjs");
const { classifyPhotoshopReadiness } = require("./m4c7c_native_readiness.cjs");
const { compareSavedPair } = require("./m4c7c_saved_pair_gate.cjs");

const ROOT = path.resolve(__dirname, "../../../../");
const M4C2 = require(path.join(ROOT, "docs/data/psd-safe-mutation-scope-m4c2-v1.json"));
const OUTPUT_ROOT = path.join(ROOT,"target/m4c7-psd-photoshop-save");
const REPORT = path.join(ROOT,"docs/data/m4c7c-photoshop-control-v2.json");
const AG_PATH = path.join(ROOT,"packaging/ag-psd-engine/node_modules/ag-psd");
const sha = data=>crypto.createHash("sha256").update(data).digest("hex");

function requireAbsent(file) {
  try {
    fs.lstatSync(file);
    throw Error("destination already exists (including symlink)");
  }catch(error) {
    if(error.code!=="ENOENT")throw error;
  }
}
function saveAsIndependentCopy(input,output) {
  assert.ok(fs.lstatSync(input).isFile(),"Photoshop input must be a regular disposable file");
  requireAbsent(output);
  assert.notEqual(path.resolve(input),path.resolve(output));
  assert.equal(path.dirname(path.resolve(input)),path.dirname(path.resolve(output)));
  assert.ok(path.resolve(input).startsWith(path.resolve(OUTPUT_ROOT) + path.sep));
  assert.ok(path.resolve(output).startsWith(path.resolve(OUTPUT_ROOT) + path.sep));
  // This private run directory is an isolation boundary. Prechecking that
  // output is absent is NOT an atomic no-clobber guarantee against a process
  // with filesystem access; this is never allowed as a production publisher.
  const native = classifyPhotoshopReadiness();
  if (!native.ready) throw Error("Photoshop native save preflight blocked: "+native.status);
  const script = [
    "(function(){",
    "var d=null, prior=app.displayDialogs;",
    "try{",
    "app.displayDialogs=DialogModes.NO;",
    "d=app.open(new File("+JSON.stringify(input)+"));",
    "var opts=new PhotoshopSaveOptions();",
    "opts.layers=true;opts.embedColorProfile=true;",
    "d.saveAs(new File("+JSON.stringify(output)+"),opts,true,Extension.LOWERCASE);",
    "return 'saved';",
    "}finally{",
    "if(d)d.close(SaveOptions.DONOTSAVECHANGES);",
    "app.displayDialogs=prior;",
    "}",
    "})()",
  ].join("");
  const osa='tell application id "com.adobe.Photoshop" to do javascript '+
    JSON.stringify(script);
  const p=spawnSync("/usr/bin/osascript",["-e",osa],{
    cwd:ROOT,encoding:"utf8",timeout:90000,maxBuffer:1024*1024,
  });
  if(p.error||p.status!==0||p.stdout.trim()!=="saved"){
    throw Error("Photoshop save-as failed: "+(p.error?.message||p.stderr||p.stdout).slice(0,400));
  }
  const file=fs.lstatSync(output);
  if(!file.isFile()||file.size>8*1024*1024)throw Error("unexpected output file");
  return {sha256:sha(fs.readFileSync(output)),bytes:file.size};
}

function layerEqualityExceptName(before,after,expectedName) {
  if(before.layers.length!==after.layers.length)return false;
  return before.layers.every((layer,i)=>{
    const second=after.layers[i];
    return layer.id===second.id && layer.kind===second.kind &&
      (layer.name===second.name ||
        (layer.id==="L0002"&&second.name===expectedName));
  });
}
function keysDiff(before,after,selector,hash){
  const index=items=>{
    const seen=new Map(),out=new Map();
    for(const item of items){
      const id=selector(item),n=seen.get(id)||0;
      seen.set(id,n+1);out.set(id+"#"+n,hash(item));
    }
    return out;
  };
  const a=index(before),b=index(after);
  const added=[],removed=[],changed=[];
  for(const [k,v] of a) {
    if(!b.has(k))removed.push(k);
    else if(b.get(k)!==v)changed.push(k);
  }
  for(const k of b.keys())if(!a.has(k))added.push(k);
  return {added,removed,changed};
}
function compareRaw(source,after) {
  const before=scan(fs.readFileSync(source)),next=scan(fs.readFileSync(after));
  return {
    resources:keysDiff(before.image_resources,next.image_resources,
      x=>String(x.id)+":"+x.name,x=>x.data_sha256),
    tagged:keysDiff(before.tagged_blocks,next.tagged_blocks,
      x=>x.scope+":"+(x.layer_index??"-")+":"+x.key,x=>x.data_sha256),
    composite_byte_equal:before.merged_image.data_sha256===next.merged_image.data_sha256,
    source_bytes:before.source.bytes,
    output_bytes:next.source.bytes,
  };
}

function buildEvidence() {
  if(process.platform!=="darwin"||process.versions.node!=="22.23.3")
    throw Error("requires macOS and pinned Node 22.23.3");
  const ag=require(AG_PATH);
  assert.equal(require(path.join(AG_PATH,"package.json")).version,"31.0.2");
  const scope=buildScope();
  assert.equal(scope.summary.research_save_reopen_candidates,1);
  const admitted=scope.cases.filter(x=>x.gate==="research_save_reopen_candidate");
  const nativePreflight = classifyPhotoshopReadiness();
  if(!nativePreflight.ready) {
    const status = nativePreflight.status==="apple_events_unresponsive" ?
      "blocked_native_application_unresponsive" : "blocked_native_app_preflight";
    return JSON.stringify({
      schema_version:"1",milestone:"M4c-7b/c",
      operation:"real_Photoshop_saveAs_close_reopen_isolated_copies_with_unchanged_control",
      application:"Adobe Photoshop 2026 (macOS)",
      public_write_authorized:false,
      status,
      native_preflight:nativePreflight,
      summary:{
        admitted_cases:admitted.length,
        paired_research_save_reopen_passed:0,
        blocked_or_mismatch:admitted.length,
        product_write_authorizations:0,
      },
      cases:admitted.map(a=>({
        fixture:a.fixture,
        status,
        reason:"Photoshop native preflight declined: "+nativePreflight.status,
        control_saved:false,candidate_saved:false,
        save_command_attempted:false,
        public_write_authorized:false,
      })),
    },null,2)+"\n";
  }
  fs.mkdirSync(OUTPUT_ROOT,{recursive:true});
  const run=fs.mkdtempSync(path.join(OUTPUT_ROOT,"run-"));
  const cases=[];
  for(const admittedCase of admitted) {
    const fixture=M4C2.known_fixtures.find(f=>f.id===admittedCase.fixture);
    const realInput=path.join(ROOT,fixture.path);
    const sourceBytes=fs.readFileSync(realInput);
    assert.equal(sha(sourceBytes),fixture.sha256);
    const control=path.join(run,fixture.id+"-control."+fixture.format);
    fs.writeFileSync(control,sourceBytes,{flag:"wx"});
    const patched=runCase({fixture:fixture.id,new_name:admittedCase.after,
      raw_index:admittedCase.raw_index},run,ag);
    const candidate=path.join(run,fixture.id+"."+fixture.format);
    const savedControl=path.join(run,fixture.id+"-control-saved."+fixture.format);
    const savedCandidate=path.join(run,fixture.id+"-candidate-saved."+fixture.format);
    const output={
      fixture:fixture.id,source_sha256:fixture.sha256,
      candidate_sha256:patched.candidate_sha256,
      status:"not_started",control_saved:false,candidate_saved:false,
      save_command_attempted:false,
      public_write_authorized:false,
    };
    try {
      const baseLayers=readPhotoshopLayers(control);
      const editedLayers=readPhotoshopLayers(candidate);
      if(!baseLayers.some(x=>x.name===admittedCase.before&&x.kind==="group")||
         !editedLayers.some(x=>x.name===admittedCase.after&&x.kind==="group"))
        throw Error("native Photoshop selected group name failed");
      const before=independentRead(control),edited=independentRead(candidate);
      output.save_command_attempted=true;
      const controlSave=saveAsIndependentCopy(control,savedControl);
      output.control_saved=true;
      const candidateSave=saveAsIndependentCopy(candidate,savedCandidate);
      output.candidate_saved=true;
      const reopenedControl=readPhotoshopLayers(savedControl);
      const reopenedCandidate=readPhotoshopLayers(savedCandidate);
      const afterControl=independentRead(savedControl);
      const afterCandidate=independentRead(savedCandidate);
      output.editor_control_name_preserved=
        reopenedControl.some(x=>x.kind==="group"&&x.name===admittedCase.before);
      output.editor_candidate_name_preserved=
        reopenedCandidate.some(x=>x.kind==="group"&&x.name===admittedCase.after);
      output.control_semantic_tree_preserved=layerEqualityExceptName(before,afterControl,admittedCase.before);
      output.candidate_semantic_tree_preserved=layerEqualityExceptName(edited,afterCandidate,admittedCase.after);
      output.independent_preview_control_preserved=before.independent_preview.status==="rendered" &&
        before.independent_preview.rgba_sha256===afterControl.independent_preview.rgba_sha256;
      output.independent_preview_candidate_preserved=edited.independent_preview.status==="rendered" &&
        edited.independent_preview.rgba_sha256===afterCandidate.independent_preview.rgba_sha256;
      output.independent_preview_saved_control_vs_saved_candidate_equal=
        afterControl.independent_preview.status==="rendered" &&
        afterControl.independent_preview.rgba_sha256===afterCandidate.independent_preview.rgba_sha256;
      output.photoshop_native_control_names=reopenedControl;
      output.photoshop_native_candidate_names=reopenedCandidate;
      output.output_control=controlSave;
      output.output_candidate=candidateSave;
      output.control_raw_differences=compareRaw(control,savedControl);
      output.candidate_raw_differences=compareRaw(candidate,savedCandidate);
      output.saved_pair_strict_raw_evidence=compareSavedPair(
        fs.readFileSync(savedControl),fs.readFileSync(savedCandidate),{
          raw_index:admittedCase.raw_index,
          expected_old_name:admittedCase.before,
          expected_new_name:admittedCase.after,
        });
      output.save_roundtrip_research_passed=output.editor_control_name_preserved &&
        output.editor_candidate_name_preserved &&
        output.control_semantic_tree_preserved && output.candidate_semantic_tree_preserved &&
        output.independent_preview_control_preserved &&
        output.independent_preview_candidate_preserved &&
        output.independent_preview_saved_control_vs_saved_candidate_equal &&
        output.saved_pair_strict_raw_evidence.strict_saved_pair_bytes_passed;
      output.status=output.save_roundtrip_research_passed?"paired_control_verified":"paired_control_mismatch";
    } catch(error) {
      output.status="blocked_or_failed";
      output.error=String(error?.message||error).slice(0,500);
      output.saved_control_output_exists=fs.existsSync(savedControl);
      output.saved_candidate_output_exists=fs.existsSync(savedCandidate);
      // If a native save timed out, the application might still have emitted
      // an output. Do not call the operation successfully completed.
    }
    // Must never change any input, independent of application-save outcome.
    assert.equal(sha(fs.readFileSync(realInput)),fixture.sha256);
    assert.equal(sha(fs.readFileSync(control)),fixture.sha256);
    assert.equal(sha(fs.readFileSync(candidate)),patched.candidate_sha256);
    output.source_and_experimental_candidate_unchanged=true;
    cases.push(output);
  }
  return JSON.stringify({
    schema_version:"1",milestone:"M4c-7b/c",
    operation:"real_Photoshop_saveAs_close_reopen_isolated_copies_with_unchanged_control",
    application:"Adobe Photoshop 2026 (macOS)",
    public_write_authorized:false,
    summary:{
      admitted_cases:cases.length,
      paired_research_save_reopen_passed:cases.filter(c=>c.save_roundtrip_research_passed).length,
      blocked_or_mismatch:cases.filter(c=>!c.save_roundtrip_research_passed).length,
      product_write_authorizations:0,
    },
    cases,
  },null,2)+"\n";
}

if(require.main===module) {
  const mode=process.argv[2];
  if(!["--write","--check"].includes(mode)||process.argv.length!==3){
    process.stderr.write("usage: node m4c7_photoshop_save_probe.cjs --write|--check\n");
    process.exitCode=2;
  } else {
    const report=buildEvidence();
    if(mode==="--write")fs.writeFileSync(REPORT,report);
    else assert.equal(fs.readFileSync(REPORT,"utf8"),report);
    console.log(JSON.stringify({mode,...JSON.parse(report).summary}));
  }
}
module.exports={saveAsIndependentCopy,buildEvidence,compareRaw};
