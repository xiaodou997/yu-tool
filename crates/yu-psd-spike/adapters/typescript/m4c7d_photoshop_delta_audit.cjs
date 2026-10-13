"use strict";

// M4c-7d: read-only audit of two independent isolated Photoshop saveAs runs.
// No Photoshop invocation, PSD writer, user output or product authorization.
// --write collects a new research evidence snapshot from ignored target/;
// --check validates the committed snapshot OFFLINE and never saves anything.
const fs=require("node:fs");
const path=require("node:path");
const crypto=require("node:crypto");
const assert=require("node:assert/strict");
const {spawnSync}=require("node:child_process");
const {scan}=require("./m4c3_raw_block_inventory.cjs");
const {compareSavedPair,diff}=require("./m4c7c_saved_pair_gate.cjs");

const ROOT=path.resolve(__dirname,"../../../../");
const EXPERIMENT=path.join(ROOT,"target/m4c7-psd-photoshop-save");
const EVIDENCE=path.join(ROOT,"docs/data/m4c7d-photoshop-metadata-preview-v1.json");
const PYTHON=process.env.M4C7D_PYTHON ||
  path.join(ROOT,"target/m4c7d-py312/bin/python");
const PY_SCRIPT=path.join(ROOT,"crates/yu-psd-spike/adapters/m4c7d_preview_diff.py");
const NATIVE_PY_SCRIPT=path.join(ROOT,"crates/yu-psd-spike/adapters/m4c7d_native_render_diff.py");
const POLICY=require(path.join(ROOT,"docs/data/psd-safe-mutation-scope-m4c2-v1.json"));
const PATCH_REPORT=require(path.join(ROOT,"docs/data/m4c5-psd-byte-patch-evidence-v1.json"));
const FIXTURE=POLICY.known_fixtures.find(f=>f.id==="nested-group");
const PATCH=PATCH_REPORT.cases.find(c=>c.fixture==="nested-group");
const NAMES={
  source:"nested-group-control.psd",
  patched_input:"nested-group.psd",
  control_saved:"nested-group-control-saved.psd",
  candidate_saved:"nested-group-candidate-saved.psd",
};
const sha=buffer=>crypto.createHash("sha256").update(buffer).digest("hex");

