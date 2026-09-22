"""Compare two complete Loaded-bristle workers on the same recorded frames.

The runner alternates execution order, excludes decoding and verification from
timings, and rejects any RGB or brush-state difference. It does not measure the
Rust socket, wave, or encoder.
"""

import argparse
import json
import os
import pathlib
import subprocess
import sys
import time
import types

import cv2
import numpy as np

from live import LoadedPainting


def summarize(samples):
    values = np.asarray(samples, np.float64)
    return {
        "mean": float(values.mean()),
        "median": float(np.median(values)),
        "p95": float(np.percentile(values, 95)),
    }


def run_order(frame):
    return (1, 0) if frame % 2 == 0 else (0, 1)


def baseline_worker(revision, directory=None):
    root = None
    if not directory:
        root = pathlib.Path(subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"],
            cwd=pathlib.Path(__file__).resolve().parent,
            text=True,
        ).strip())
    modules = {}
    saved = {name: sys.modules.get(name) for name in ("motion", "render", "adaptive", "live")}
    try:
        for name in ("motion", "render", "adaptive", "live"):
            relative = f"artwork/stroke-memory/{name}.py"
            if directory:
                source_path = pathlib.Path(directory) / f"{name}.py"
                source = source_path.read_bytes()
            else:
                source_path = root / relative
                source = subprocess.check_output(["git", "show", f"{revision}:{relative}"], cwd=root)
            module = types.ModuleType(f"baseline_{name}")
            module.__file__ = str(source_path)
            sys.modules[name] = module
            exec(compile(source, str(source_path), "exec"), module.__dict__)
            modules[name] = module
    finally:
        for name, module in saved.items():
            if module is None:
                sys.modules.pop(name, None)
            else:
                sys.modules[name] = module
    return modules["live"].LoadedPainting


def painted_pixels(result):
    return result[0] if isinstance(result, tuple) else result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("video")
    parser.add_argument("--baseline", default="HEAD")
    parser.add_argument("--baseline-dir")
    parser.add_argument("--baseline-native")
    parser.add_argument("--candidate-native")
    parser.add_argument("--frames", type=int, default=360)
    parser.add_argument("--warmup", type=int, default=60)
    parser.add_argument("--hour", type=float, default=15.0)
    args = parser.parse_args()
    if not 0 <= args.warmup < args.frames:
        parser.error("require 0 <= warmup < frames")

    capture = cv2.VideoCapture(args.video)
    if not capture.isOpened():
        parser.error("cannot open recorded video")
    width, height = int(capture.get(cv2.CAP_PROP_FRAME_WIDTH)), int(capture.get(cv2.CAP_PROP_FRAME_HEIGHT))
    original_native = os.environ.get("LOADED_RASTER")
    workers = []
    wall = [[], []]
    cpu = [[], []]
    stages = [[], []]
    try:
        factories = (baseline_worker(args.baseline, args.baseline_dir), LoadedPainting)
        for index, factory in enumerate(factories):
            selected = args.baseline_native if index == 0 else args.candidate_native
            if selected:
                os.environ["LOADED_RASTER"] = selected
            elif original_native:
                os.environ["LOADED_RASTER"] = original_native
            else:
                os.environ.pop("LOADED_RASTER", None)
            workers.append(factory(width, height))

        for frame in range(args.frames):
            ok, bgr = capture.read()
            if not ok:
                break
            rgb = cv2.cvtColor(bgr, cv2.COLOR_BGR2RGB)
            outputs = {}
            for index in run_order(frame):
                started_wall = time.perf_counter()
                started_cpu = time.process_time()
                outputs[index] = painted_pixels(workers[index].paint(rgb, args.hour)).copy()
                if frame >= args.warmup:
                    wall[index].append((time.perf_counter() - started_wall) * 1000.0)
                    cpu[index].append((time.process_time() - started_cpu) * 1000.0)
                    stages[index].append(dict(workers[index].cost))
            np.testing.assert_array_equal(outputs[0], outputs[1], err_msg=f"paint frame {frame}")
            np.testing.assert_array_equal(workers[0].book.ground, workers[1].book.ground,
                                          err_msg=f"ground marks {frame}")
            np.testing.assert_array_equal(workers[0].book.strokes, workers[1].book.strokes,
                                          err_msg=f"base marks {frame}")
            np.testing.assert_array_equal(workers[0].adaptive.strokes, workers[1].adaptive.strokes,
                                          err_msg=f"detail marks {frame}")
            if (frame + 1) % 60 == 0:
                print(json.dumps({"verified_frames": frame + 1,
                    "recent_mean_ms": [float(np.mean(values[-60:])) if values else None
                                       for values in wall]}), flush=True)

        if not wall[0]:
            raise ValueError("video ended before the measurement window")
        stage_summary = []
        for samples in stages:
            stage_summary.append({name: summarize([row[name] * 1000.0 for row in samples])
                                  for name in samples[0]})
        print(json.dumps({
            "resolution": [width, height],
            "measured_frames": len(wall[0]),
            "max_rgb_difference": 0,
            "marks": [len(workers[1].book.strokes), len(workers[1].adaptive.strokes)],
            "wall_ms": [summarize(values) for values in wall],
            "cpu_ms": [summarize(values) for values in cpu],
            "stage_ms": stage_summary,
        }), flush=True)
    finally:
        capture.release()
        for worker in workers:
            worker.pool.shutdown()
            worker.raster.close()
        if original_native:
            os.environ["LOADED_RASTER"] = original_native
        else:
            os.environ.pop("LOADED_RASTER", None)


if __name__ == "__main__":
    main()
