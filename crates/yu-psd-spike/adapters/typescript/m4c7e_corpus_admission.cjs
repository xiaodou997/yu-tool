"use strict";

// M4c-7e broader *read-only* PSD corpus classification.
// This does not add any fixture to M4c-2/M4c-7a writer allow-lists.
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const { spawnSync } = require("node:child_process");
const assert = require("node:assert/strict");
const { scan } = require("./m4c3_raw_block_inventory.cjs");
const ROOT = path.resolve(__dirname,"../../../../");
const OLD = require(path.join(ROOT,"docs/data/psd-raw-block-inventory-m4c3-v1.json"));
const ADMISSION = require(path.join(ROOT,"docs/data/psd-rename-scope-m4c7-v1.json"));
const OUTPUT = path.join(ROOT,"docs/data/m4c7e-readonly-corpus-profile-v1.json");
const PYTHON = process.env.M4C7D_PYTHON ||
  path.join(ROOT,"target/m4c7d-py312/bin/python");
const READER = path.join(ROOT,"crates/yu-psd-spike/adapters/m4c7e_psd_layer_profile.py");
const sha = b=>crypto.createHash("sha256").update(b).digest("hex");

function profile(file){
  const r=spawnSync(PYTHON,[READER,file],{
    cwd:ROOT,encoding:"utf8",timeout:25000,maxBuffer:1024*1024
  });
  if(r.error||r.status!==0)throw Error("independent read failed: "+
    (r.error?.message||r.stderr||r.stdout).slice(0,260));
  const x=JSON.parse(r.stdout);
  if(x.status!=="ok")throw Error("independent parse rejected fixture");
  return x.profile;
}
function inspect(c){
  const source=path.join(ROOT,c.path);
  const bytes=fs.readFileSync(source);
  assert.equal(sha(bytes),c.sha256);
  const raw=scan(bytes),parsed=profile(source);
  assert.equal(parsed.source_sha256,c.sha256);
  const names=parsed.layers.map(x=>x.name);
  const kinds=[...new Set(parsed.layers.map(x=>x.kind))].sort();
  const duplicateNames=names.filter((name,i)=>names.indexOf(name)!==i);
  const unicodeNames=names.filter(x=>/[^\u0000-\u007f]/.test(x));
  const highBit=raw.header.bit_depth!==8;
  const complexKinds=kinds.filter(k=>!["pixel","group"].includes(k));
  const reasons=[];
  if(highBit)reasons.push("high_bit_depth");
  if(duplicateNames.length)reasons.push("duplicate_logical_names");
  if(unicodeNames.length)reasons.push("non_ascii_names_require_legacy_fidelity");
  if(complexKinds.length)reasons.push("complex_layer_kinds_not_admitted");
  if(raw.summary.unknown_block_count>0)reasons.push("unknown_metadata_requires_preservation_review");
  const frozen=ADMISSION.known_research_cases.find(x=>{
    const old=OLD.cases.find(z=>z.fixture===x.fixture);
    return old?.sha256===c.sha256;
  });
  return {
    fixture:c.fixture,source_sha256:c.sha256,format:raw.header.format,
    bit_depth:raw.header.bit_depth,color_mode:raw.header.color_mode,
    logical_layer_count:parsed.layers.length,layer_kinds:kinds,
    duplicate_logical_name_count:duplicateNames.length,
    non_ascii_logical_name_count:unicodeNames.length,
    unknown_raw_metadata_count:raw.summary.unknown_block_count,
    existing_m4c7a_gate:frozen?.gate||"not_in_exact_hash_research_scope",
    next_research_classification:
      !highBit&&!complexKinds.length&&!duplicateNames.length&&
      !unicodeNames.length?
        "candidate_for_new_independent_read_only_editor_validation":
        "read_only_profile_only_not_rename_admitted",
    reasons,mutation_authorized:false,public_write_authorized:false
  };
}
function build(){
  const cases=OLD.cases.filter(x=>x.outcome==="structurally_parsed")
    .map(inspect).sort((a,b)=>a.fixture.localeCompare(b.fixture,"en"));
  assert.equal(cases.length,13);
  return {
    schema_version:"1",milestone:"M4c-7e",
    operation:"psd.readonly.fixture_classification.research",
    public_write_authorized:false,
    summary:{
      scanned:cases.length,
      unadmitted_to_public_writes:cases.length,
      new_editor_research_profile_candidates:cases.filter(x=>
        x.next_research_classification.startsWith("candidate_for")).length,
      production_writes_authorized:0
    },cases
  };
}
function check(r){
  assert.equal(r.summary.scanned,13);
  assert.equal(r.public_write_authorized,false);
  assert.equal(r.summary.production_writes_authorized,0);
  assert.ok(r.cases.every(c=>!c.public_write_authorized&&!c.mutation_authorized));
}
if(require.main===module){
  const mode=process.argv[2];
  if(mode==="--write" && process.argv.length===3){
    const r=build();check(r);
    fs.writeFileSync(OUTPUT,JSON.stringify(r,null,2)+"\n");
    console.log(JSON.stringify({mode,...r.summary}));
  } else if(mode==="--check"&&process.argv.length===3) {
    const r=JSON.parse(fs.readFileSync(OUTPUT,"utf8"));check(r);
    console.log(JSON.stringify({mode,offline:true,...r.summary}));
  } else {
    process.stderr.write("usage: node m4c7e_corpus_admission.cjs --write|--check\n");
    process.exitCode=2;
  }
}
module.exports={build,check};
