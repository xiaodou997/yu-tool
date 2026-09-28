"use strict";

const fs = require("node:fs");
const { crc32, deflateSync } = require("node:zlib");
const MAX_RGBA_BYTES = 256 * 1024 * 1024;
const MAX_PNG_BYTES = 320 * 1024 * 1024;


const PROTOCOL_VERSION = "1";
const CONTRACT_VERSION = "1";
const EXPECTED_NODE_MAJOR = "22";
const EXPECTED_AG_PSD_VERSION = "31.0.2";

const SUPPORTED_CAPABILITIES = new Set([
  "psd.inspect",
  "psd.tree",
  "psd.layer.list",
  "psd.layer.info",
  "psd.layer.export",
]);

function transportFailure(message) {
  process.stderr.write(String(message) + "\n");
  process.exit(2);
}

function responseOk(requestId, result, warnings = []) {
  return {
    protocol_version: PROTOCOL_VERSION,
    request_id: requestId,
    status: "ok",
    result,
    warnings,
  };
}

function responseError(requestId, code, message, warnings = []) {
  return {
    protocol_version: PROTOCOL_VERSION,
    request_id: requestId,
    status: "error",
    error: { code, message },
    warnings,
  };
}

function validateHeader(request) {
  if (!request || typeof request !== "object" || Array.isArray(request)) {
    transportFailure("external engine request must be a JSON object");
  }
  if (request.protocol_version !== PROTOCOL_VERSION) {
    transportFailure(
      `unsupported external engine protocol version: ${String(request.protocol_version)}`
    );
  }
  if (
    typeof request.request_id !== "string" ||
    !/^[A-Za-z0-9._:-]{1,128}$/.test(request.request_id)
  ) {
    transportFailure("invalid external engine request_id");
  }
  if (
    typeof request.capability !== "string" ||
    !/^[a-z0-9._-]{1,128}$/.test(request.capability)
  ) {
    transportFailure("invalid external engine capability");
  }
  if (!Object.prototype.hasOwnProperty.call(request, "payload")) {
    transportFailure("external engine request payload is required");
  }
}

function requireInputPath(payload) {
  if (
    !payload ||
    typeof payload !== "object" ||
    typeof payload.input_path !== "string" ||
    payload.input_path.length === 0
  ) {
    throw {
      code: "INVALID_ARGUMENT",
      message: "payload.input_path must be a non-empty string",
    };
  }
  return payload.input_path;
}

