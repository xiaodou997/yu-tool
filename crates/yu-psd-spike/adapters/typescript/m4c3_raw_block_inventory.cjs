"use strict";

// M4c-3: strict, bounded PSD/PSB byte-structure inventory. RESEARCH ONLY.
// Never writes PSD bytes, interprets opaque payloads, or authorizes mutation.
// Structural recognition is not a guarantee that ag-psd preserves the data.

const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");

const MAX_FILE = 8 * 1024 * 1024;
const MAX_PIXELS = 2_000_000;
const MAX_LAYERS = 64;
const MAX_CHANNELS = 128;
const MAX_BLOCKS = 1024;
const PSB_LONG_KEYS = new Set([
  "LMsk", "Lr16", "Lr32", "Layr", "Mt16", "Mt32", "Mtrn",
  "Alph", "FMsk", "lnk2", "FEid", "FXid", "PxSD",
]);

// These are *identified*, not marked safe to serialize. Unrecognized values
// are reported distinctly. A known key may still hold unknown sub-records.
const RECOGNIZED_LAYER_KEYS = new Set([
  "luni", "lyid", "lsct", "lsdk", "lclr", "lspf", "fxrp", "lrFX", "lfx2",
  "lfxs", "iOpa", "shmd", "TySh", "Txt2", "vmsk", "vsms", "vscg", "vscw",
  "SoLd", "SoLE", "PlLd", "lnkD", "lnk2", "lnk3", "lnkE", "lnkF",
  "GdFl", "PtFl", "SoCo", "brit", "levl", "curv", "hue ", "hue2",
  "vibA", "expA", "phfl", "selc", "mixr", "grdm", "blnc",
  "Layr", "Lr16", "Lr32", "LMsk", "Mt16", "Mt32", "Mtrn",
  "Alph", "FMsk", "FEid", "FXid", "PxSD", "artb", "artd",
  "sn2P", "tsly", "infx", "knko", "clbl", "lmgm", "brst",
  "vstk", "vogk", "vogf", "cinf", "CgEd", "Pat2", "Pat3",
]);
const RECOGNIZED_RESOURCE_IDS = new Set([
  1005, 1006, 1007, 1008, 1009, 1010, 1011, 1022, 1024, 1026,
  1028, 1032, 1033, 1034, 1035, 1036, 1037, 1039, 1040, 1041,
  1042, 1043, 1044, 1045, 1046, 1047, 1049, 1050, 1051, 1052,
  1053, 1054, 1057, 1058, 1059, 1060, 1061, 1062, 1064, 1065,
  1066, 1067, 1069, 1070, 1071, 1072, 1073, 1074, 1075, 1076,
  1077, 1078, 1079, 1080, 1082, 1083, 1084, 1085, 1086, 1087,
  1088, 1089, 1090, 1091, 1092, 1093, 1094, 1095, 1096, 1097,
  1098, 1099, 1100, 1101, 1102, 1103, 1104, 1105, 1106, 1107,
  1108, 1109, 1110, 1111, 1112, 1113, 1114, 1115, 1116, 1117,
  1118, 1119, 1120, 1121, 1122, 1123, 1124, 1125, 1126, 1127,
  1128, 1129, 1130, 1131, 1132, 1133, 1134, 1135, 1136, 1137,
  1138, 1139, 1140, 1141, 1142, 1143, 1144, 1145, 1146,
]);

class ScanError extends Error {
  constructor(code, message, offset) {
    super(message);
    this.code = code;
    this.offset = offset;
  }
}

