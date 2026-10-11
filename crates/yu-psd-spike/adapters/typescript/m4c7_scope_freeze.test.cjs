"use strict";

const {test}=require("node:test");
const assert=require("node:assert/strict");
const fs=require("node:fs");
const path=require("node:path");
const {assess,buildReport,POLICY}=require("./m4c7_scope_freeze.cjs");
const {saveAsIndependentCopy}=require("./m4c7_photoshop_save_probe.cjs");

const ROOT=path.resolve(__dirname,"../../../../");
const SNAPSHOT=require(path.join(ROOT,"docs/data/m4c7-rename-scope-evidence-v1.json"));
const NATIVE=require(path.join(ROOT,"docs/data/m4c7-photoshop-control-v1.json"));

test("four frozen research input identities are default-denied except one compatible group",()=>{
  const report=buildReport();
  assert.deepEqual(report,SNAPSHOT);
  assert.equal(report.public_write_authorized,false);
  assert.equal(POLICY.public_write_authorized,false);
  assert.equal(report.summary.examined,4);
  assert.equal(report.summary.research_save_reopen_candidates,1);
  assert.equal(report.summary.blocked,3);
  assert.equal(report.summary.production_writes_authorized,0);
  assert.deepEqual(report.cases.filter(x=>x.gate==="research_save_reopen_candidate")
    .map(x=>x.fixture),["nested-group"]);
  assert.ok(report.cases.every(x=>!x.public_write_authorized));
});

test("native Photoshop Background and out-of-sync legacy layer names are excluded",()=>{
  for(const f of ["simple-psd","simple-psb"]) {
    const item=SNAPSHOT.cases.find(x=>x.fixture===f);
    assert.equal(item.gate,"blocked");
    assert.ok(item.reasons.includes("native_Photoshop_read_failed"));
    assert.ok(item.reasons.includes("legacy_Pascal_mismatch"));
  }
  const duplicate=SNAPSHOT.cases.find(x=>x.fixture==="duplicate-names");
  assert.equal(duplicate.gate,"blocked");
  assert.deepEqual(duplicate.reasons,["legacy_Pascal_mismatch"]);
  const group=SNAPSHOT.cases.find(x=>x.fixture==="nested-group");
  assert.equal(group.gate,"research_save_reopen_candidate");
  assert.equal(group.raw_legacy_ascii_matches_unicode,true);
});

test("mismatched layer id, raw index, or unreviewed fixture cannot enter scope",()=>{
  const group=POLICY.known_research_cases.find(x=>x.fixture==="nested-group");
  assert.throws(()=>assess({...group,layer_id:"L0001"}),/wrong canonical/);
  assert.throws(()=>assess({...group,raw_index:1}),/raw Unicode|mismatch|name/i);
  assert.throws(()=>assess({...group,fixture:"fake-file"}),/unknown research fixture/);
  const changedName=assess({...group,proposed_name:"New Name Too Long"});
  assert.equal(changedName.gate,"blocked");
  assert.ok(changedName.reasons.includes("UTF16_code_unit_length_not_equal"));
});

test("Photoshop research-save receipt never upgrades a failed native gate",()=>{
  assert.equal(NATIVE.public_write_authorized,false);
  assert.equal(NATIVE.summary.admitted_cases,1);
  assert.equal(NATIVE.summary.product_write_authorizations,0);
  assert.ok(NATIVE.cases.every(x=>!x.public_write_authorized));
  assert.equal(NATIVE.cases[0].fixture,"nested-group");
  // No Photoshop document was saved when the native automation interface
  // failed. This blocked receipt is valid evidence, not a passed fidelity gate.
  if(NATIVE.status==="blocked_native_application_unresponsive") {
    assert.equal(NATIVE.cases[0].control_saved,false);
    assert.equal(NATIVE.cases[0].candidate_saved,false);
    assert.equal(NATIVE.summary.paired_research_save_reopen_passed,0);
  }
});

test("Photoshop output destination guards reject any path outside private scratch",()=>{
  const fixture=path.join(ROOT,"fixtures/psd/upstream/psd-tools/group.psd");
  const scratch=path.join(ROOT,"target/m4c7-psd-photoshop-save");
  fs.mkdirSync(scratch,{recursive:true});
  assert.throws(()=>saveAsIndependentCopy(fixture,
    path.join(scratch,"unapproved.psd")));
  assert.throws(()=>saveAsIndependentCopy(fixture,fixture));
});
