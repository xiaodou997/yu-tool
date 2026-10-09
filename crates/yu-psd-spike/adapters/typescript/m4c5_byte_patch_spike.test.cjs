"use strict";

const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const {
  CASES, buildReport, createCandidate, utf16be, decodeLuni
} = require("./m4c5_byte_patch_spike.cjs");
const { scan } = require("./m4c3_raw_block_inventory.cjs");
const { preflight, PreflightError } = require("./m4c2_safe_mutation_preflight.cjs");

const ROOT = path.resolve(__dirname, "../../../../");
const POLICY = require(path.join(ROOT, "docs/data/psd-safe-mutation-scope-m4c2-v1.json"));
const REPORT = path.join(ROOT, "docs/data/m4c5-psd-byte-patch-evidence-v1.json");
const AG = require(path.join(ROOT, "packaging/ag-psd-engine/node_modules/ag-psd"));
const sha = b => crypto.createHash("sha256").update(b).digest("hex");
function fixture(id) {
  return POLICY.known_fixtures.find(f => f.id === id);
}
function makeRequest(f, name) {
  return {
    contract_version: "1", operation: "rename",
    input_path: path.join(ROOT, f.path),
    expected_source_sha256: f.sha256,
    layer_id: f.research_target.layer_id,
    expected_old_name: f.research_target.name,
    new_name: name,
    output_path: path.join(ROOT, "target", "m4c5-placeholder." + f.format),
  };
}
function expectResearchRefusal(fn) {
  assert.throws(fn, e => e && e.code === "RESEARCH_REFUSAL");
}

test("frozen M4c-5 snapshot reproduces and authorizes zero production writes", () => {
  assert.equal(process.versions.node, "22.23.3");
  assert.equal(require(path.join(ROOT,"packaging/ag-psd-engine/node_modules/ag-psd/package.json")).version,"31.0.2");
  assert.equal(buildReport(), fs.readFileSync(REPORT, "utf8"));
  const data=JSON.parse(fs.readFileSync(REPORT,"utf8"));
  assert.equal(data.public_write_authorized,false);
  assert.equal(data.summary.tested,4);
  assert.equal(data.summary.research_byte_patches_verified,4);
  assert.equal(data.summary.production_writes_authorized,0);
  assert.equal(data.summary.legacy_name_representation_stale,3);
  assert.ok(data.cases.every(x=>
    x.status==="research_byte_patch_verified" &&
    x.raw_resources_equal && x.raw_layer_channels_equal &&
    x.raw_merged_composite_equal &&
    x.logical_tree_except_requested_name_equal &&
    x.source_bytes===x.candidate_bytes &&
    x.candidate_reopened_and_byte_verified &&
    !x.public_write_authorized));
  const group=data.cases.find(x=>x.fixture==="nested-group");
  assert.equal(group.raw_record_index,3);
  assert.equal(group.logical_layer_id,"L0002");
  assert.equal(group.legacy_pascal_updated,true);
  assert.equal(group.change_ranges.length,2);
  const duplicate=data.cases.find(x=>x.fixture==="duplicate-names");
  assert.equal(duplicate.logical_layer_id,"L0002");
  assert.equal(duplicate.raw_record_index,1);
  assert.equal(duplicate.original_unicode_name,"X");
  assert.equal(duplicate.updated_unicode_name,"Y");
});

test("direct raw-byte proof: all changed byte offsets belong to the target name ranges",()=>{
  for(const choice of CASES){
    const f=fixture(choice.fixture),original=fs.readFileSync(path.join(ROOT,f.path));
    const request=makeRequest(f,choice.new_name);
    assert.equal(preflight(request).write_authorized,false);
    const {bytes,evidence}=createCandidate(original,f,request,choice.raw_index,AG);
    assert.equal(original.length,bytes.length);
    const touched=evidence.change_ranges;
    let changed=0;
    for(let i=0;i<original.length;i++){
      if(original[i]!==bytes[i]){
        changed++;
        assert.ok(touched.some(r=>i>=r.offset&&i<r.offset+r.bytes),
          `non-name byte changed at ${i} in ${f.id}`);
      }
    }
    assert.equal(changed,evidence.changed_byte_count);
    assert.ok(changed>0);
    assert.equal(sha(original),f.sha256);
    assert.equal(sha(bytes),evidence.candidate_sha256);
    const before=scan(original),after=scan(bytes);
    assert.deepEqual(after.image_resources,before.image_resources);
    assert.deepEqual(after.layer_records.map(r=>r.channels),before.layer_records.map(r=>r.channels));
    assert.deepEqual(after.merged_image,before.merged_image);
    assert.deepEqual(after.header,before.header);
  }
});

test("fixed UTF-16BE unit count is compulsory; malformed Unicode/length changes are refused",()=>{
  const f=fixture("simple-psd");
  const original=fs.readFileSync(path.join(ROOT,f.path));
  const request=makeRequest(f,"Дом");
  for(const name of ["OneTooLong","", "\uD800\u0000","e\u0301","n\nq"]){
    expectResearchRefusal(()=>
      createCandidate(original,f,{...request,new_name:name},0,AG));
  }
  assert.equal(utf16be("Дом").length,6);
  assert.equal(utf16be("Group 2").length,14);
});

test("changed or ambiguous Unicode-name records cannot be guessed or replaced",()=>{
  const f=fixture("simple-psd"),original=fs.readFileSync(path.join(ROOT,f.path));
  const request=makeRequest(f,"Дом");
  expectResearchRefusal(()=>createCandidate(original,f,request,2,AG));
  expectResearchRefusal(()=>createCandidate(original,f,request,1,AG));
  let corrupted=Buffer.from(original);
  const tag=scan(original).tagged_blocks.find(x=>x.key==="luni"&&x.layer_index===0);
  corrupted.writeUInt32BE(999,tag.offset+12);
  expectResearchRefusal(()=>createCandidate(corrupted,f,request,0,AG));
  corrupted=Buffer.from(original);
  corrupted.write("YuZZ",tag.offset+4,"ascii");
  expectResearchRefusal(()=>createCandidate(corrupted,f,request,0,AG));
  assert.equal(sha(original),f.sha256);
});

test("source version guard, output no-clobber, and unsupported operation reject independently",()=>{
  const f=fixture("simple-psd");
  const request=makeRequest(f,"Дом");
  assert.throws(()=>preflight({...request,expected_source_sha256:"0".repeat(64)}),
    e=>e instanceof PreflightError&&e.code==="SOURCE_VERSION_CONFLICT");
  assert.throws(()=>preflight({...request,operation:"visibility"}),
    e=>e instanceof PreflightError&&e.code==="UNSUPPORTED_OPERATION");
  assert.throws(()=>preflight({...request,output_path:request.input_path}),
    e=>e instanceof PreflightError&&e.code==="OUTPUT_CONFLICT");
});