function fail(message, offset, code = "INVALID_INPUT") {
  throw new ScanError(code, message, offset);
}
function sha(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

class Cursor {
  constructor(buf, start = 0, end = buf.length, label = "file") {
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) ||
        start < 0 || end > buf.length || end < start) {
      fail(`invalid cursor bounds for ${label}`, start);
    }
    this.buf = buf;
    this.pos = start;
    this.end = end;
    this.label = label;
  }
  left() { return this.end - this.pos; }
  require(count, label = "data") {
    if (!Number.isSafeInteger(count) || count < 0 || count > this.left()) {
      fail(`${this.label}: truncated or out-of-bounds ${label} (${count} bytes)`, this.pos);
    }
  }
  u8() { this.require(1); return this.buf.readUInt8(this.pos++); }
  u16() { this.require(2); const v = this.buf.readUInt16BE(this.pos); this.pos += 2; return v; }
  i16() { this.require(2); const v = this.buf.readInt16BE(this.pos); this.pos += 2; return v; }
  i32() { this.require(4); const v = this.buf.readInt32BE(this.pos); this.pos += 4; return v; }
  u32() { this.require(4); const v = this.buf.readUInt32BE(this.pos); this.pos += 4; return v; }
  u64() {
    this.require(8);
    const v = this.buf.readBigUInt64BE(this.pos);
    this.pos += 8;
    if (v > BigInt(Number.MAX_SAFE_INTEGER)) {
      fail(`${this.label}: length exceeds JS safe integer`, this.pos - 8, "UNSUPPORTED_DOCUMENT");
    }
    return Number(v);
  }
  ascii(len) { const b = this.bytes(len); return b.toString("latin1"); }
  bytes(len) {
    this.require(len);
    const b = this.buf.subarray(this.pos, this.pos + len);
    this.pos += len;
    return b;
  }
  skip(len) { this.require(len); this.pos += len; }
  child(len, label) {
    this.require(len, label);
    const c = new Cursor(this.buf, this.pos, this.pos + len, label);
    this.pos += len;
    return c;
  }
  pascal(padTo) {
    const start = this.pos;
    const len = this.u8();
    const raw = this.bytes(len);
    const padding = (padTo - ((this.pos - start) % padTo)) % padTo;
    this.skip(padding);
    return { name: raw.toString("latin1"), padding };
  }
}

