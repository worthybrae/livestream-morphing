#!/usr/bin/env python3
"""Collect and evaluate the private painter's one-hour qualification evidence."""

import argparse
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import sys
import time
from typing import Any
from urllib import error, request


DEFAULT_URL = "http://127.0.0.1:18080/api/live/status"
DEFAULT_SECONDS = 3600.0
DEFAULT_INTERVAL = 6.0
DEFAULT_OUTPUT = Path("qualification-samples.ndjson")
MINIMUM_UNIQUE_SEGMENTS = 10
WARMUP_POLLS = 1


class BaselineError(ValueError):
    """A Railway baseline cannot safely supply a qualification threshold."""


def is_number(value: Any, *, positive: bool = False) -> bool:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return False
    return math.isfinite(value) and (value > 0 if positive else value >= 0)


def status_values(status: Any, *, require_positive_buffer: bool = False) -> dict[str, float]:
    """Validate the public status contract and return its gate-relevant values."""
    if not isinstance(status, dict):
        raise ValueError("status must be an object")
    engine = status.get("engine")
    if not isinstance(engine, dict):
        raise ValueError("engine must be an object")
    if not isinstance(status.get("state"), str):
        raise ValueError("state must be a string")

    buffer_count = status.get("buffer_count")
    if not is_number(buffer_count, positive=require_positive_buffer):
        raise ValueError("buffer_count must be a finite number")

    fields = {
        "rendered_frames": engine.get("rendered_frames"),
        "total_ms": engine.get("total_ms"),
        "duration_seconds": engine.get("duration_seconds"),
        "paint_fps": engine.get("paint_fps"),
        "width": engine.get("width"),
        "height": engine.get("height"),
    }
    if not all(is_number(value, positive=True) for value in fields.values()):
        raise ValueError("engine metrics must be positive finite numbers")
    if int(fields["rendered_frames"]) != fields["rendered_frames"]:
        raise ValueError("rendered_frames must be an integer")
    if int(fields["width"]) != fields["width"] or int(fields["height"]) != fields["height"]:
        raise ValueError("dimensions must be integers")
    return {"buffer_count": float(buffer_count), **{name: float(value) for name, value in fields.items()}}


def empty_report(minimum_paint_fps: float) -> dict[str, Any]:
    return {
        "passed": False,
        "failed_gates": [],
        "segment_count": 0,
        "minimum_paint_fps": minimum_paint_fps,
        "observed_minimum_paint_fps": None,
        "maximum_segment_milliseconds": None,
        "minimum_buffer": None,
        "first_rendered_frame": None,
        "last_rendered_frame": None,
    }


def evaluate(samples: list[dict], minimum_paint_fps: float) -> dict:
    """Evaluate status samples without time or network dependencies.

    A repeated rendered frame is a repeated poll, not another completed segment.
    It is validated but excluded from all segment statistics and gates.
    """
    report = empty_report(minimum_paint_fps)
    failed: set[str] = set()
    if not is_number(minimum_paint_fps, positive=True):
        report["failed_gates"] = ["minimum_paint_fps_invalid"]
        return report
    if not isinstance(samples, list):
        report["failed_gates"] = ["invalid_sample"]
        return report

    polls: list[tuple[dict, dict[str, float]]] = []
    segments: list[tuple[dict, dict[str, float]]] = []
    last_rendered_frame: int | None = None
    seen_frames: set[int] = set()
    for status in samples:
        try:
            values = status_values(status)
        except ValueError:
            failed.add("invalid_sample")
            continue
        frame = int(values["rendered_frames"])
        if last_rendered_frame is not None and frame < last_rendered_frame:
            failed.add("render_restart")
        last_rendered_frame = frame
        polls.append((status, values))
        if frame in seen_frames:
            continue
        seen_frames.add(frame)
        segments.append((status, values))

    if not segments:
        failed.add("no_samples")
    else:
        report["segment_count"] = len(segments)
        report["observed_minimum_paint_fps"] = min(values["paint_fps"] for _, values in segments)
        report["maximum_segment_milliseconds"] = max(values["total_ms"] for _, values in segments)
        report["minimum_buffer"] = min(values["buffer_count"] for _, values in segments)
        report["first_rendered_frame"] = int(segments[0][1]["rendered_frames"])
        report["last_rendered_frame"] = int(segments[-1][1]["rendered_frames"])
        if len(segments) < MINIMUM_UNIQUE_SEGMENTS:
            failed.add("minimum_progress")

        for index, (status, values) in enumerate(polls):
            if index >= WARMUP_POLLS and status["state"] != "live":
                failed.add("live_state")
            if values["width"] != 1920 or values["height"] != 1080:
                failed.add("full_hd")
            if values["paint_fps"] < minimum_paint_fps:
                failed.add("paint_fps")

        for _, values in segments:
            if values["total_ms"] > values["duration_seconds"] * 1000:
                failed.add("segment_deadline")

        buffers = [values["buffer_count"] for _, values in polls]
        for start in range(len(buffers) - 5):
            window = buffers[start:start + 6]
            mean_index = 2.5
            mean_buffer = sum(window) / len(window)
            slope = sum((index - mean_index) * (value - mean_buffer) for index, value in enumerate(window)) / 17.5
            declining_steps = sum(current < previous for previous, current in zip(window, window[1:]))
            if slope < 0 and window[-1] < window[0] and declining_steps >= 3:
                failed.add("buffer_trend")
                break

    report["failed_gates"] = sorted(failed)
    report["passed"] = not report["failed_gates"]
    return report