function loadChecked(dir) {
  const absolute=path.resolve(dir);
  if(!absolute.startsWith(EXPERIMENT+path.sep) ||
      path.dirname(absolute)!==EXPERIMENT ||
      !path.basename(absolute).startsWith("run-"))throw Error("not an isolated research run");
  const out={};
  for(const [key,name] of Object.entries(NAMES)){
    const file=path.join(absolute,name);
    const stat=fs.lstatSync(file);
    if(!stat.isFile()||stat.isSymbolicLink()||stat.size>8*1024*1024)
      throw Error("invalid Photoshop research file "+key);
    out[key]=fs.readFileSync(file);
  }
  assert.equal(sha(out.source),FIXTURE.sha256);
  assert.equal(sha(out.patched_input),PATCH.candidate_sha256);
  return out;
}
function rawDelta(a,b) {
  const x=scan(a),y=scan(b);
  const keyed=(list,key,hash)=>diff(list(x),list(y),key,hash);
  const channels=i=>i.layer_records.flatMap(r=>r.channels.map((c,n)=>({
    key:r.index+":"+n+":"+c.id,sha:c.data_sha256
  })));
  return {
    source_bytes:a.length,output_bytes:b.length,
    image_resources:keyed(i=>i.image_resources,
      r=>String(r.id)+":"+r.name,r=>r.data_sha256),
    additional_layer_information:keyed(i=>i.tagged_blocks,
      t=>t.scope+":"+(t.layer_index??"-")+":"+t.key,t=>t.data_sha256),
    channel_pixels:keyed(channels,c=>c.key,c=>c.sha),
    compressed_composite_equal:x.merged_image.data_sha256===y.merged_image.data_sha256,
    unknown_blocks_before:x.summary.unknown_block_count,
    unknown_blocks_after:y.summary.unknown_block_count,
    post_save_padding_detected:y.risks.includes("tagged_block_trailing_zero_padding"),
  };
}
function runPython(script,dir) {
  const process=spawnSync(PYTHON,[script,dir],{
    cwd:ROOT,encoding:"utf8",maxBuffer:1024*1024,timeout:45000
  });
  if(process.error||process.status!==0)
    throw Error("psd-tools preview audit failed: "+
      (process.error?.message||process.stderr||process.stdout).slice(0,250));
  const value=JSON.parse(process.stdout);
  if(value.status==="error")throw Error("preview audit rejected: "+
    (value.message||value.reason));
  return value;
}
function basicRun(dir) {
  const files=loadChecked(dir);
  const preview=runPython(PY_SCRIPT,dir);
  const native=runPython(NATIVE_PY_SCRIPT,dir);
  const originalControl=rawDelta(files.source,files.control_saved);
  const originalCandidate=rawDelta(files.patched_input,files.candidate_saved);
  const savedPair=rawDelta(files.control_saved,files.candidate_saved);
  const pair=compareSavedPair(files.control_saved,files.candidate_saved,{
    raw_index:3,expected_old_name:"Group 1",expected_new_name:"Group 2",
  });
  assert.equal(pair.public_write_authorized,false);
  return {
    files_sha256:Object.fromEntries(Object.entries(files).map(([k,v])=>[k,sha(v)])),
    preview,native_rendering:native,
    source_to_unchanged_control_save:originalControl,
    patched_input_to_candidate_save:originalCandidate,
    saved_original_control_vs_saved_candidate:savedPair,
    native_saved_pair_strict_check:pair,
  };
}
function normalizeDiff(delta) {
  // All item lists are already sorted by their serialized occurrence; sort
  // for stable evidence independent of iteration order in future parsers.
  return {
    added:delta.added.slice().sort(),
    removed:delta.removed.slice().sort(),
    changed:delta.changed.slice().sort(),
  };
}
function capture(dirs) {
  if(!Array.isArray(dirs)||dirs.length!==2||dirs[0]===dirs[1])
    throw Error("exactly two distinct research save directories required");
  if(!FIXTURE||!PATCH||PATCH.candidate_sha256===undefined)
    throw Error("research source/patch freeze missing");
  const a=basicRun(dirs[0]),b=basicRun(dirs[1]);
  const sourceComparison=a.preview.source_vs_control_saved;
  const editComparison=a.preview.patched_input_vs_candidate_saved;
  const sameSaved=a.preview.control_saved_vs_candidate_saved;
  const metrics=r=>r.preview.source_vs_control_saved;
  const rawComparison={
    unchanged_control_across_runs:rawDelta(
      loadChecked(dirs[0]).control_saved,loadChecked(dirs[1]).control_saved),
    patched_candidate_across_runs:rawDelta(
      loadChecked(dirs[0]).candidate_saved,loadChecked(dirs[1]).candidate_saved),
  };
  const repeatedPreviewPattern=JSON.stringify(metrics(a))===JSON.stringify(metrics(b)) &&
    JSON.stringify(a.preview.patched_input_vs_candidate_saved)===
      JSON.stringify(b.preview.patched_input_vs_candidate_saved);
  const repeatedOutputEquality=
    a.preview.control_saved_vs_candidate_saved.changed_pixels===0 &&
    b.preview.control_saved_vs_candidate_saved.changed_pixels===0;
  const nativeComparisons=[a,b].flatMap(x=>[
    x.native_rendering.native_source_vs_native_saved_control,
    x.native_rendering.native_patch_vs_native_saved_candidate,
    x.native_rendering.native_saved_control_vs_native_saved_candidate,
  ]);
  const nativeCompositesIdentical=nativeComparisons.every(x=>x.changed_pixels===0);
  const independentSourceDivergence=[a,b].every(x=>
    x.native_rendering.independent_parser_vs_native_for_each_file.source.changed_pixels===160 &&
    x.native_rendering.independent_parser_vs_native_for_each_file.patched_input.changed_pixels===160 &&
    x.native_rendering.independent_parser_vs_native_for_each_file.control_saved.changed_pixels===0 &&
    x.native_rendering.independent_parser_vs_native_for_each_file.candidate_saved.changed_pixels===0);
  const nameBytesOnlyPassed=[a,b].every(r=>
    r.native_saved_pair_strict_check.strict_saved_pair_bytes_passed);
  const output={
    schema_version:"1",milestone:"M4c-7d",
    status:"repeated_native_save_analysis_complete_fidelity_blocked",
    research_only:true,public_write_authorized:false,
    fixture:"nested-group",known_fixture_sha256:FIXTURE.sha256,
    engine_versions:{photoshop:"2026",psd_tools:"1.20.0",python:"3.12",node:"22.23.3"},
    explanation:"psd-tools preview is not a native Photoshop render oracle; raw SHA changes do not alone prove damage",
    summary:{
      repeated_native_save_runs:2,
      source_preview_changed_pixels:sourceComparison.changed_pixels,
      edited_input_preview_changed_pixels:editComparison.changed_pixels,
      source_vs_save_preview_same_pattern_across_runs:repeatedPreviewPattern,
      saved_control_vs_candidate_pixel_equal_in_each_run:repeatedOutputEquality,
      photoshop_native_composites_equal_across_original_and_saved:nativeCompositesIdentical,
      independent_source_composite_differs_from_native_only_before_save:
        independentSourceDivergence,
      exact_saved_pair_bytes_only_name_changed:nameBytesOnlyPassed,
      production_writes_authorized:0,
    },
    runs:[a,b],
    between_runs:rawComparison,
    classification:{
      editor_save_reproduces_source_preview_difference_on_unchanged_control:true,
      editor_saved_outputs_are_pixel_equivalent_between_control_and_rename:repeatedOutputEquality,
      photoshop_native_before_after_save_composites_unchanged:nativeCompositesIdentical,
      independent_parser_on_original_differs_from_native_render:independentSourceDivergence,
      saved_metadata_changes_across_identical_runs:
        rawComparison.unchanged_control_across_runs.image_resources.changed.length>0 ||
        rawComparison.unchanged_control_across_runs.additional_layer_information.changed.length>0,
      unresolved_serializer_or_name_metadata_variations:
        !nameBytesOnlyPassed,
      photoshop_visual_equivalence_independently_certified:false,
      comprehensive_metadata_fidelity_certified:false,
    }
  };
  assert.equal(output.summary.source_preview_changed_pixels,160);
  assert.equal(output.summary.edited_input_preview_changed_pixels,160);
  assert.ok(output.summary.source_vs_save_preview_same_pattern_across_runs);
  assert.ok(output.summary.saved_control_vs_candidate_pixel_equal_in_each_run);
  assert.ok(output.summary.photoshop_native_composites_equal_across_original_and_saved);
  assert.ok(output.summary.independent_source_composite_differs_from_native_only_before_save);
  assert.equal(output.summary.exact_saved_pair_bytes_only_name_changed,false);
  return output;
}
function checkSnapshot(obj) {
  assert.equal(obj.schema_version,"1");
  assert.equal(obj.milestone,"M4c-7d");
  assert.equal(obj.public_write_authorized,false);
  assert.equal(obj.summary.production_writes_authorized,0);
  assert.equal(obj.summary.repeated_native_save_runs,2);
  assert.equal(obj.runs.length,2);
  assert.equal(obj.summary.exact_saved_pair_bytes_only_name_changed,false);
  assert.ok(obj.runs.every(r=>
    r.native_saved_pair_strict_check.public_write_authorized===false &&
    r.native_saved_pair_strict_check.strict_saved_pair_bytes_passed===false));
}
if(require.main===module) {
  const mode=process.argv[2];
  if(mode==="--write") {
    if(process.versions.node!=="22.23.3")throw Error("pinned Node 22.23.3 required");
    const dirs=process.argv.slice(3).map(x=>path.resolve(x));
    const obj=capture(dirs);
    checkSnapshot(obj);
    fs.writeFileSync(EVIDENCE,JSON.stringify(obj,null,2)+"\n");
    console.log(JSON.stringify({mode,...obj.summary}));
  } else if(mode==="--check"&&process.argv.length===3) {
    const obj=JSON.parse(fs.readFileSync(EVIDENCE,"utf8"));
    checkSnapshot(obj);
    console.log(JSON.stringify({mode,offline:true,...obj.summary}));
  } else {
    process.stderr.write("usage: node m4c7d_photoshop_delta_audit.cjs --write RUN_DIR_1 RUN_DIR_2 | --check\n");
    process.exitCode=2;
  }
}
module.exports={capture,checkSnapshot,rawDelta,normalizeDiff};