function scan(buffer, options = {}) {
  if (!Buffer.isBuffer(buffer)) fail("input must be a Buffer", 0, "INVALID_ARGUMENT");
  if (buffer.length > (options.maxBytes ?? MAX_FILE)) {
    fail("PSD/PSB exceeds scanner input size limit", 0, "UNSUPPORTED_DOCUMENT");
  }
  const c = new Cursor(buffer);
  const state = {
    resources: [], tagged: [], layer_records: [], sections: [],
    unknown: [], risks: [], opaque_bytes: 0, block_count: 0,
  };
  function incBlock(offset) {
    state.block_count++;
    if (state.block_count > MAX_BLOCKS) fail("block count exceeds limit", offset, "UNSUPPORTED_DOCUMENT");
  }
  function addRisk(value) { if (!state.risks.includes(value)) state.risks.push(value); }
  function unknown(block) { state.unknown.push(block); addRisk("unknown_metadata"); }
  function section(parent, name, wide = false) {
    const field = parent.pos;
    const len = wide ? parent.u64() : parent.u32();
    const start = parent.pos;
    const sub = parent.child(len, name);
    state.sections.push({
      name, offset: start, length: len, length_field_offset: field,
      data_sha256: sha(parent.buf.subarray(start, start + len)),
    });
    return sub;
  }
  function recordOpaque(bytes) {
    state.opaque_bytes += bytes;
    addRisk("opaque_payload_not_proven_roundtrip_safe");
  }
  function tagRecords(cursor, scope, layerIndex = null) {
    while (cursor.left() > 0) {
      const offset = cursor.pos;
      // Some PSD/PSB writers put two zero alignment bytes before a tagged
      // block sequence. Record this noncanonical feature; never silently
      // certify a write based on our ability to step over the padding.
      if (cursor.left() >= 6 &&
          cursor.buf.readUInt16BE(cursor.pos) === 0 &&
          ["8BIM", "8B64"].includes(cursor.buf.toString("latin1", cursor.pos + 2, cursor.pos + 6))) {
        cursor.skip(2);
        addRisk("noncanonical_tagged_block_alignment");
      }
      incBlock(offset);
      const signature = cursor.ascii(4);
      if (signature !== "8BIM" && signature !== "8B64") {
        fail(`unexpected tagged block signature ${JSON.stringify(signature)} in ${scope}`, offset);
      }
      const key = cursor.ascii(4);
      const lenWidth = version === 2 && PSB_LONG_KEYS.has(key) ? 8 : 4;
      const len = lenWidth === 8 ? cursor.u64() : cursor.u32();
      const data = cursor.bytes(len);
      const padding = len % 2;
      cursor.skip(padding);
      const recognized = RECOGNIZED_LAYER_KEYS.has(key);
      const record = {
        scope, ...(layerIndex === null ? {} : { layer_index: layerIndex }),
        offset, signature, key, length_width: lenWidth, length: len,
        padding, recognized, data_sha256: sha(data),
      };
      state.tagged.push(record);
      recordOpaque(len);
      if (!recognized) unknown({ category: "tagged_block", scope, offset, key, signature });
      if (signature === "8B64") addRisk("8B64_signature_requires_fidelity_review");
      if (["Layr", "Lr16", "Lr32"].includes(key)) {
        addRisk("embedded_layer_records_not_recursively_decoded");
      }
    }
  }

  c.require(26, "PSD header");
  const signature = c.ascii(4);
  if (signature !== "8BPS") fail("invalid PSD signature", 0);
  const version = c.u16();
  if (![1, 2].includes(version)) fail("unsupported PSD/PSB version", 4, "UNSUPPORTED_DOCUMENT");
  const reserved = c.bytes(6);
  if (reserved.some(n => n !== 0)) fail("nonzero reserved PSD header bytes", 6);
  const channels = c.u16();
  const height = c.u32();
  const width = c.u32();
  const bit_depth = c.u16();
  const color_mode = c.u16();
  if (channels < 1 || channels > 56 || width === 0 || height === 0 ||
      !Number.isSafeInteger(width * height) || width * height > MAX_PIXELS) {
    fail("PSD header channel/geometry outside scanner limits", 12, "UNSUPPORTED_DOCUMENT");
  }
  if (![1, 8, 16, 32].includes(bit_depth)) {
    fail("unknown PSD bit depth", 22, "UNSUPPORTED_DOCUMENT");
  }
  const header = {
    signature, version, format: version === 1 ? "psd" : "psb",
    channels, width, height, bit_depth, color_mode,
  };
  if (bit_depth !== 8) addRisk("high_bit_depth_not_writable");
  if (color_mode !== 3) addRisk("non_rgb_not_writable");

  const color = section(c, "color_mode_data");
  if (color.left() > 0) recordOpaque(color.left());
  color.skip(color.left());

  const resources = section(c, "image_resources");
  while (resources.left() > 0) {
    const offset = resources.pos;
    incBlock(offset);
    const resourceSignature = resources.ascii(4);
    if (resourceSignature !== "8BIM") {
      fail("invalid Image Resources signature", offset);
    }
    const id = resources.u16();
    const { name } = resources.pascal(2);
    const len = resources.u32();
    const data = resources.bytes(len);
    resources.skip(len % 2);
    const recognized = RECOGNIZED_RESOURCE_IDS.has(id);
    const rec = { offset, id, name, length: len, recognized, data_sha256: sha(data) };
    state.resources.push(rec);
    recordOpaque(len);
    if (!recognized) unknown({ category: "image_resource", offset, id });
  }

  const layerMask = section(c, "layer_and_mask_info", version === 2);
  if (layerMask.left()) {
    const layerInfo = section(layerMask, "layer_info", version === 2);
    let channelDataLengths = [];
    if (layerInfo.left()) {
      const signedCount = layerInfo.i16();
      const count = Math.abs(signedCount);
      if (count > MAX_LAYERS) fail("PSD layer count exceeds scanner limit", layerInfo.pos - 2, "UNSUPPORTED_DOCUMENT");
      for (let i = 0; i < count; i++) {
        const recordOffset = layerInfo.pos;
        incBlock(recordOffset);
        const bounds = [layerInfo.i32(), layerInfo.i32(), layerInfo.i32(), layerInfo.i32()];
        const countChannels = layerInfo.u16();
        if (countChannels > MAX_CHANNELS) {
          fail("PSD layer channel count exceeds limit", layerInfo.pos - 2, "UNSUPPORTED_DOCUMENT");
        }
        const channelsList = [];
        for (let j = 0; j < countChannels; j++) {
          const id = layerInfo.i16();
          const length = version === 2 ? layerInfo.u64() : layerInfo.u32();
          const channel = { id, length };
          channelsList.push(channel);
          channelDataLengths.push(channel);
        }
        if (layerInfo.ascii(4) !== "8BIM") fail("invalid layer blend signature", layerInfo.pos - 4);
        const blend_mode = layerInfo.ascii(4);
        const opacity = layerInfo.u8();
        const clipping = layerInfo.u8();
        const flags = layerInfo.u8();
        layerInfo.skip(1);
        const extra = section(layerInfo, `layer_${i}_extra`);
        const mask = section(extra, `layer_${i}_mask_data`);
        recordOpaque(mask.left()); mask.skip(mask.left());
        const blend = section(extra, `layer_${i}_blending_ranges`);
        recordOpaque(blend.left()); blend.skip(blend.left());
        const name = extra.pascal(4).name;
        tagRecords(extra, "layer", i);
        state.layer_records.push({
          index: i, offset: recordOffset, bounds, channel_count: countChannels,
          channels: channelsList, blend_mode, opacity, clipping, flags,
          legacy_name: name,
        });
      }
      for (const channel of channelDataLengths) {
        layerInfo.require(channel.length, "layer channel pixel bytes");
        channel.data_offset = layerInfo.pos;
        channel.data_sha256 = sha(layerInfo.bytes(channel.length));
        recordOpaque(channel.length);
      }
      // PSD writers in the frozen corpus sometimes leave 1-3 trailing zero
      // bytes after channel records. Recognize, inventory and conservatively
      // flag such padding rather than treating those exact cases as opaque
      // unbounded channel data.
      const unused = layerInfo.left();
      if (unused > 0 && unused <= 3 &&
          layerInfo.buf.subarray(layerInfo.pos, layerInfo.end).every(b => b === 0)) {
        layerInfo.skip(unused);
        addRisk("noncanonical_layer_info_padding");
      }
      if (layerInfo.left()) fail("unexpected unused bytes in layer info", layerInfo.pos);
    }
    if (layerMask.left() >= 4) {
      const globalMask = section(layerMask, "global_layer_mask_data");
      recordOpaque(globalMask.left()); globalMask.skip(globalMask.left());
      tagRecords(layerMask, "document");
    } else if (layerMask.left() === 0) {
      addRisk("absent_global_layer_mask_length");
    } else {
      fail("truncated global layer mask length", layerMask.pos);
    }
  }

  // Image Data section is opaque compressed composite pixels; only verify
  // the compression header and inventory byte count. Never decode/rewrite.
  if (c.left() < 2) fail("PSD image data compression field missing", c.pos);
  const mergedCompression = c.u16();
  if (![0, 1, 2, 3].includes(mergedCompression)) {
    fail("unsupported merged image compression marker", c.pos - 2);
  }
  const mergedImage = {
    offset: c.pos - 2, bytes: c.left() + 2, compression: mergedCompression,
    data_sha256: sha(c.buf.subarray(c.pos - 2, c.end)),
  };
  recordOpaque(c.left());
  addRisk("merged_composite_payload_not_decoded");
  c.skip(c.left());
  const opaqueTotal = state.opaque_bytes;
  const summary = {
    image_resource_count: state.resources.length,
    layer_record_count: state.layer_records.length,
    tagged_block_count: state.tagged.length,
    unknown_block_count: state.unknown.length,
    opaque_payload_bytes: opaqueTotal,
  };
  return {
    schema_version: "1",
    operation: "psd.raw.inspect.research",
    status: "structurally_parsed",
    mutation_authorized: false,
    safe_to_rewrite: false,
    reason: "format inventory cannot prove opaque resource preservation or Photoshop fidelity",
    source: { bytes: buffer.length, sha256: sha(buffer) },
    header, sections: state.sections, merged_image: mergedImage,
    summary, image_resources: state.resources, layer_records: state.layer_records,
    tagged_blocks: state.tagged, unknown_blocks: state.unknown,
    risks: state.risks.sort(),
  };
}

