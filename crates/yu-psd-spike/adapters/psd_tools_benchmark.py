from __future__ import annotations

import argparse
import ctypes
import hashlib
import io
import json
import sys
import time
from pathlib import Path


UNAVAILABLE_EXIT = 3


def unavailable(message: str) -> int:
    print(message, file=sys.stderr)
    return UNAVAILABLE_EXIT


def peak_rss_bytes() -> int:
    if sys.platform == "win32":
        from ctypes import wintypes

        class PROCESS_MEMORY_COUNTERS(ctypes.Structure):
            _fields_ = [
                ("cb", wintypes.DWORD),
                ("PageFaultCount", wintypes.DWORD),
                ("PeakWorkingSetSize", ctypes.c_size_t),
                ("WorkingSetSize", ctypes.c_size_t),
                ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                ("PagefileUsage", ctypes.c_size_t),
                ("PeakPagefileUsage", ctypes.c_size_t),
            ]

        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        psapi = ctypes.WinDLL("psapi", use_last_error=True)

        get_current_process = kernel32.GetCurrentProcess
        get_current_process.argtypes = []
        get_current_process.restype = wintypes.HANDLE

        get_process_memory_info = psapi.GetProcessMemoryInfo
        get_process_memory_info.argtypes = [
            wintypes.HANDLE,
            ctypes.POINTER(PROCESS_MEMORY_COUNTERS),
            wintypes.DWORD,
        ]
        get_process_memory_info.restype = wintypes.BOOL

        counters = PROCESS_MEMORY_COUNTERS()
        counters.cb = ctypes.sizeof(counters)
        ok = get_process_memory_info(
            get_current_process(), ctypes.byref(counters), counters.cb
        )
        if not ok:
            raise ctypes.WinError(ctypes.get_last_error())
        return int(counters.PeakWorkingSetSize)

    import resource

    value = int(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss)
    return value if sys.platform == "darwin" else value * 1024


def iter_layers(container):
    for layer in container:
        yield layer
        if layer.is_group():
            yield from iter_layers(layer)


def main() -> int:
    parser = argparse.ArgumentParser(description="YuTool psd-tools benchmark adapter")
    parser.add_argument("--expected-version", required=True)
    parser.add_argument("--expected-python", required=True)
    parser.add_argument("--warmup", type=int, required=True)
    parser.add_argument("--iterations", type=int, required=True)
    parser.add_argument("input", type=Path)
    args = parser.parse_args()

    actual_python = f"{sys.version_info.major}.{sys.version_info.minor}"
    if actual_python != args.expected_python:
        return unavailable(
            f"Python version mismatch: expected {args.expected_python}, got {actual_python}"
        )

    try:
        from psd_tools import PSDImage
        from psd_tools.version import __version__ as psd_tools_version
    except Exception as error:
        return unavailable(f"psd-tools is unavailable: {error}")

    if psd_tools_version != args.expected_version:
        return unavailable(
            f"psd-tools version mismatch: expected {args.expected_version}, got {psd_tools_version}"
        )

    data = args.input.read_bytes()

    def parse_once() -> None:
        psd = PSDImage.open(io.BytesIO(data))
        sum(1 for _ in iter_layers(psd))

    def export_once():
        psd = PSDImage.open(io.BytesIO(data))
        started = time.perf_counter_ns()
        digest = hashlib.sha256()
        exported_layer_count = 0
        total_rgba_bytes = 0

        for layer in iter_layers(psd):
            image = layer.topil(apply_icc=False)
            if image is None:
                continue
            raw = image.convert("RGBA").tobytes()
            digest.update(raw)
            exported_layer_count += 1
            total_rgba_bytes += len(raw)

        elapsed_ms = (time.perf_counter_ns() - started) / 1_000_000.0
        return elapsed_ms, exported_layer_count, total_rgba_bytes, digest.hexdigest()

    for _ in range(args.warmup):
        parse_once()
    warm_parse_samples_ms = []
    for _ in range(args.iterations):
        started = time.perf_counter_ns()
        parse_once()
        warm_parse_samples_ms.append(
            (time.perf_counter_ns() - started) / 1_000_000.0
        )

    for _ in range(args.warmup):
        export_once()

    layer_export_samples_ms = []
    fingerprint = None
    for _ in range(args.iterations):
        elapsed_ms, layer_count, byte_count, checksum = export_once()
        current = (layer_count, byte_count, checksum)
        if fingerprint is None:
            fingerprint = current
        elif fingerprint != current:
            raise RuntimeError("psd-tools layer export fingerprint changed between iterations")
        layer_export_samples_ms.append(elapsed_ms)

    if fingerprint is None:
        raise RuntimeError("psd-tools benchmark produced no export fingerprint")

    layer_count, byte_count, checksum = fingerprint
    print(
        json.dumps(
            {
                "warm_parse_samples_ms": warm_parse_samples_ms,
                "layer_export_samples_ms": layer_export_samples_ms,
                "exported_layer_count": layer_count,
                "total_rgba_bytes": byte_count,
                "export_checksum_sha256": checksum,
                "peak_rss_bytes": peak_rss_bytes(),
            },
            separators=(",", ":"),
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
