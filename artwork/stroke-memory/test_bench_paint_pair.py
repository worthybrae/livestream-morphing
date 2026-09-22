import unittest

from bench_paint_pair import baseline_worker, run_order, summarize


class SummaryTests(unittest.TestCase):
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