function loadPinnedAgPsd() {
  const actualNodeMajor = process.versions.node.split(".")[0];
  if (actualNodeMajor !== EXPECTED_NODE_MAJOR) {
    throw {
      code: "ENGINE_INCOMPATIBLE",
      message: `Node.js major mismatch: expected ${EXPECTED_NODE_MAJOR}, got ${process.versions.node}`,
    };
  }

  try {
    const api = require("ag-psd");
    const version = require("ag-psd/package.json").version;
    if (version !== EXPECTED_AG_PSD_VERSION) {
      throw {
        code: "ENGINE_INCOMPATIBLE",
        message: `ag-psd version mismatch: expected ${EXPECTED_AG_PSD_VERSION}, got ${version}`,
      };
    }
    return api;
  } catch (error) {
    if (error && error.code === "ENGINE_INCOMPATIBLE") throw error;
    throw {
      code: "ENGINE_INCOMPATIBLE",
      message: `ag-psd runtime is unavailable: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
}

function readDocument(inputPath, exportPixels = false) {
  let buffer;
  try {
    if (exportPixels && fs.statSync(inputPath).size > 512 * 1024 * 1024) {
      throw new Error("PSD export input exceeds 512 MiB");
    }
    buffer = fs.readFileSync(inputPath);
  } catch (error) {
    throw {
      code: "INVALID_INPUT",
      message: `cannot read PSD input: ${error instanceof Error ? error.message : String(error)}`,
    };
  }

  if (
    buffer.length < 6 ||
    buffer.toString("ascii", 0, 4) !== "8BPS"
  ) {
    throw {
      code: "INVALID_INPUT",
      message: "input is not a PSD/PSB document",
    };
  }

  const version = buffer.readUInt16BE(4);
  const format = version === 1 ? "psd" : version === 2 ? "psb" : null;
  if (!format) {
    throw {
      code: "INVALID_INPUT",
      message: `unsupported PSD header version: ${version}`,
    };
  }

  if (exportPixels) {
    if (buffer.length < 26) throw { code: "INVALID_INPUT", message: "truncated PSD header" };
    if (buffer.readUInt16BE(22) !== 8 || buffer.readUInt16BE(24) !== 3) {
      throw { code: "UNSUPPORTED_CAPABILITY", message: "layer export currently requires an 8-bit RGB PSD/PSB; high-bit and other color modes are unsupported" };
    }
  }
  const { readPsd, initializeCanvas } = loadPinnedAgPsd();
  if (exportPixels) initializeCanvas(
    () => { throw new Error("canvas rendering is not part of layer bitmap export"); },
    (width, height) => {
      checkedPixelBytes(width, height);
      return { width, height, data: new Uint8ClampedArray(width * height * 4) };
    }
  );

  try {
    const psd = readPsd(buffer, {
      skipLayerImageData: !exportPixels,
      useRawData: exportPixels,
      skipCompositeImageData: true,
      skipThumbnail: true,
      skipLinkedFilesData: true,
      logMissingFeatures: false,
    });
    return { psd, format };
  } catch (error) {
    throw {
      code: "INVALID_INPUT",
      message: `ag-psd rejected input: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
}

function checkedPixelBytes(width, height) {
  const bytes = width * height * 4;
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width <= 0 || height <= 0 ||
      width > 0x7fffffff || height > 0x7fffffff || !Number.isSafeInteger(bytes) || bytes > MAX_RGBA_BYTES) {
    throw { code: "UNSUPPORTED_CAPABILITY", message: "layer bitmap is empty or exceeds the 256 MiB RGBA8 limit" };
  }
  return bytes;
}

// PNG color type 6, depth 8, filter 0, zlib-wrapped DEFLATE; no canvas dependency.
// https://www.w3.org/TR/png-3/#11IHDR and #5CRC-algorithm
function encodePng(image) {
  const { width, height, data } = image;
  const bytes = checkedPixelBytes(width, height);
  if (!(data instanceof Uint8Array || data instanceof Uint8ClampedArray) || data.byteLength !== bytes) {
    throw new Error("engine did not materialize RGBA8 pixels");
  }
  const rgba = Buffer.from(data.buffer, data.byteOffset, data.byteLength);
  const stride = width * 4;
  const rows = Buffer.alloc(bytes + height);
  for (let y = 0; y < height; y++) rgba.copy(rows, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
  const chunk = (type, payload) => {
    const name = Buffer.from(type, "ascii");
    const header = Buffer.alloc(8);
    header.writeUInt32BE(payload.length, 0);
    name.copy(header, 4);
    const checksum = Buffer.alloc(4);
    checksum.writeUInt32BE(crc32(payload, crc32(name)), 0);
    return Buffer.concat([header, payload, checksum]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(rows, { level: 6, maxOutputLength: MAX_PNG_BYTES - 57 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

function exportLayer(request, psd) {
  const { layer_id: id, output_path: output } = request.payload;
  const index = typeof id === "string" && /^L\d{4,}$/.test(id) ? Number(id.slice(1)) : NaN;
  if (!Number.isSafeInteger(index) || index <= 0 || makeLayerId(index) !== id) {
    return responseError(request.request_id, "INVALID_ARGUMENT", "layer_id must be a canonical PSD layer ID");
  }
  if (typeof output !== "string" || !output || output.includes("\0")) {
    return responseError(request.request_id, "INVALID_ARGUMENT", "output_path must be a non-empty path");
  }
  const stack = [...(psd.children || [])].reverse();
  let selected;
  let current = 0;
  while (stack.length) {
    const layer = stack.pop();
    if (++current === index) { selected = layer; break; }
    if (Array.isArray(layer.children)) stack.push(...[...layer.children].reverse());
  }
  if (!selected) return responseError(request.request_id, "INVALID_ARGUMENT", `PSD layer ID was not found: ${id}`);
  if (Array.isArray(selected.children) || !selected.rawData ||
      ![0, 1, 2].every(id => selected.rawData.channels.some(channel => channel.id === id && channel.data && channel.data.byteLength))) {
    return responseError(request.request_id, "UNSUPPORTED_CAPABILITY", "selected layer has no independent RGB bitmap; groups are not composited");
  }
  try {
    checkedPixelBytes(selected.right - selected.left, selected.bottom - selected.top);
    const image = loadPinnedAgPsd().getLayerImageData(selected);
    if (!image) return responseError(request.request_id, "UNSUPPORTED_CAPABILITY", "selected layer bitmap is unavailable");
    const png = encodePng(image);
    // The Rust host supplies a private staging path, never the user's final destination.
    fs.writeFileSync(output, png, { flag: "wx", mode: 0o600 });
    return responseOk(request.request_id, {
      contract_version: CONTRACT_VERSION, layer_id: id, output_path: output,
      width: image.width, height: image.height, pixel_format: "rgba8", container: "png",
    }, ["Exports the stored layer bitmap only: masks, opacity, blend modes, effects, group composition and ICC color conversion are not applied; cached text/shape/Smart Object pixels are not re-rendered."]);
  } catch (error) {
    return responseError(request.request_id,
      error.code === "UNSUPPORTED_CAPABILITY" ? error.code : "EXECUTION_FAILED",
      error.message || String(error));
  }
}

function colorModeName(value) {
  switch (value) {
    case 0:
      return "bitmap";
    case 1:
      return "grayscale";
    case 2:
      return "indexed";
    case 3:
      return "rgb";
    case 4:
      return "cmyk";
    case 7:
      return "multichannel";
    case 8:
      return "duotone";
    case 9:
      return "lab";
    default:
      return "unknown";
  }
}

function layerKind(layer) {
  if (Array.isArray(layer.children)) return "group";
  if (layer.text) return "text";
  if (layer.placedLayer) return "smart_object";
  if (layer.vectorFill || layer.vectorStroke) return "shape";
  return "pixel";
}

function layerBounds(layer) {
  const values = [layer.top, layer.left, layer.bottom, layer.right];
  if (!values.every((value) => Number.isFinite(value))) return undefined;

  return {
    top: Math.trunc(layer.top),
    left: Math.trunc(layer.left),
    bottom: Math.trunc(layer.bottom),
    right: Math.trunc(layer.right),
  };
}

function hasPixelMask(layer) {
  return Boolean(
    (layer.mask && layer.mask.fromVectorData !== true) ||
      (layer.realMask && layer.realMask.fromVectorData !== true)
  );
}

function makeLayerId(index) {
  const digits = String(index);
  return "L" + digits.padStart(4, "0");
}

function buildCanonicalLayers(psd) {
  let nextIndex = 1;
  let maximumTreeDepth = 0;
  const flat = [];

  function visit(children, parentId, depth) {
    if (!Array.isArray(children)) return [];

    return children.map((layer) => {
      const id = makeLayerId(nextIndex++);
      const childSource = Array.isArray(layer.children) ? layer.children : [];
      maximumTreeDepth = Math.max(maximumTreeDepth, depth);

      const summary = {
        id,
        ...(parentId ? { parent_id: parentId } : {}),
        depth,
        name: typeof layer.name === "string" ? layer.name : "",
        kind: layerKind(layer),
        visible: layer.hidden !== true,
        ...(layerBounds(layer) ? { bounds: layerBounds(layer) } : {}),
        has_pixel_mask: hasPixelMask(layer),
        has_vector_mask: Boolean(layer.vectorMask),
        child_count: childSource.length,
      };

      flat.push(summary);
      const node = {
        ...summary,
        children: visit(childSource, id, depth + 1),
      };
      return node;
    });
  }

  const tree = visit(psd.children, null, 1);
  return { tree, flat, maximumTreeDepth };
}

function documentInfo(psd, format, canonical) {
  return {
    format,
    width: Number(psd.width),
    height: Number(psd.height),
    channels: Number(psd.channels || 0),
    bits_per_channel: Number(psd.bitsPerChannel || 8),
    color_mode: colorModeName(psd.colorMode),
    layer_count: canonical.flat.length,
    maximum_tree_depth: canonical.maximumTreeDepth,
  };
}

function handleRequest(request) {
  if (!SUPPORTED_CAPABILITIES.has(request.capability)) {
    return responseError(
      request.request_id,
      "UNSUPPORTED_CAPABILITY",
      `capability is not implemented by protocol adapter v1: ${request.capability}`
    );
  }

  let inputPath;
  try {
    inputPath = requireInputPath(request.payload);
  } catch (error) {
    return responseError(
      request.request_id,
      error.code || "INVALID_ARGUMENT",
      error.message || String(error)
    );
  }

  let loaded;
  try {
    loaded = readDocument(inputPath, request.capability === "psd.layer.export");
  } catch (error) {
    return responseError(
      request.request_id,
      error.code || "EXECUTION_FAILED",
      error.message || String(error)
    );
  }

  if (request.capability === "psd.layer.export") return exportLayer(request, loaded.psd);

  const canonical = buildCanonicalLayers(loaded.psd);
  const document = documentInfo(loaded.psd, loaded.format, canonical);
  const base = {
    contract_version: CONTRACT_VERSION,
    document,
  };

  switch (request.capability) {
    case "psd.inspect":
      return responseOk(request.request_id, base);

    case "psd.tree":
      return responseOk(request.request_id, {
        ...base,
        layers: canonical.tree,
      });

    case "psd.layer.list":
      return responseOk(request.request_id, {
        ...base,
        layers: canonical.flat,
      });

    case "psd.layer.info": {
      const layerId = request.payload.layer_id;
      const layerIndex =
        typeof layerId === "string" && /^L\d{4,}$/.test(layerId)
          ? Number(layerId.slice(1))
          : NaN;
      if (
        !Number.isSafeInteger(layerIndex) ||
        layerIndex <= 0 ||
        makeLayerId(layerIndex) !== layerId
      ) {
        return responseError(
          request.request_id,
          "INVALID_ARGUMENT",
          "payload.layer_id must be a canonical PSD layer ID"
        );
      }

      const layer = canonical.flat.find((item) => item.id === layerId);
      if (!layer) {
        return responseError(
          request.request_id,
          "INVALID_ARGUMENT",
          `PSD layer ID was not found: ${layerId}`
        );
      }

      return responseOk(request.request_id, {
        ...base,
        layer,
      });
    }

    default:
      return responseError(
        request.request_id,
        "UNSUPPORTED_CAPABILITY",
        `unsupported capability: ${request.capability}`
      );
  }
}

let raw = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => {
  raw += chunk;
  if (raw.length > 4 * 1024 * 1024) {
    transportFailure("external engine request exceeds 4 MiB");
  }
});
process.stdin.on("end", () => {
  let request;
  try {
    request = JSON.parse(raw);
  } catch (error) {
    transportFailure(
      `external engine request is not valid JSON: ${error instanceof Error ? error.message : String(error)}`
    );
  }

  validateHeader(request);

  let response;
  try {
    response = handleRequest(request);
  } catch (error) {
    response = responseError(
      request.request_id,
      "EXECUTION_FAILED",
      error instanceof Error ? error.message : String(error)
    );
  }

  process.stdout.write(JSON.stringify(response));
});
