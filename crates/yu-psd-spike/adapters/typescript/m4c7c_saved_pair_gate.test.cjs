"use strict";

const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { compareSavedPair } = require("./m4c7c_saved_pair_gate.cjs");
const { createCandidate } = require("./m4c5_byte_patch_spike.cjs");
const { scan } = require("./m4c3_raw_block_inventory.cjs");

const ROOT = path.resolve(__dirname,"../../../../");
const F = require(path.join(ROOT,"docs/data/psd-safe-mutation-scope-m4c2-v1.json"))
  .known_fixtures.find(f=>f.id==="nested-group");
const bytes=fs.readFileSync(path.join(ROOT,F.path));
const proposal={raw_index:3,expected_old_name:"Group 1",expected_new_name:"Group 2"};
function candidate(){
  const ag=require(path.join(ROOT,"packaging/ag-psd-engine/node_modules/ag-psd"));
  return createCandidate(bytes,F,{
    layer_id:"L0002",expected_old_name:"Group 1",new_name:"Group 2",
  },3,ag).bytes;
}
test("known fixed-width double-name rename passes strict raw paired comparison",()=>{
  const patched=candidate();
  const result=compareSavedPair(bytes,patched,proposal);
  assert.equal(result.strict_saved_pair_bytes_passed,true);
  assert.equal(result.public_write_authorized,false);
  assert.deepEqual(result.tags.changed,["layer:3:luni#0"]);
  assert.deepEqual(result.legacy_name_changed_raw_indices,[3]);
  assert.equal(result.merged_composite_equal,true);
  assert.deepEqual(result.resources,{added:[],removed:[],changed:[]});
  assert.deepEqual(result.channels,{added:[],removed:[],changed:[]});
});
test("same byte input, wrong target, and wrong expected new name never pass",()=>{
  assert.equal(compareSavedPair(bytes,bytes,proposal).strict_saved_pair_bytes_passed,false);
  assert.equal(compareSavedPair(bytes,candidate(),
    {...proposal,expected_new_name:"Other"}).strict_saved_pair_bytes_passed,false);
  assert.throws(()=>compareSavedPair(bytes,candidate(),
    {...proposal,raw_index:77}),/missing target raw record/);
});
test("mutating an unrelated image resource or compressed layer channel fails",()=>{
  const parsed=scan(bytes);
  const resource=parsed.image_resources.find(r=>r.length>3);
  assert.ok(resource);
  let altered=Buffer.from(candidate());
  const rawBlock=parsed.sections.find(x=>x.name==="image_resources");
  const start=resource.offset+12; // known resource data offset for empty Pascal name
  assert.ok(start>=rawBlock.offset&&start<rawBlock.offset+rawBlock.length);
  altered[start]^=1;
  let check=compareSavedPair(bytes,altered,proposal);
  assert.equal(check.strict_saved_pair_bytes_passed,false);
  assert.ok(check.resources.changed.length>0);

  altered=Buffer.from(candidate());
  const channel=parsed.layer_records.flatMap(r=>r.channels)
    .find(c=>c.length>4);
  assert.ok(channel);
  altered[channel.data_offset+2]^=1;
  check=compareSavedPair(bytes,altered,proposal);
  assert.equal(check.strict_saved_pair_bytes_passed,false);
  assert.ok(check.channels.changed.length>0);
});
test("unexpected metadata tagged block differences are rejected",()=>{
  const orig=scan(bytes);
  const tag=orig.tagged_blocks.find(t=>
    t.key!=="luni" && t.length>3 && t.scope==="layer");
  assert.ok(tag);
  const altered=Buffer.from(candidate());
  const offset=tag.offset+8+tag.length_width;
  altered[offset]^=1;
  const report=compareSavedPair(bytes,altered,proposal);
  assert.equal(report.strict_saved_pair_bytes_passed,false);
  assert.ok(report.tags.changed.length>1);
});
