"""Read-only independent layer-kind profile for frozen PSD/PSB corpus.

Never saves Photoshop data or changes the fixture. Not a native-editor
or mutation-admission oracle.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys

from psd_tools import PSDImage


def inspect(filename: str) -> dict:
    source = Path(filename)
    if not source.is_file() or source.is_symlink() or source.stat().st_size > 8 * 1024 * 1024:
        raise ValueError("invalid or oversized research fixture")
    before = source.read_bytes()
    psd = PSDImage.open(source)
    layers = []
    for layer in psd.descendants():
        if len(layers) >= 64:
            raise ValueError("too many logical layers")
        layers.append({
            "id": f"L{len(layers)+1:04d}",
            "name": layer.name,
            "kind": layer.kind,
            "group": bool(layer.is_group()),
        })
    if source.read_bytes() != before:
        raise ValueError("fixture unexpectedly modified")
    return {
        "parser": "psd-tools 1.20.0",
        "source_sha256": hashlib.sha256(before).hexdigest(),
        "layers": layers,
    }


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: python m4c7e_psd_layer_profile.py SOURCE")
    try:
        print(json.dumps({"status": "ok", "profile": inspect(sys.argv[1])},
                         ensure_ascii=False))
    except Exception as error:
        print(json.dumps({"status": "error", "reason": str(error)}, ensure_ascii=False))
        sys.exit(2)
