"use strict";

const {test}=require("node:test");
const assert=require("node:assert/strict");
const fs=require("node:fs");
const path=require("node:path");
const {main,compare,bytesInventory}=require("./m4c4_noop_fidelity.cjs");
const ROOT=path.resolve(__dirname,"../../../../");
const SNAPSHOT=path.join(ROOT,"docs/data/m4c4-psd-noop-fidelity-v1.json");

test("pinned ag-psd 31.0.2 / Node 22.23.3 no-op evidence is deterministic",()=>{
  const expected=fs.readFileSync(SNAPSHOT,"utf8");
  const actual=main();
  assert.equal(actual,expected,"fidelity snapshot must be reproducible");
  const r=JSON.parse(actual);
  assert.equal(r.runtime.node,"22.23.3");
  assert.equal(r.runtime.ag_psd,"31.0.2");
  assert.equal(r.public_write_authorized,false);
  assert.equal(r.summary.fixture_count,13);
  assert.equal(r.summary.saved_reopened,11);
  assert.equal(r.summary.failed_closed,2);
  assert.equal(r.summary.raw_diff_found,11);
  assert.equal(r.summary.semantic_composite_mismatch,1);
  assert.equal(r.summary.fidelity_certified,0);
  assert.ok(r.cases.every(x=>x.source_unchanged&&!x.public_write_authorized));

  const find=id=>{let x=r.cases.find(x=>x.fixture===id);
    assert.ok(x,`missing fixture ${id}`);return x;};
  for(const id of ["bench-high-bit-rgb","bench-high-bit-psb"]){
    const x=find(id);
    assert.equal(x.status,"failed_closed");
    assert.match(x.error,/bitsPerChannel other than 8/i);
  }
  const composite=find("bench-baseline-rgb");
  assert.equal(composite.semantic_check.composite_pixels_equal,false);
  const advanced=find("bench-advanced-blending");
  assert.ok(advanced.raw_diff.resources.removed_count>0);
  assert.ok(advanced.raw_diff.unknown.removed_count>0);
  const smart=find("bench-smart-object");
  assert.ok(smart.raw_diff.additional_layer_blocks.removed_count>0);
  assert.ok(smart.raw_diff.additional_layer_blocks.added_count>0);
  const simple=find("simple-pixel-layers-psd");
  assert.ok(simple.raw_diff.resources.added_count>0);
  assert.equal(simple.semantic_check.decoded_layer_image_hashes_equal,true);
  assert.equal(simple.fidelity_certified,false);
});

test("source-vs-source comparison is exactly equal and difference-free",()=>{
  const b=fs.readFileSync(path.join(ROOT,"fixtures/psd/upstream/psd-tools/2layers.psd"));
  const a=bytesInventory(b),c=bytesInventory(Buffer.from(b));
  const d=compare(a,c);
  assert.equal(d.byte_equal,true);
  assert.equal(d.differences_found,false);
  assert.equal(d.merged_composite_byte_equal,true);
  for(const name of ["resources","additional_layer_blocks","channels",
    "layer_properties","sections","unknown","nested_layer_properties","nested_channel_bytes"]){
    assert.equal(d[name].added_count,0);
    assert.equal(d[name].removed_count,0);
    assert.equal(d[name].changed_count,0);
  }
});
