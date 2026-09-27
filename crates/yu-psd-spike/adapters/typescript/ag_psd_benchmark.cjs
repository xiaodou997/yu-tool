"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");

const UNAVAILABLE_EXIT = 3;

function unavailable(message) {
  console.error(message);
  process.exit(UNAVAILABLE_EXIT);
}

function readArg(name) {
  const index = process.argv.indexOf(name);
  if (index === -1 || index + 1 >= process.argv.length) {
    unavailable("missing required argument: " + name);
  }
  return process.argv[index + 1];
}

const expectedVersion = readArg("--expected-version");
const expectedNodeMajor = readArg("--expected-node-major");
const warmup = Number(readArg("--warmup"));
const iterations = Number(readArg("--iterations"));
const input = process.argv[process.argv.length - 1];

const actualNodeMajor = process.versions.node.split(".")[0];
if (actualNodeMajor !== expectedNodeMajor) {
  unavailable(
    "Node.js version mismatch: expected major " +
      expectedNodeMajor +
      ", got " +
      process.versions.node
  );
}

let readPsd;
let getLayerImageData;
let initializeCanvas;
let packageVersion;
try {
  ({ readPsd, getLayerImageData, initializeCanvas } = require("ag-psd"));
  packageVersion = require("ag-psd/package.json").version;
} catch (error) {
  unavailable(
    "ag-psd is unavailable: " +
      (error instanceof Error ? error.message : String(error))
  );
}

if (packageVersion !== expectedVersion) {
  unavailable(
    "ag-psd version mismatch: expected " +
      expectedVersion +
      ", got " +
      packageVersion
  );
}

// getLayerImageData() allocates through ag-psd's createImageData hook even
// with useRawData enabled. Supply a pure in-memory ImageData implementation
// so the RGBA benchmark does not require node-canvas or a native graphics runtime.
initializeCanvas(
  () => {
    throw new Error("canvas allocation is disabled in the PSD benchmark");
  },
  (width, height) => ({
    width,
    height,
    data: new Uint8ClampedArray(width * height * 4),
  })
);

const buffer = fs.readFileSync(input);

function visit(children, callback) {
  if (!Array.isArray(children)) return;
  for (const layer of children) {
    callback(layer);
    visit(layer.children, callback);
  }
}

function parseOnce() {
  const psd = readPsd(buffer, {
    skipLayerImageData: true,
    skipCompositeImageData: true,
    skipThumbnail: true,
    skipLinkedFilesData: true,
    logMissingFeatures: false,
  });
  let count = 0;
  visit(psd.children, () => {
    count += 1;
  });
  return count;
}

function exportOnce() {
  const psd = readPsd(buffer, {
    useRawData: true,
    skipCompositeImageData: true,
    skipThumbnail: true,
    skipLinkedFilesData: true,
    logMissingFeatures: false,
  });

  const started = process.hrtime.bigint();
  const hash = crypto.createHash("sha256");
  let exportedLayerCount = 0;
  let totalRgbaBytes = 0;

  visit(psd.children, (layer) => {
    const image = getLayerImageData(layer);
    if (!image || !image.data) return;
    const raw = Buffer.from(
      image.data.buffer,
      image.data.byteOffset,
      image.data.byteLength
    );
    hash.update(raw);
    exportedLayerCount += 1;
    totalRgbaBytes += raw.byteLength;
  });

  const elapsedMs = Number(process.hrtime.bigint() - started) / 1_000_000;
  return {
    elapsedMs,
    exportedLayerCount,
    totalRgbaBytes,
    checksum: hash.digest("hex"),
  };
}

for (let i = 0; i < warmup; i += 1) parseOnce();

const warmParseSamplesMs = [];
for (let i = 0; i < iterations; i += 1) {
  const started = process.hrtime.bigint();
  parseOnce();
  warmParseSamplesMs.push(
    Number(process.hrtime.bigint() - started) / 1_000_000
  );
}

for (let i = 0; i < warmup; i += 1) exportOnce();

const layerExportSamplesMs = [];
let fingerprint = null;
for (let i = 0; i < iterations; i += 1) {
  const result = exportOnce();
  const current =
    result.exportedLayerCount +
    ":" +
    result.totalRgbaBytes +
    ":" +
    result.checksum;
  if (fingerprint === null) {
    fingerprint = current;
  } else if (fingerprint !== current) {
    throw new Error("ag-psd layer export fingerprint changed between iterations");
  }
  layerExportSamplesMs.push(result.elapsedMs);
}

if (fingerprint === null) {
  throw new Error("ag-psd benchmark produced no export fingerprint");
}

const [layerCount, byteCount, checksum] = fingerprint.split(":");
process.stdout.write(
  JSON.stringify({
    warm_parse_samples_ms: warmParseSamplesMs,
    layer_export_samples_ms: layerExportSamplesMs,
    exported_layer_count: Number(layerCount),
    total_rgba_bytes: Number(byteCount),
    export_checksum_sha256: checksum,
    peak_rss_bytes: process.resourceUsage().maxRSS * 1024,
  })
);