def parse_rfc3339(value: Any) -> datetime:
    if not isinstance(value, str):
        raise BaselineError("timestamp must be an RFC3339 string")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise BaselineError("timestamp must be RFC3339") from exc
    if parsed.tzinfo is None:
        raise BaselineError("timestamp must include a timezone")
    return parsed


def load_baseline(path: Path) -> float:
    """Return a fail-closed threshold from a strict paired Railway baseline."""
    try:
        document = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise BaselineError(f"cannot read baseline: {exc}") from exc
    if not isinstance(document, dict) or not isinstance(document.get("deployment_id"), str) or not document["deployment_id"].strip():
        raise BaselineError("baseline must contain a non-empty deployment_id")
    samples = document.get("samples")
    if not isinstance(samples, list):
        raise BaselineError("baseline must contain a samples list")

    previous_timestamp: datetime | None = None
    previous_frame: int | None = None
    seen_frames: set[int] = set()
    paint_fps: list[float] = []
    for item in samples:
        if not isinstance(item, dict) or set(item) != {"timestamp", "status"}:
            raise BaselineError("each baseline sample must contain only timestamp and status")
        timestamp = parse_rfc3339(item["timestamp"])
        if previous_timestamp is not None and timestamp <= previous_timestamp:
            raise BaselineError("baseline timestamps must increase")
        try:
            values = status_values(item["status"], require_positive_buffer=True)
        except ValueError as exc:
            raise BaselineError(f"baseline status is invalid: {exc}") from exc
        frame = int(values["rendered_frames"])
        if previous_frame is not None and frame < previous_frame:
            raise BaselineError("baseline rendered_frames must increase")
        previous_timestamp = timestamp
        previous_frame = frame
        if frame in seen_frames:
            continue
        seen_frames.add(frame)
        paint_fps.append(values["paint_fps"])

    if len(paint_fps) < MINIMUM_UNIQUE_SEGMENTS:
        raise BaselineError("baseline requires at least 10 unique valid segments")
    return min(paint_fps)


def timestamp_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def append_evidence(handle, row: dict[str, Any]) -> None:
    handle.write(json.dumps(row, separators=(",", ":"), allow_nan=False) + "\n")
    handle.flush()
    os.fsync(handle.fileno())


def fetch_status(url: str) -> dict:
    with request.urlopen(url, timeout=5) as response:
        if response.status != 200:
            raise error.HTTPError(url, response.status, "unexpected HTTP status", response.headers, None)
        payload = json.loads(response.read().decode("utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("status response must be a JSON object")
    return payload


def add_failure(report: dict[str, Any], gate: str) -> None:
    report["failed_gates"] = sorted(set(report["failed_gates"]) | {gate})
    report["passed"] = False


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", default=DEFAULT_URL)
    parser.add_argument("--seconds", type=float, default=DEFAULT_SECONDS)
    parser.add_argument("--interval", type=float, default=DEFAULT_INTERVAL)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    if args.seconds < 0 or args.interval <= 0:
        parser.error("--seconds must be non-negative and --interval must be positive")

    try:
        minimum_paint_fps = load_baseline(args.baseline)
    except BaselineError as exc:
        report = empty_report(None)
        report["failed_gates"] = ["baseline_invalid"]
        report["error"] = str(exc)
        print(json.dumps(report, sort_keys=True))
        return 1

    args.output.parent.mkdir(parents=True, exist_ok=True)
    samples: list[dict] = []
    http_failure_count = 0
    deadline = time.monotonic() + args.seconds
    with args.output.open("w", encoding="utf-8") as evidence:
        while True:
            timestamp = timestamp_now()
            try:
                status = fetch_status(args.url)
            except (OSError, ValueError, json.JSONDecodeError) as exc:
                http_failure_count += 1
                append_evidence(evidence, {"timestamp": timestamp, "http_error": str(exc)})
            else:
                samples.append(status)
                append_evidence(evidence, {"timestamp": timestamp, "status": status})
            if time.monotonic() >= deadline:
                break
            time.sleep(min(args.interval, max(0, deadline - time.monotonic())))

    report = evaluate(samples, minimum_paint_fps)
    report["http_failure_count"] = http_failure_count
    if http_failure_count:
        add_failure(report, "http_failure")
    print(json.dumps(report, sort_keys=True, allow_nan=False))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
