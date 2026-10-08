"use strict";

const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");
const crypto = require("node:crypto");
const { spawnSync } = require("node:child_process");

const ROOT = path.resolve(__dirname, "../../../../");
const SCRIPT = path.join(__dirname, "m4c2_safe_mutation_preflight.cjs");
const POLICY = require(path.join(ROOT, "docs/data/psd-safe-mutation-scope-m4c2-v1.json"));
const EVIDENCE = require(path.join(ROOT, "docs/data/m4c1-psd-mutation-evidence-v1.json"));
const sha = bytes => crypto.createHash("sha256").update(bytes).digest("hex");

function run(request) {
  const processResult = spawnSync(process.execPath, [SCRIPT], {
    cwd: ROOT,
    input: JSON.stringify(request),
    encoding: "utf8",
    timeout: 30000,
    maxBuffer: 1024 * 1024,
  });
  assert.equal(processResult.stderr, "", processResult.stderr);
  return {
    exit_code: processResult.status,
    ...JSON.parse(processResult.stdout)
  };
}

function makeRequest(fixture, output) {
  return {
    contract_version: "1",
    operation: "rename",
    input_path: path.join(ROOT, fixture.path),
    expected_source_sha256: fixture.sha256,
    layer_id: fixture.research_target.layer_id,
    expected_old_name: fixture.research_target.name,
    new_name: "YuTool research rename",
    output_path: output || path.join(os.tmpdir(), `yu-m4c2-${fixture.id}.${fixture.format}`),
  };
}

function expectError(request, errorCode, exitCode) {
  const result = run(request);
  assert.equal(result.status, "error", JSON.stringify(result));
  assert.equal(result.error.code, errorCode);
  assert.equal(result.exit_code, exitCode);
  return result;
}

test("frozen policy is derived only from independently observed M4c-1 research candidates", () => {
  assert.equal(POLICY.public_mutation_authorized, false);
  assert.equal(POLICY.preflight_only, true);
  assert.deepEqual(POLICY.research_admission.operations, ["rename"]);
  assert.equal(POLICY.research_admission.mode, "exact_known_fixture_sha256_only");
  assert.equal(POLICY.known_fixtures.length, 4);
  assert.equal(POLICY.engine.version, "31.0.2");
  assert.equal(POLICY.engine.node_version, "22.23.3");
  for (const fixture of POLICY.known_fixtures) {
    const source = fs.readFileSync(path.join(ROOT, fixture.path));
    const evidence = EVIDENCE.trials.find(t =>
      t.fixture === fixture.id && t.operation === "rename"
    );
    assert.equal(sha(source), fixture.sha256);
    assert.equal(evidence.source_sha256, fixture.sha256);
    assert.equal(evidence.restricted_rename_candidate, true);
    assert.equal(evidence.layer_id, fixture.research_target.layer_id);
    assert.equal(evidence.layer_kind, fixture.research_target.kind);
    assert.equal(evidence.format, fixture.format);
    assert.equal(evidence.bits_per_channel, POLICY.research_admission.bit_depth);
  }
});

test("four exact fixture identities yield read-only, non-authorizing stable plans", () => {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "yu-m4c2-positive-"));
  try {
    for (const fixture of POLICY.known_fixtures) {
      const request = makeRequest(fixture, path.join(home, fixture.id + "." + fixture.format));
      const original = fs.readFileSync(request.input_path);
      const result = run(request);
      assert.equal(result.exit_code, 0, JSON.stringify(result));
      assert.equal(result.status, "ok");
      assert.equal(result.plan.write_authorized, false);
      assert.equal(result.plan.side_effects, "none");
      assert.equal(result.plan.status, "research_candidate");
      assert.equal(result.plan.dry_run, true);
      assert.equal(result.plan.output.policy, "new_file_only");
      assert.equal(result.plan.source.sha256, fixture.sha256);
      assert.equal(result.plan.source.fixture_profile, fixture.id);
      assert.equal(result.plan.target.layer_id, fixture.research_target.layer_id);
      assert.equal(result.plan.target.expected_old_name, fixture.research_target.name);
      assert.equal(result.plan.engine.node_version, "22.23.3");
      assert.match(result.plan.policy_sha256, /^[0-9a-f]{64}$/);
      assert.equal(fs.existsSync(request.output_path), false);
      assert.equal(sha(fs.readFileSync(request.input_path)), sha(original));
      assert.deepEqual(run(request), result, "same input must yield identical plan");
    }
    assert.deepEqual(fs.readdirSync(home), [], "preflight must not create any output");
  } finally {
    fs.rmSync(home, { recursive: true, force: true });
  }
});

test("unsupported mutation and unexpected request fields fail closed", () => {
  const fixture = POLICY.known_fixtures[0];
  for (const operation of ["visibility", "opacity", "delete", "move", "noop"]) {
    expectError({ ...makeRequest(fixture), operation }, "UNSUPPORTED_OPERATION", 3);
  }
  expectError({ ...makeRequest(fixture), overwrite: true }, "INVALID_ARGUMENT", 2);
  const missingHash = makeRequest(fixture);
  delete missingHash.expected_source_sha256;
  expectError(missingHash, "INVALID_ARGUMENT", 2);
  expectError({ ...makeRequest(fixture), contract_version: "2" }, "INVALID_ARGUMENT", 2);
});

