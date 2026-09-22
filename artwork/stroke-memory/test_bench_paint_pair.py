import unittest

from bench_paint_pair import baseline_worker, run_order, summarize, summarize_telemetry


class SummaryTests(unittest.TestCase):
    def test_telemetry_summary_separates_planner_rate_stage_means_and_peak_rss(self):
        rows = [dict(planner=True, mutated_pixels=10, transport_ms=1., planner_ms=3.,
                     dirty_ms=4., worker_cpu_ms=8., full_raster_ms=5., worker_rss_bytes=100),
                dict(planner=False, mutated_pixels=6, transport_ms=2., planner_ms=0.,
                     dirty_ms=2., worker_cpu_ms=6., full_raster_ms=0., worker_rss_bytes=120)]

        result = summarize_telemetry(rows)

        self.assertEqual(result['planner_updates'], 1)
        self.assertEqual(result['mutated_pixels'], 16)
        self.assertEqual(result['worker_rss_bytes'], 120)
        self.assertEqual(result['mean_ms']['dirty_ms'], 3.)

    def test_summary_reports_mean_median_and_linear_p95(self):
        result = summarize([10.0, 20.0, 30.0, 40.0, 50.0])

        self.assertEqual(result, {"mean": 30.0, "median": 30.0, "p95": 48.0})

    def test_run_order_alternates_which_worker_goes_first(self):
        self.assertEqual(run_order(0), (1, 0))
        self.assertEqual(run_order(1), (0, 1))
        self.assertEqual(run_order(2), (1, 0))

    def test_directory_baseline_does_not_require_repository_parent_depth(self):
        with self.assertRaises(FileNotFoundError):
            baseline_worker("HEAD", "/missing-paint-worker")


if __name__ == "__main__":
    unittest.main()
