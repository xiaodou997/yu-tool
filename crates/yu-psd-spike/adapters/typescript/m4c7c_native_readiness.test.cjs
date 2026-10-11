"use strict";

const assert = require("node:assert/strict");
const { test } = require("node:test");
const path = require("node:path");
const { classifyPhotoshopReadiness } = require("./m4c7c_native_readiness.cjs");
const RECEIPT = require(path.resolve(__dirname,
  "../../../../docs/data/m4c7c-photoshop-control-v2.json"));

function runCases(processReply, queryReply) {
  const calls=[];
  const result=classifyPhotoshopReadiness({
    platform:"darwin",timeoutMs:1500,
    run(executable,args,opts) {
      calls.push({executable,args,timeout:opts.timeout});
      if (executable==="/usr/bin/pgrep") return processReply;
      if (executable==="/usr/bin/osascript") return queryReply;
      throw Error("unapproved command");
    },
  });
  return {result,calls};
}

test("must not launch Photoshop when it is not running",()=>{
  const absent=runCases({status:1,stdout:""},{status:0,stdout:"0\n"});
  assert.equal(absent.result.status,"photoshop_not_running");
  assert.equal(absent.calls.length,1);
  assert.equal(absent.result.ready,false);
});
test("read-only native document count must be zero, not merely responsive",()=>{
  for(const [text,expected,ready] of [
    ["0\n","ready_empty_workspace",true],
    ["2\n","existing_documents_open",false],
    ["whoops\n","unexpected_application_response",false],
  ]) {
    const {result,calls}=runCases({status:0,stdout:"30123\n"},
      {status:0,stdout:text});
    assert.equal(result.status,expected);
    assert.equal(result.ready,ready);
    assert.equal(calls.length,2);
    assert.ok(calls[1].args[1].includes("count documents"));
    assert.equal(calls[1].timeout,1500);
  }
});
test("Apple event hangs, permission denial, and unexpected errors fail closed",()=>{
  const outcomes=[
    [{status:null,error:{code:"ETIMEDOUT"}},"apple_events_unresponsive"],
    [{status:1,stderr:"(-1743) Not authorized"},"native_permission_denied"],
    [{status:1,stderr:"(-1728) 不允许辅助访问"},"native_permission_denied"],
    [{status:1,stderr:"(-1728) not permitted"},"native_permission_denied"],
    [{status:1,stderr:"application error"},"apple_events_error"],
    [{status:0,stdout:"-1"},"unexpected_application_response"],
  ];
  for(const [response,status] of outcomes){
    const {result}=runCases({status:0,stdout:"400\n"},response);
    assert.equal(result.ready,false);
    assert.equal(result.status,status);
  }
});
test("other platforms are rejected without process inspection",()=>{
  const result=classifyPhotoshopReadiness({platform:"linux",run(){
    throw Error("must not invoke process");
  }});
  assert.equal(result.status,"unsupported_platform");
  assert.equal(result.ready,false);
});
test("invalid timeout cannot make an unbounded probe",()=>{
  assert.throws(()=>classifyPhotoshopReadiness({
    platform:"darwin",timeoutMs:60000,run(){throw Error("not called");}
  }),/bounded/);
});
test("M4c-7c recorded blocked native receipt must never imply a successful save",()=>{
  assert.equal(RECEIPT.status,"blocked_native_application_unresponsive");
  assert.deepEqual(RECEIPT.native_preflight,{
    status:"apple_events_unresponsive",ready:false
  });
  assert.equal(RECEIPT.summary.admitted_cases,1);
  assert.equal(RECEIPT.summary.paired_research_save_reopen_passed,0);
  assert.equal(RECEIPT.summary.product_write_authorizations,0);
  assert.ok(RECEIPT.cases.every(c=>
    !c.save_command_attempted && !c.control_saved && !c.candidate_saved &&
    !c.public_write_authorized));
});
