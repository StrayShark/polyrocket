"""Dedicated tests for the v0.10b train_job + promote_model.

These tests use a tmp model dir so they don't touch the user's
real `~/.polyrocket/sidecar/models/`.
"""

import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

# Make the package importable
HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(ROOT))

import polyrocket_sidecar.train as train
from polyrocket_sidecar.train import run_train_job, run_promote_model


class TrainJobTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self._env = os.environ.get("POLYROCKET_SIDECAR_MODEL_DIR")
        os.environ["POLYROCKET_SIDECAR_MODEL_DIR"] = self.tmp.name
        # Reload module-level path constants to pick up the new env
        train.MODEL_DIR = Path(self.tmp.name)
        train.CANDIDATE_FILE = train.MODEL_DIR / "candidate.json"
        train.ACTIVE_FILE = train.MODEL_DIR / "active.json"

    def tearDown(self) -> None:
        if self._env is None:
            os.environ.pop("POLYROCKET_SIDECAR_MODEL_DIR", None)
        else:
            os.environ["POLYROCKET_SIDECAR_MODEL_DIR"] = self._env
        self.tmp.cleanup()

    def test_train_returns_completed(self) -> None:
        result = run_train_job(n_trials=2, epochs=20)
        self.assertEqual(result["status"], "completed")
        self.assertIn("job_id", result)
        self.assertIn("best_brier", result)
        self.assertIn("best_params", result)
        self.assertIn("trials", result)
        self.assertGreaterEqual(len(result["trials"]), 2)
        # Brier score is in [0, 1] (squared error on probabilities)
        self.assertGreaterEqual(result["best_brier"], 0.0)
        self.assertLessEqual(result["best_brier"], 1.0)

    def test_train_writes_candidate_file(self) -> None:
        result = run_train_job(n_trials=1, epochs=10)
        candidate_path = Path(result["candidate_path"])
        self.assertTrue(candidate_path.exists())
        # The file is valid JSON
        data = json.loads(candidate_path.read_text())
        self.assertEqual(data["job_id"], result["job_id"])
        self.assertIn("best", data)
        self.assertIn("all_trials", data)

    def test_train_atomic_write(self) -> None:
        """No half-written candidate file should ever be visible."""
        result = run_train_job(n_trials=1, epochs=10)
        # After completion, no .tmp file should remain
        tmp_files = list(train.MODEL_DIR.glob("*.json.tmp"))
        self.assertEqual(tmp_files, [])

    def test_train_default_n_trials(self) -> None:
        # n_trials=0 should still produce at least 1 trial (clamped)
        result = run_train_job(n_trials=0, epochs=5)
        self.assertEqual(result["status"], "completed")
        self.assertGreaterEqual(len(result["trials"]), 1)


class PromoteModelTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self._env = os.environ.get("POLYROCKET_SIDECAR_MODEL_DIR")
        os.environ["POLYROCKET_SIDECAR_MODEL_DIR"] = self.tmp.name
        train.MODEL_DIR = Path(self.tmp.name)
        train.CANDIDATE_FILE = train.MODEL_DIR / "candidate.json"
        train.ACTIVE_FILE = train.MODEL_DIR / "active.json"

    def tearDown(self) -> None:
        if self._env is None:
            os.environ.pop("POLYROCKET_SIDECAR_MODEL_DIR", None)
        else:
            os.environ["POLYROCKET_SIDECAR_MODEL_DIR"] = self._env
        self.tmp.cleanup()

    def test_promote_fails_without_candidate(self) -> None:
        result = run_promote_model()
        self.assertFalse(result["promoted"])
        self.assertEqual(result["status"], "failed")
        self.assertIn("no candidate", result["message"])

    def test_promote_succeeds_after_train(self) -> None:
        # Run a train first
        train_result = run_train_job(n_trials=1, epochs=5)
        self.assertEqual(train_result["status"], "completed")
        # Now promote
        promote = run_promote_model()
        self.assertTrue(promote["promoted"], msg=str(promote))
        self.assertEqual(promote["status"], "ok")
        self.assertTrue(Path(promote["active_path"]).exists())
        # The active file contains the candidate's best params
        active_data = json.loads(Path(promote["active_path"]).read_text())
        self.assertIn("best", active_data)
        self.assertIn("promoted_at_ms", active_data)

    def test_promote_with_matching_job_id(self) -> None:
        train_result = run_train_job(n_trials=1, epochs=5)
        promote = run_promote_model(job_id=train_result["job_id"])
        self.assertTrue(promote["promoted"])

    def test_promote_with_mismatched_job_id(self) -> None:
        run_train_job(n_trials=1, epochs=5)
        promote = run_promote_model(job_id="wrong-job-id")
        self.assertFalse(promote["promoted"])
        self.assertEqual(promote["status"], "failed")
        self.assertIn("mismatch", promote["message"])

    def test_full_workflow_train_then_promote(self) -> None:
        """End-to-end: train writes candidate, promote moves it to active."""
        # 1. No active file yet
        self.assertFalse(train.ACTIVE_FILE.exists())
        # 2. Train
        train_result = run_train_job(n_trials=2, epochs=10)
        self.assertEqual(train_result["status"], "completed")
        # 3. Promote
        promote = run_promote_model()
        self.assertTrue(promote["promoted"])
        # 4. Both files exist
        self.assertTrue(train.CANDIDATE_FILE.exists())
        self.assertTrue(train.ACTIVE_FILE.exists())
        # 5. Active file is a superset of the candidate
        active = json.loads(train.ACTIVE_FILE.read_text())
        candidate = json.loads(train.CANDIDATE_FILE.read_text())
        self.assertEqual(active["job_id"], candidate["job_id"])
        self.assertEqual(active["best"], candidate["best"])

    def test_promote_appends_to_history(self) -> None:
        """v0.19a: each successful promote appends one entry to
        active.json.promotion_history. Two promotes → 2 entries.
        """
        # First train + promote
        t1 = run_train_job(n_trials=1, epochs=5)
        p1 = run_promote_model()
        self.assertTrue(p1["promoted"])
        active = json.loads(train.ACTIVE_FILE.read_text())
        self.assertIn("promotion_history", active)
        self.assertEqual(len(active["promotion_history"]), 1)
        self.assertEqual(active["promotion_history"][0]["job_id"], t1["job_id"])
        self.assertEqual(
            active["promotion_history"][0]["model_version"],
            f"logistic-{t1['job_id']}",
        )

        # Second train + promote → history grows to 2
        t2 = run_train_job(n_trials=1, epochs=5)
        p2 = run_promote_model()
        self.assertTrue(p2["promoted"])
        active2 = json.loads(train.ACTIVE_FILE.read_text())
        self.assertEqual(len(active2["promotion_history"]), 2)
        # Newest entry is last
        self.assertEqual(active2["promotion_history"][1]["job_id"], t2["job_id"])
        # Oldest is still there
        self.assertEqual(active2["promotion_history"][0]["job_id"], t1["job_id"])

    def test_run_list_promote_history_round_trip(self) -> None:
        """v0.19a: list_promote_history returns what was written."""
        from polyrocket_sidecar.train import run_list_promote_history
        # Train + promote once
        t = run_train_job(n_trials=1, epochs=5)
        run_promote_model()
        h = run_list_promote_history()
        self.assertTrue(h["ok"])
        self.assertEqual(h["count"], 1)
        self.assertEqual(h["entries"][0]["job_id"], t["job_id"])
        self.assertEqual(
            h["entries"][0]["model_version"],
            f"logistic-{t['job_id']}",
        )


if __name__ == "__main__":
    unittest.main()