test("only the exact reviewed canonical layer ID and expected old name are accepted", () => {
  const simple = POLICY.known_fixtures[0];
  const duplicate = POLICY.known_fixtures.find(f => f.id === "duplicate-names");
  const group = POLICY.known_fixtures.find(f => f.id === "nested-group");
  assert.equal(run(makeRequest(group)).plan.target.kind, "group");
  assert.equal(run(makeRequest(duplicate)).plan.target.layer_id, "L0002");
  for (const id of ["L1", "L0000", "X", "L9999", "L0002", "L010000"]) {
    const canonical = /^L\d{4,}$/.test(id) &&
      "L" + String(Number(id.slice(1))).padStart(4, "0") === id;
    expectError({ ...makeRequest(simple), layer_id: id },
      canonical ? "UNSUPPORTED_DOCUMENT" : "INVALID_ARGUMENT",
      canonical ? 3 : 2);
  }
  expectError({
    ...makeRequest(duplicate),
    layer_id: "L0001"
  }, "UNSUPPORTED_DOCUMENT", 3);
  expectError({ ...makeRequest(simple), expected_old_name: "Слой" },
    "SOURCE_VERSION_CONFLICT", 2);
});

test("new names must be bounded and unambiguous", () => {
  const f = POLICY.known_fixtures[0];
  for (const name of ["", "  Padding", "extra ", "line\nbreak", "a\0b",
    "x".repeat(81), "Фон", "e\u0301"]) {
    expectError({ ...makeRequest(f), new_name: name }, "INVALID_ARGUMENT", 2);
  }
  assert.equal(run({ ...makeRequest(f), new_name: "同名图层" }).status, "ok");
});

test("input SHA/version mismatch, arbitrary docs, and non-approved features are rejected", () => {
  const f = POLICY.known_fixtures[0];
  expectError({ ...makeRequest(f), expected_source_sha256: "0".repeat(64) },
    "SOURCE_VERSION_CONFLICT", 2);
  expectError({ ...makeRequest(f), expected_source_sha256: "not-sha" },
    "INVALID_ARGUMENT", 2);
  const arbitraryPaths = [
    "fixtures/psd/upstream/psd-tools/type-layer.psd",
    "fixtures/psd/upstream/psd-tools/masks-2.psd",
    "fixtures/psd/benchmark/upstream/psd-tools/layer-effects.psd",
    "fixtures/psd/benchmark/upstream/psd-tools/placed-layer.psd",
    "fixtures/psd/benchmark/upstream/psd-tools/background-red-opacity-80.psd",
    "fixtures/psd/benchmark/upstream/psd-tools/posterize-16bit-rgb.psd",
    "fixtures/psd/benchmark/upstream/psd-tools/32bit.psb"
  ];
  for (const file of arbitraryPaths) {
    const actual = path.join(ROOT, file);
    expectError({
      ...makeRequest(f),
      input_path: actual,
      expected_source_sha256: sha(fs.readFileSync(actual))
    }, "UNSUPPORTED_DOCUMENT", 3);
  }
});

test("altered bytes and copying an exact fixture have distinct outcomes", () => {
  const f = POLICY.known_fixtures[0];
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "yu-m4c2-copy-"));
  const copied = path.join(home, "copy.psd");
  const output = path.join(home, "out.psd");
  try {
    fs.copyFileSync(path.join(ROOT, f.path), copied);
    const request = { ...makeRequest(f, output), input_path: copied };
    assert.equal(run(request).status, "ok", "exact copied bytes have same reviewed identity");
    fs.appendFileSync(copied, Buffer.from([0x1]));
    expectError(request, "SOURCE_VERSION_CONFLICT", 2);
    expectError({
      ...request,
      expected_source_sha256: sha(fs.readFileSync(copied))
    }, "UNSUPPORTED_DOCUMENT", 3);
    assert.equal(fs.existsSync(output), false);
  } finally {
    fs.rmSync(home, { recursive: true, force: true });
  }
});

test("source/output collision, existing output and unsupported extensions fail closed", () => {
  const f = POLICY.known_fixtures[0];
  expectError({ ...makeRequest(f), output_path: path.join(ROOT, f.path) },
    "OUTPUT_CONFLICT", 2);
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "yu-m4c2-dest-"));
  const destination = path.join(home, "file.psd");
  try {
    fs.writeFileSync(destination, Buffer.from("do not overwrite"));
    expectError(makeRequest(f, destination), "OUTPUT_CONFLICT", 2);
    assert.equal(fs.readFileSync(destination, "utf8"), "do not overwrite");
    fs.rmSync(destination);
    expectError(makeRequest(f, path.join(home, "out.psb")),
      "INVALID_ARGUMENT", 2);
    expectError(makeRequest(f, path.join(home, "absent", "out.psd")),
      "INVALID_INPUT", 2);
    fs.symlinkSync("missing.psd", destination);
    expectError(makeRequest(f, destination), "OUTPUT_CONFLICT", 2);
    assert.equal(fs.readlinkSync(destination), "missing.psd");
  } finally {
    fs.rmSync(home, { recursive: true, force: true });
  }
});

test("the preflight CLI reports machine-readable errors without printing to stderr", () => {
  const invalid = spawnSync(process.execPath, [SCRIPT], {
    cwd: ROOT, encoding: "utf8", input: "{ invalid-json"
  });
  assert.equal(invalid.status, 2);
  assert.equal(invalid.stderr, "");
  assert.equal(JSON.parse(invalid.stdout).error.code, "INVALID_ARGUMENT");
});
