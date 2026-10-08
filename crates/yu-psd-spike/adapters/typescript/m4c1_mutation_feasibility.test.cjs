"use strict";

const assert = require("node:assert/strict");
const { test } = require("node:test");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.resolve(__dirname, "../../../../");
const SCRIPT = path.join(__dirname, "m4c1_mutation_feasibility.cjs");

test("M4c-1 PSD mutation evidence remains reproducible and never authorizes public mutation", () => {
  const run = spawnSync(process.execPath, ["--max-old-space-size=512", SCRIPT], {
    cwd: ROOT,
    encoding: "utf8",
    timeout: 120_000,
    maxBuffer: 1024 * 1024,
  });
  assert.equal(run.status, 0, `probe did not complete: ${run.stderr || run.stdout}`);
  const summary = JSON.parse(run.stdout.trim());
  const report = JSON.parse(fs.readFileSync(
    path.join(ROOT, "target", "m4c1-psd-mutation", "evidence.json"), "utf8"
  ));
  assert.equal(report.runtime.node, "22.23.3");
  assert.equal(report.engine.version, "31.0.2");
  assert.equal(summary.trial_count, 48);
  assert.equal(report.trials.length, 48);
  assert.equal(summary.written_and_reparsed, 40);
  assert.equal(summary.failed_closed, 8);
  assert.equal(summary.public_mutation_authorized, 0);
  assert.ok(report.trials.every(t => t.source_unchanged));
  assert.ok(report.trials.every(t => t.status !== "written_and_reparsed" || t.mutation_applied));

  const find = (fixture, operation) => {
    const found = report.trials.find(t => t.fixture === fixture && t.operation === operation);
    assert.ok(found, `missing trial: ${fixture}/${operation}`);
    return found;
  };
  for (const fixture of ["simple-psd", "simple-psb", "nested-group", "duplicate-names"]) {
    assert.equal(find(fixture, "rename").restricted_rename_candidate, true);
  }
  assert.equal(find("nested-group", "rename").layer_kind, "group");
  assert.equal(find("duplicate-names", "rename").layer_id, "L0002");
  assert.equal(find("advanced-blending", "noop").structure_preserved_except_target, false);
  assert.equal(find("layer-effects", "noop").structure_preserved_except_target, false);
  assert.equal(find("baseline-opacity", "noop").composite_preserved, false);
  assert.equal(find("simple-psd", "visibility").stale_composite_risk, true);
  assert.equal(find("simple-psd", "opacity").stale_composite_risk, true);
  assert.equal(find("high-bit-psd", "rename").status, "failed_closed");
  assert.equal(find("high-bit-psb", "rename").status, "failed_closed");
});