function inspectFile(file) {
  const source = path.resolve(file);
  let st;
  try { st = fs.lstatSync(source); }
  catch { fail("PSD input cannot be inspected", 0); }
  if (!st.isFile()) fail("PSD input must be a regular file, not a symlink", 0, "UNSUPPORTED_DOCUMENT");
  if (st.size > MAX_FILE) fail("PSD input exceeds 8 MiB", 0, "UNSUPPORTED_DOCUMENT");
  const bytes = fs.readFileSync(source);
  const result = scan(bytes);
  return { ...result, input_path: source };
}

if (require.main === module) {
  try {
    if (process.argv.length !== 3) fail("usage: node m4c3_raw_block_inventory.cjs FILE", 0, "INVALID_ARGUMENT");
    const result = inspectFile(process.argv[2]);
    process.stdout.write(JSON.stringify({ status: "ok", inventory: result }) + "\n");
  } catch (error) {
    const failure = error instanceof ScanError ? error :
      new ScanError("EXECUTION_FAILED", String(error?.message || error), 0);
    process.stdout.write(JSON.stringify({
      status: "error", error: {
        code: failure.code, message: failure.message, offset: failure.offset,
      }
    }) + "\n");
    process.exitCode = failure.code === "INVALID_ARGUMENT" || failure.code === "INVALID_INPUT" ? 2 :
      failure.code === "UNSUPPORTED_DOCUMENT" ? 3 : 1;
  }
}
module.exports = { scan, inspectFile, ScanError };
