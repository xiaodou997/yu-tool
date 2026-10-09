"""M4c-6 independent, strictly read-only PSD/PSB layer-name reference.

Use psd-tools 1.20.0 in an isolated Python 3.12 environment. Do not save.
Outputs canonical logical preorder names, without Photoshop-specific promises.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys


def read_document(filename: str) -> dict:
    from psd_tools import PSDImage

    path = Path(filename)
    data = path.read_bytes()
    if len(data) > 8 * 1024 * 1024:
        raise ValueError("M4c-6 fixture exceeds 8 MiB")

    document = PSDImage.open(path)
    layers = []

    def walk(group, depth: int) -> None:
        if depth > 32:
            raise ValueError("layer nesting too deep")
        for layer in group:
            if len(layers) >= 64:
                raise ValueError("too many logical layers")
            layers.append(
                {
                    "id": f"L{len(layers) + 1:04d}",
                    "name": layer.name,
                    "kind": "group" if layer.is_group() else layer.kind,
                }
            )
            if layer.is_group():
                walk(layer, depth + 1)

    walk(document, 1)
    preview = {"status": "unavailable", "rgba_sha256": None}
    try:
        composite = document.composite()
        if composite is not None:
            rgba = composite.convert("RGBA")
            preview = {
                "status": "rendered",
                "rgba_sha256": hashlib.sha256(rgba.tobytes()).hexdigest(),
                "width": rgba.width,
                "height": rgba.height,
            }
    except Exception as exc:
        # This independent preview is not Photoshop. A failed preview
        # must not be mistaken for equivalence or certification.
        preview = {"status": "failed", "reason": type(exc).__name__}

    return {
        "parser": "psd-tools",
        "parser_version": "1.20.0",
        "sha256": hashlib.sha256(data).hexdigest(),
        "width": document.width,
        "height": document.height,
        "layers": layers,
        "independent_preview": preview,
    }


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: python m4c6_psd_tools_reader.py FILE")
    try:
        print(json.dumps({"status": "ok", "document": read_document(sys.argv[1])},
                         ensure_ascii=False, sort_keys=True))
    except Exception as exc:
        print(json.dumps({"status": "error", "message": str(exc)}, ensure_ascii=False))
        sys.exit(2)
