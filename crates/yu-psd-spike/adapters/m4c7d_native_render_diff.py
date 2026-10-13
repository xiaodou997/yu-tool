"""M4c-7d compare Photoshop-exported PNGs with psd-tools PSD composites.

Only reads previously generated disposable PSD and PNG experiment files.
The PNG exports come from the actual Photoshop 2026 application, not
the psd-tools compositor. This is a single fixture, not a general oracle.
"""

from __future__ import annotations

import json
from pathlib import Path
import sys

import numpy as np
from PIL import Image
from psd_tools import PSDImage

from m4c7d_preview_diff import FILES, compare


def analyze(folder: Path) -> dict:
    native: dict[str, np.ndarray] = {}
    independent: dict[str, np.ndarray] = {}
    for label, filename in FILES.items():
        psd = folder / filename
        png = folder / filename.replace(".psd", ".native.png")
        if not png.is_file() or png.is_symlink() or png.stat().st_size > 8 * 1024 * 1024:
            raise ValueError("missing or unsafe Photoshop native PNG " + label)
        native[label] = np.asarray(Image.open(png).convert("RGBA"), dtype=np.uint8)
        doc = PSDImage.open(psd)
        composite = doc.composite()
        if composite is None:
            raise ValueError("no independent psd-tools composite " + label)
        independent[label] = np.asarray(composite.convert("RGBA"), dtype=np.uint8)
    output = {
        "native_source_vs_native_saved_control":
            compare(native["source"], native["control_saved"]),
        "native_patch_vs_native_saved_candidate":
            compare(native["patched_input"], native["candidate_saved"]),
        "native_saved_control_vs_native_saved_candidate":
            compare(native["control_saved"], native["candidate_saved"]),
        "independent_parser_vs_native_for_each_file": {
            key: compare(independent[key], native[key]) for key in FILES
        }
    }
    return output


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: python m4c7d_native_render_diff.py RUN_DIR")
    try:
        print(json.dumps(analyze(Path(sys.argv[1])), sort_keys=True))
    except Exception as error:
        print(json.dumps({"status": "error", "reason": str(error)}))
        sys.exit(2)
