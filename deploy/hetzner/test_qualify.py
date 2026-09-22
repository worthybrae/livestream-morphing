import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

from qualify import evaluate, load_baseline


def sample(frame, buffer_count=10, total_ms=5000, paint_fps=14.985):
    return {"state": "live", "buffer_count": buffer_count, "engine": {
        "rendered_frames": frame, "total_ms": total_ms,
        "duration_seconds": 6.006, "paint_fps": paint_fps,
        "width": 1920, "height": 1080,
    }}


class QualificationTests(unittest.TestCase):
    def test_accepts_stable_full_hd_run(self):
        report = evaluate([sample(i * 180) for i in range(1, 11)], 14.9)
        self.assertTrue(report["passed"])

    def test_rejects_segment_slower_than_playback(self):
        report = evaluate([sample(180), sample(360, total_ms=6100)], 14.9)
        self.assertIn("segment_deadline", report["failed_gates"])

    def test_rejects_restart_or_falling_buffer(self):
        restarted = [sample(180), sample(20)]
        falling = [sample(i * 180, buffer_count=10-i) for i in range(1, 7)]
        self.assertIn("render_restart", evaluate(restarted, 14.9)["failed_gates"])
        self.assertIn("buffer_trend", evaluate(falling, 14.9)["failed_gates"])

    def test_ignores_repeated_polls_for_one_completed_segment(self):
        samples = [sample(180), sample(180, total_ms=99999), sample(360)]
        report = evaluate(samples, 14.9)
        self.assertEqual(report["segment_count"], 2)
        self.assertNotIn("segment_deadline", report["failed_gates"])

    def test_rejects_malformed_samples_with_named_gate(self):
        report = evaluate([sample(180), {"state": "live"}], 14.9)
        self.assertFalse(report["passed"])
        self.assertIn("invalid_sample", report["failed_gates"])

    def test_rejects_non_live_status_after_warmup(self):
        paused = sample(360)
        paused["state"] = "starting"
        self.assertIn("live_state", evaluate([sample(180), paused], 14.9)["failed_gates"])

    def test_baseline_requires_ten_unique_ordered_valid_segments(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            baseline = Path(temp_dir) / "baseline.json"
            baseline.write_text(json.dumps({
                "deployment_id": "railway-1",
                "samples": [
                    {"timestamp": f"2026-09-22T00:00:{i:02d}Z", "status": sample(i * 180, paint_fps=15 + i / 10)}
                    for i in range(1, 11)
                ],
            }))
            self.assertEqual(load_baseline(baseline), 15.1)

    def test_cli_writes_parseable_evidence_on_http_failure(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            baseline = root / "baseline.json"
            output = root / "samples.ndjson"
            baseline.write_text(json.dumps({
                "deployment_id": "railway-1",
                "samples": [
                    {"timestamp": f"2026-09-22T00:00:{i:02d}Z", "status": sample(i * 180)}
                    for i in range(1, 11)
                ],
            }))
            result = subprocess.run(
                [sys.executable, ROOT / "qualify.py", "--url", "http://127.0.0.1:1/api/live/status",
                 "--seconds", "0", "--baseline", baseline, "--output", output],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 1)
            report = json.loads(result.stdout)
            self.assertIn("http_failure", report["failed_gates"])
            rows = [json.loads(line) for line in output.read_text().splitlines()]
            self.assertEqual(len(rows), 1)
            self.assertIn("http_error", rows[0])


if __name__ == "__main__":
    unittest.main()
