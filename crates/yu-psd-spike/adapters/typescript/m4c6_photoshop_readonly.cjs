"use strict";

// M4c-6: explicitly-invoked real Photoshop application receipt.
// Opens *only isolated fixture copies*, reads layer names, closes without
// saving, checks source/candidate SHA-256 afterward. No GUI screenshots,
// no PSD saves and no unattended production capability.

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const crypto = require("node:crypto");
const { spawnSync } = require("node:child_process");
const assert = require("node:assert/strict");
const { CASES, runCase } = require("./m4c5_byte_patch_spike.cjs");

const ROOT = path.resolve(__dirname, "../../../../");
const POLICY = require(path.join(ROOT,"docs/data/psd-safe-mutation-scope-m4c2-v1.json"));
const AG_ROOT = path.join(ROOT,"packaging/ag-psd-engine/node_modules/ag-psd");
const OUTPUT_ROOT = path.join(ROOT,"target/m4c6-photoshop-readonly");
const EVIDENCE = path.join(ROOT,"docs/data/m4c6-photoshop-name-receipt-v1.json");
const sha = b => crypto.createHash("sha256").update(b).digest("hex");

function readPhotoshopLayers(file) {
  // ExtendScript/Photoshop 2026 does not necessarily define JSON.stringify.
  // Restrict returned payload to URL-encoded names and a simple group bit.
  const script = [
    "(function(){",
    "var doc=null, prior=app.displayDialogs;",
    "try {",
    "app.displayDialogs=DialogModes.NO;",
    "doc=app.open(new File(" + JSON.stringify(file) + "));",
    "var a=[];",
    "function walk(p){for(var i=0;i<p.layers.length;i++){",
    "var l=p.layers[i];var g=l.typename==='LayerSet';",
    "a.push((g?'G:':(l.isBackgroundLayer?'B:':'L:'))+encodeURIComponent(l.name));",
    "if(g)walk(l);",
    "}}",
    "walk(doc);return a.join('|');",
    "}finally{if(doc)doc.close(SaveOptions.DONOTSAVECHANGES);",
    "app.displayDialogs=prior;}",
    "})()",
  ].join("");
  const appleScript = 'tell application id "com.adobe.Photoshop" to do javascript ' +
    JSON.stringify(script);
  const r = spawnSync("/usr/bin/osascript",["-e",appleScript],{
    cwd:ROOT,encoding:"utf8",timeout:90000,maxBuffer:1024*1024,
  });
  if (r.error || r.status !== 0) {
    throw Error("Photoshop read-only script unavailable: " +
      (r.error?.message || r.stderr || r.stdout));
  }
  return r.stdout.trim().split("|").filter(Boolean).map(x=>({
    kind:x.startsWith("G:")?"group":x.startsWith("B:")?"background":"layer",
    name:decodeURIComponent(x.slice(2)),
  }));
}

function sortedNames(records) {
  return records.map(x=>
    (x.kind==="group"?"G:":x.kind==="background"?"B:":"L:")+x.name
  ).sort();
}

function buildReport() {
  if(process.platform!=="darwin")throw Error("Photoshop automation requires macOS");
  if(process.versions.node!=="22.23.3")throw Error("pinned Node 22.23.3 required");
  const ag=require(AG_ROOT);
  assert.equal(require(path.join(AG_ROOT,"package.json")).version,"31.0.2");
  fs.mkdirSync(OUTPUT_ROOT,{recursive:true});
  const folder=fs.mkdtempSync(path.join(OUTPUT_ROOT,"run-"));
  const results=[];
  for(const choice of CASES){
    const f=POLICY.known_fixtures.find(x=>x.id===choice.fixture);
    assert.ok(f);
    const realSource=path.join(ROOT,f.path);
    const sourceBytes=fs.readFileSync(realSource);
    assert.equal(sha(sourceBytes),f.sha256);
    const copy=path.join(folder,choice.fixture+"-source."+f.format);
    fs.writeFileSync(copy,sourceBytes,{flag:"wx"});
    const patch=runCase(choice,folder,ag);
    const candidate=path.join(folder,choice.fixture+"."+f.format);
    let entry={
      fixture:choice.fixture,
      source_sha256:f.sha256,
      candidate_sha256:patch.candidate_sha256,
      expected_old_name:f.research_target.name,
      expected_new_name:choice.new_name,
      expected_kind:f.research_target.kind==="group"?"group":"layer",
      status:"not_run",
      photoshop_layer_name_read_verified:false,
      photoshop_save_or_visual_roundtrip_verified:false,
      production_write_authorized:false,
    };
    try {
      const prior=readPhotoshopLayers(copy);
      const after=readPhotoshopLayers(candidate);
      const expectedPrior=[...prior];
      const index=expectedPrior.findIndex(x=>x.name===entry.expected_old_name&&
        x.kind===entry.expected_kind);
      if(index>=0)expectedPrior[index]={...expectedPrior[index],name:entry.expected_new_name};
      const namesMatch=index>=0 && JSON.stringify(sortedNames(expectedPrior))===
        JSON.stringify(sortedNames(after));
      entry={
        ...entry,status:index<0?"source_name_not_as_expected":
          namesMatch?"read_verified":"name_mismatch",
        photoshop_source_layers:prior,
        photoshop_candidate_layers:after,
        photoshop_layer_name_read_verified:namesMatch,
      };
    }catch(error){
      entry.status="unavailable_or_failed";
      entry.error=String(error?.message||error).slice(0,360);
    }
    assert.equal(sha(fs.readFileSync(copy)),f.sha256);
    assert.equal(sha(fs.readFileSync(candidate)),patch.candidate_sha256);
    assert.equal(sha(fs.readFileSync(realSource)),f.sha256);
    entry.all_input_and_candidate_bytes_unchanged_by_photoshop=true;
    results.push(entry);
  }
  const report={
    schema_version:"1",milestone:"M4c-6",
    application:"Adobe Photoshop 2026 on macOS",
    operation:"open/read_layer_names/close_without_saving",
    visual_or_save_roundtrip_certified:false,
    public_write_authorized:false,
    summary:{
      fixtures:results.length,
      read_verified:results.filter(x=>x.photoshop_layer_name_read_verified).length,
      unavailable_or_mismatch:results.filter(x=>!x.photoshop_layer_name_read_verified).length,
      saved_or_modified_inputs:0,
      production_writes_authorized:0,
    },
    cases:results,
  };
  return JSON.stringify(report,null,2)+"\n";
}

if(require.main===module){
  const mode=process.argv[2];
  if(!["--write","--check"].includes(mode)||process.argv.length!==3){
    process.stderr.write("usage: node m4c6_photoshop_readonly.cjs --write|--check\n");
    process.exit(2);
  }
  const report=buildReport();
  if(mode==="--write")fs.writeFileSync(EVIDENCE,report);
  else assert.equal(fs.readFileSync(EVIDENCE,"utf8"),report);
  process.stdout.write(JSON.stringify({mode,...JSON.parse(report).summary})+"\n");
}
module.exports={buildReport,readPhotoshopLayers,sortedNames};
