"""M4c-7d independent psd-tools pixel-level control/candidate comparison.

Read-only. This is a psd-tools composite interpretation, not a native
Photoshop pixel oracle. Never saves PSD files or changes source documents.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys

import numpy as np
from psd_tools import PSDImage

FILES = {
    "source": "nested-group-control.psd",
    "patched_input": "nested-group.psd",
    "control_saved": "nested-group-control-saved.psd",
    "candidate_saved": "nested-group-candidate-saved.psd",
}


def compare(left: np.ndarray, right: np.ndarray) -> dict:
    if left.shape != right.shape or left.ndim != 3 or left.shape[2] != 4:
        raise ValueError("RGBA dimensions mismatch")
    delta = np.abs(left.astype(np.int16) - right.astype(np.int16))
    changed = np.any(delta != 0, axis=2)
    count = int(np.count_nonzero(changed))
    y, x = np.nonzero(changed)
    samples = []
    for py, px in list(zip(y, x, strict=True))[:4]:
        samples.append({
            "x": int(px), "y": int(py),
            "before": left[py, px, :].tolist(),
            "after": right[py, px, :].tolist(),
        })
    return {
        "changed_pixels": count,
        "total_pixels": int(left.shape[0] * left.shape[1]),
        "changed_bbox_xyxy": (
            [int(x.min()), int(y.min()), int(x.max()) + 1, int(y.max()) + 1]
            if count else None
        ),
        "changed_channel_values": [int(np.count_nonzero(delta[:, :, ch]))
                                   for ch in range(4)],
        "max_channel_delta": int(delta.max()),
        "samples": samples,
    }


def audit(folder: Path) -> dict:
    arrays: dict[str, np.ndarray] = {}
    hashes: dict[str, str] = {}
    for label, filename in FILES.items():
        file = folder / filename
        if not file.is_file() or file.is_symlink() or file.stat().st_size > 8 * 1024 * 1024:
            raise ValueError("missing, symlink or oversized fixture " + label)
        doc = PSDImage.open(file)
        rgba = doc.composite()
        if rgba is None:
            raise ValueError("missing composite for " + label)
        rgba = rgba.convert("RGBA")
        if rgba.width * rgba.height > 2_000_000:
            raise ValueError("pixel budget exceeded")
        array = np.asarray(rgba, dtype=np.uint8)
        arrays[label] = array
        hashes[label] = hashlib.sha256(rgba.tobytes()).hexdigest()

    return {
        "preview_hashes": hashes,
        "source_vs_patched_input": compare(arrays["source"], arrays["patched_input"]),
        "source_vs_control_saved": compare(arrays["source"], arrays["control_saved"]),
        "patched_input_vs_candidate_saved": compare(arrays["patched_input"], arrays["candidate_saved"]),
        "control_saved_vs_candidate_saved": compare(arrays["control_saved"], arrays["candidate_saved"]),
    }


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: python m4c7d_preview_diff.py ISOLATED_RUN_DIRECTORY")
    try:
        print(json.dumps(audit(Path(sys.argv[1])), sort_keys=True))
    except Exception as exc:
        print(json.dumps({"status": "error", "message": str(exc)}))
        sys.exit(2)
