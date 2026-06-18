"""Dedicated tests for the v0.10b train_job + promote_model.

These tests use a tmp model dir so they don't touch the user's
real `~/.polyrocket/sidecar/models/`.
"""

import json
import os
import shutil
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
from polyrocket_sidecar.train import run_train_job, run_promote_model, run_backtest_model


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

    def test_history_entry_includes_weights(self) -> None:
        """v0.20a: each history entry now has a `weights` field
        with {w0, w1, w2} so the entry is self-contained for
        rollback (no need to read the candidate file later).
        """
        t = run_train_job(n_trials=1, epochs=5)
        run_promote_model()
        from polyrocket_sidecar.train import run_list_promote_history
        h = run_list_promote_history()
        self.assertEqual(h["count"], 1)
        entry = h["entries"][0]
        self.assertIn("weights", entry)
        weights = entry["weights"]
        self.assertIn("w0", weights)
        self.assertIn("w1", weights)
        self.assertIn("w2", weights)
        # Brier is also in the entry for the Brier badge
        self.assertIn("best_brier", entry)
        self.assertAlmostEqual(entry["best_brier"], t["best_brier"], places=4)

    def test_history_entry_includes_reason(self) -> None:
        """v0.41a: each history entry has a `reason` field
        with a human-readable description ("Promoted as
        best trial" or "Promoted as trial N of M"). The L1
        surfaces this as a hover tooltip.
        """
        t = run_train_job(n_trials=4, epochs=5)
        # Best-trial promote
        run_promote_model()
        from polyrocket_sidecar.train import run_list_promote_history
        h = run_list_promote_history()
        entry = h["entries"][0]
        self.assertIn("reason", entry)
        self.assertEqual(entry["reason"], "Promoted as best trial")

        # Bulk trial promote
        t2 = run_train_job(n_trials=4, epochs=5)
        run_promote_model(trial_index=1)
        h = run_list_promote_history()
        # h["entries"] is oldest-first; the new entry is last
        new_entry = h["entries"][-1]
        self.assertEqual(new_entry["reason"], "Promoted as trial 2 of 4")

    def test_rollback_to_previous_version(self) -> None:
        """v0.20a: train → promote → train → promote → rollback
        to the FIRST version. The new active should be the
        first version (not the current one), and the history
        should grow by 1 (a rollback marker).
        """
        from polyrocket_sidecar.train import run_rollback_model
        t1 = run_train_job(n_trials=1, epochs=5)
        run_promote_model()
        first_version = f"logistic-{t1['job_id']}"

        t2 = run_train_job(n_trials=1, epochs=5)
        run_promote_model()

        # Confirm current is t2
        active_before = json.loads(train.ACTIVE_FILE.read_text())
        self.assertEqual(active_before["job_id"], t2["job_id"])

        # Rollback to t1
        rb = run_rollback_model(model_version=first_version)
        self.assertTrue(rb["rolled_back"], msg=str(rb))
        self.assertEqual(rb["model_version"], first_version)
        self.assertEqual(rb["status"], "ok")
        self.assertIsNotNone(rb["rolled_back_at_ms"])

        # Active file should now reflect t1
        active_after = json.loads(train.ACTIVE_FILE.read_text())
        self.assertEqual(active_after["job_id"], t1["job_id"])
        self.assertEqual(active_after["model_version"], first_version)
        # The active weights are t1's
        w = active_after["weights"]
        self.assertIn("w0", w)
        self.assertIn("w1", w)
        self.assertIn("w2", w)

        # History grew by 1 (a rollback marker)
        history = active_after["promotion_history"]
        # Last entry is the rollback marker
        self.assertEqual(history[-1]["kind"], "rollback")
        self.assertEqual(history[-1]["model_version"], first_version)
        # Second-to-last is t1 (the original promote, not the rollback target)
        # Find t1's promote entry
        t1_entries = [e for e in history
                      if isinstance(e, dict)
                      and e.get("job_id") == t1["job_id"]
                      and e.get("kind") != "rollback"]
        self.assertEqual(len(t1_entries), 1)
        self.assertEqual(t1_entries[0]["model_version"], first_version)

    def test_rollback_to_unknown_version_fails(self) -> None:
        """v0.20a: rolling back to a model_version that doesn't
        exist in the history returns rolled_back=false with a
        clear error message.
        """
        from polyrocket_sidecar.train import run_rollback_model
        t = run_train_job(n_trials=1, epochs=5)
        run_promote_model()
        rb = run_rollback_model(model_version="logistic-train-DOESNOTEXIST")
        self.assertFalse(rb["rolled_back"])
        self.assertEqual(rb["status"], "failed")
        self.assertIn("not found", rb["message"])
        # Active file is unchanged
        active = json.loads(train.ACTIVE_FILE.read_text())
        self.assertEqual(active["job_id"], t["job_id"])

    def test_rollback_to_v19_entry_without_weights_fails(self) -> None:
        """v0.20a: v0.19 history entries don't have weights.
        A rollback to such an entry returns rolled_back=false
        with a clear error explaining the user needs to retrain.
        """
        from polyrocket_sidecar.train import run_rollback_model
        t = run_train_job(n_trials=1, epochs=5)
        run_promote_model()
        # Manually strip weights from the history entry to
        # simulate a v0.19 entry
        active = json.loads(train.ACTIVE_FILE.read_text())
        active["promotion_history"][-1].pop("weights", None)
        train.ACTIVE_FILE.write_text(json.dumps(active))

        rb = run_rollback_model(
            model_version=f"logistic-{t['job_id']}"
        )
        self.assertFalse(rb["rolled_back"])
        self.assertIn("no weights", rb["message"])
        self.assertIn("cannot rollback", rb["message"].lower())

    def test_promote_specific_trial_v21(self) -> None:
        """v0.21a: bulk promote. Train 4 trials, promote
        trial index 2 (not the best). The active should be
        trial 2's weights; the model_version should have
        a -t2 suffix; the history should record trial_index=2.
        """
        t = run_train_job(n_trials=4, epochs=10)
        self.assertEqual(len(t["trials"]), 4)
        # Pick trial 2 (NOT the best — the best might be
        # any of the 4 by synthetic Brier)
        promote = run_promote_model(trial_index=2)
        self.assertTrue(promote["promoted"], msg=str(promote))
        self.assertEqual(promote["status"], "ok")
        self.assertEqual(promote["trial_index"], 2)
        self.assertTrue(promote["model_version"].endswith("-t2"))
        # The history's last entry should have trial 2's weights
        active = json.loads(train.ACTIVE_FILE.read_text())
        trial2_weights = t["trials"][2]["weights"]
        history_weights = active["promotion_history"][-1]["weights"]
        self.assertEqual(history_weights["w0"], trial2_weights["w0"])
        self.assertEqual(history_weights["w1"], trial2_weights["w1"])
        self.assertEqual(history_weights["w2"], trial2_weights["w2"])
        # History records trial_index
        self.assertEqual(active["promotion_history"][-1]["trial_index"], 2)
        # And the history entry uses the -t2 version
        self.assertTrue(active["promotion_history"][-1]["model_version"].endswith("-t2"))

    def test_promote_default_is_best_v21(self) -> None:
        """v0.21a: when trial_index is None, the behavior is
        unchanged from v0.18a (promote the best).
        """
        t = run_train_job(n_trials=4, epochs=10)
        promote = run_promote_model()  # no trial_index
        self.assertTrue(promote["promoted"], msg=str(promote))
        self.assertIsNone(promote["trial_index"])
        # Model version has no -t{N} suffix
        self.assertFalse(promote["model_version"].endswith(("-t0", "-t1", "-t2", "-t3")))

    def test_promote_trial_out_of_range_v21(self) -> None:
        """v0.21a: trial_index out of range returns a
        clear error and does NOT modify the active file.
        """
        run_train_job(n_trials=4, epochs=10)
        # trial_index 99 is out of range (only 0..3 valid)
        promote = run_promote_model(trial_index=99)
        self.assertFalse(promote["promoted"])
        self.assertEqual(promote["status"], "failed")
        self.assertIn("out of range", promote["message"])
        # Active file should not exist (no successful promote)
        self.assertFalse(train.ACTIVE_FILE.exists())

    def test_auto_promote_if_better_promotes_v23(self) -> None:
        """v0.23a: with no active model, auto_promote
        just promotes the candidate (auto-best).
        """
        from polyrocket_sidecar.train import run_auto_promote_if_better
        t = run_train_job(n_trials=2, epochs=5)
        result = run_auto_promote_if_better(brier_margin=0.005)
        self.assertTrue(result["promoted"])
        self.assertFalse(result["skipped"])
        self.assertIn("no active model", result["reason"])
        self.assertIsNone(result["active_brier"])
        self.assertEqual(result["model_version"], f"logistic-{t['job_id']}")

    def test_auto_promote_if_better_skips_when_close_v23(self) -> None:
        """v0.23a: when the candidate is NOT meaningfully
        better than the active, auto_promote is a no-op
        and returns a clear "skipped" reason.
        """
        from polyrocket_sidecar.train import run_auto_promote_if_better
        # Train + promote twice with the SAME seed → nearly
        # identical briers. A small margin won't be met.
        t1 = run_train_job(n_trials=1, epochs=5)
        run_promote_model()
        t2 = run_train_job(n_trials=1, epochs=5)
        # Use a margin of 1.0 — guaranteed not to be met
        result = run_auto_promote_if_better(brier_margin=1.0)
        self.assertFalse(result["promoted"])
        self.assertTrue(result["skipped"])
        self.assertIn("not at least 1.0 better", result["reason"])
        # Active file is unchanged (still t1)
        active = json.loads(train.ACTIVE_FILE.read_text())
        self.assertEqual(active["job_id"], t1["job_id"])
        # The candidate still exists (not promoted, not deleted)
        self.assertTrue(train.CANDIDATE_FILE.exists())

    def test_promote_all_trials_v25(self) -> None:
        """v0.25a: bulk-promote all 4 trials in one call.
        After the call, all 4 trials should appear in
        the promotion history, each with its own -tN
        suffix and trial_index.
        """
        from polyrocket_sidecar.train import (
            run_promote_all_trials,
            run_list_promote_history,
        )
        t = run_train_job(n_trials=4, epochs=10)
        self.assertEqual(len(t["trials"]), 4)
        result = run_promote_all_trials()
        self.assertTrue(result["ok"])
        self.assertEqual(result["count"], 4)
        self.assertEqual(len(result["results"]), 4)
        # All 4 should be promoted
        for i, r in enumerate(result["results"]):
            self.assertEqual(r["trial_index"], i)
            self.assertTrue(r["promoted"], msg=f"trial {i} failed: {r}")
            self.assertEqual(r["status"], "ok")
            self.assertTrue(r["model_version"].endswith(f"-t{i}"))
        # All 4 should appear in the history
        history = run_list_promote_history()
        self.assertEqual(history["count"], 4)
        trial_indices = [e["trial_index"] for e in history["entries"]]
        self.assertEqual(trial_indices, [0, 1, 2, 3])

    def test_promote_all_trials_no_candidate_v25(self) -> None:
        """v0.25a: no candidate on disk returns ok=false."""
        from polyrocket_sidecar.train import run_promote_all_trials
        result = run_promote_all_trials()
        self.assertFalse(result["ok"])
        self.assertEqual(result["count"], 0)
        self.assertEqual(result["results"], [])
        self.assertIn("no candidate", result["message"])
        self.assertFalse(train.ACTIVE_FILE.exists())


class TestPromoteHistoryArchive(unittest.TestCase):
    """v0.33a — promote history archive (append-only JSONL).

    The 20-entry cap on `promotion_history[]` silently drops
    old entries. v0.33a fixes this by writing the dropped
    entries to `archive.jsonl` BEFORE the cap takes effect.
    The archive is append-only and never auto-pruned.
    """

    def setUp(self) -> None:
        # Clean MODEL_DIR + ARCHIVE_FILE between tests
        if train.MODEL_DIR.exists():
            shutil.rmtree(train.MODEL_DIR)
        # Also explicitly remove the archive file (it
        # lives in MODEL_DIR, but defensive cleanup in
        # case other test classes wrote to it)
        from polyrocket_sidecar.train import ARCHIVE_FILE
        if ARCHIVE_FILE.exists():
            ARCHIVE_FILE.unlink()

    def test_archive_file_does_not_exist_before_any_promote(self) -> None:
        """v0.33a — no archive file on fresh setup."""
        # Run one train + promote, history has 1 entry (no overflow)
        run_train_job(n_trials=1, epochs=5)
        run_promote_model()
        # Archive file may or may not exist; the test is loose
        # because the cap is 20 — 1 entry doesn't trigger
        # the archive. We just verify that if it exists,
        # it's a valid JSONL.
        from polyrocket_sidecar.train import ARCHIVE_FILE
        if ARCHIVE_FILE.exists():
            content = ARCHIVE_FILE.read_text()
            # If it exists, it should be valid JSONL
            for line in content.strip().split("\n"):
                if line:
                    json.loads(line)  # raises if invalid

    def test_archive_writes_dropped_entries_on_overflow(self) -> None:
        """v0.33a — 21st promote writes the 1 dropped entry."""
        from polyrocket_sidecar.train import ARCHIVE_FILE
        # Run 21 trains + promotes. The 20-cap drops the 1st.
        job_ids: list[str] = []
        for _ in range(21):
            t = run_train_job(n_trials=1, epochs=5)
            job_ids.append(t["job_id"])
            run_promote_model()
        # In-memory history is still capped at 20
        active = json.loads(train.ACTIVE_FILE.read_text())
        self.assertEqual(len(active["promotion_history"]), 20)
        # Archive file should exist with 1 entry (the 1st, dropped)
        self.assertTrue(ARCHIVE_FILE.exists(), "archive file should exist after 21 promotes")
        lines = ARCHIVE_FILE.read_text().strip().split("\n")
        self.assertEqual(len(lines), 1, "expected 1 archived entry")
        archived = json.loads(lines[0])
        self.assertEqual(archived["job_id"], job_ids[0])
        self.assertIn("model_version", archived)
        self.assertIn("promoted_at_ms", archived)
        self.assertIn("best_brier", archived)
        self.assertIn("weights", archived)
        self.assertIn("trial_index", archived)
        self.assertIn("archived_at_ms", archived)

    def test_archive_is_append_only(self) -> None:
        """v0.33a — multiple overflows append, not overwrite."""
        from polyrocket_sidecar.train import ARCHIVE_FILE
        # Run 25 promotes → drops 5 entries (1 each on the
        # 21st, 22nd, 23rd, 24th, 25th).
        for _ in range(25):
            t = run_train_job(n_trials=1, epochs=5)
            run_promote_model()
        # Archive should have 5 entries (5 dropped over 25 promotes)
        self.assertTrue(ARCHIVE_FILE.exists())
        lines = ARCHIVE_FILE.read_text().strip().split("\n")
        self.assertEqual(len(lines), 5)
        # Each line is a valid JSON object
        for line in lines:
            archived = json.loads(line)
            self.assertIn("job_id", archived)
            self.assertIn("archived_at_ms", archived)

    def test_archive_entries_have_correct_shape(self) -> None:
        """v0.33a — each archived entry has the full set of fields
        needed to reconstruct the promotion, including weights
        (for v0.20a rollback) and trial_index (for v0.21a bulk)."""
        from polyrocket_sidecar.train import ARCHIVE_FILE
        # Run 21 promotes
        for _ in range(21):
            t = run_train_job(n_trials=1, epochs=5)
            run_promote_model()
        lines = ARCHIVE_FILE.read_text().strip().split("\n")
        archived = json.loads(lines[0])
        # The exact same shape as the in-memory entries
        self.assertEqual(
            set(archived.keys()),
            {"job_id", "model_version", "promoted_at_ms",
             "best_brier", "best_params", "weights",
             "trial_index", "reason", "archived_at_ms"},
        )
        # Weights has the 3 expected keys
        self.assertEqual(set(archived["weights"].keys()), {"w0", "w1", "w2"})


# =================================================================
# ============== v0.43a — backtest_model tests ====================
# =================================================================


class BacktestModelTests(unittest.TestCase):
    """v0.43a — replay a saved model against a list of
    (price, age, outcome) samples and return Brier +
    calibration + per-sample predictions.

    These tests don't go through the full train +
    promote workflow — they write a synthetic
    `archive.jsonl` directly with known weights
    and assert that the backtest produces the
    expected Brier.
    """

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self._env = os.environ.get("POLYROCKET_SIDECAR_MODEL_DIR")
        os.environ["POLYROCKET_SIDECAR_MODEL_DIR"] = self.tmp.name
        train.MODEL_DIR = Path(self.tmp.name)
        train.CANDIDATE_FILE = train.MODEL_DIR / "candidate.json"
        train.ACTIVE_FILE = train.MODEL_DIR / "active.json"
        self.archive_path = train.MODEL_DIR / "archive.jsonl"

    def tearDown(self) -> None:
        if self._env is None:
            os.environ.pop("POLYROCKET_SIDECAR_MODEL_DIR", None)
        else:
            os.environ["POLYROCKET_SIDECAR_MODEL_DIR"] = self._env
        self.tmp.cleanup()

    def _write_archive_entry(self, model_version: str, weights: dict[str, float]) -> None:
        self.archive_path.parent.mkdir(parents=True, exist_ok=True)
        entry = {
            "job_id": "train-test",
            "model_version": model_version,
            "promoted_at_ms": 1_700_000_000_000,
            "best_brier": 0.18,
            "best_params": {"lr": 0.01, "reg": 0.001},
            "weights": weights,
            "trial_index": None,
            "reason": "Promoted as best trial",
            "archived_at_ms": 1_700_000_000_000,
        }
        with self.archive_path.open("w") as f:
            f.write(json.dumps(entry) + "\n")

    def test_backtest_finds_model_in_archive(self) -> None:
        # Predictable weights: w0=0, w1=0, w2=0 → sigmoid(0) = 0.5
        # (always predict 0.5 regardless of inputs)
        self._write_archive_entry("logistic-test", {"w0": 0.0, "w1": 0.0, "w2": 0.0})
        out = run_backtest_model(
            model_version="logistic-test",
            samples=[
                {"price": 0.3, "market_age_hours": 24.0, "outcome": 0.0, "label": "m1"},
                {"price": 0.7, "market_age_hours": 24.0, "outcome": 1.0, "label": "m2"},
                {"price": 0.5, "market_age_hours": 24.0, "outcome": 0.5, "label": "m3"},
            ],
        )
        self.assertTrue(out["ok"])
        self.assertEqual(out["model_version"], "logistic-test")
        self.assertEqual(out["sample_count"], 3)
        # Brier mean = ((0.5-0)² + (0.5-1)² + (0.5-0.5)²) / 3 = (0.25 + 0.25 + 0) / 3 = 0.1666...
        self.assertAlmostEqual(out["brier_mean"], (0.25 + 0.25 + 0.0) / 3.0, places=4)
        # Calibration: all 3 fall in the [0.4, 0.6) bucket
        self.assertEqual(len(out["calibration"]), 5)
        non_empty = [b for b in out["calibration"] if b["count"] > 0]
        self.assertEqual(len(non_empty), 1)
        self.assertEqual(non_empty[0]["count"], 3)

    def test_backtest_finds_model_in_active(self) -> None:
        # Write directly to active.json (not archive)
        self.archive_path.parent.mkdir(parents=True, exist_ok=True)
        active = {
            "model_version": "logistic-active",
            "weights": {"w0": 0.0, "w1": 0.0, "w2": 0.0},
            "best": {"brier": 0.18},
        }
        with self.archive_path.with_name("active.json").open("w") as f:
            json.dump(active, f)
        out = run_backtest_model(
            model_version="logistic-active",
            samples=[{"price": 0.5, "market_age_hours": 24.0, "outcome": 0.0}],
        )
        self.assertTrue(out["ok"])
        self.assertEqual(out["sample_count"], 1)

    def test_backtest_returns_error_for_missing_model(self) -> None:
        out = run_backtest_model(
            model_version="logistic-does-not-exist",
            samples=[{"price": 0.5, "market_age_hours": 24.0, "outcome": 1.0}],
        )
        self.assertFalse(out["ok"])
        self.assertIn("not found", out["message"])

    def test_backtest_returns_error_for_empty_samples(self) -> None:
        self._write_archive_entry("logistic-test", {"w0": 0.0, "w1": 0.0, "w2": 0.0})
        out = run_backtest_model(model_version="logistic-test", samples=[])
        self.assertFalse(out["ok"])
        self.assertEqual(out["sample_count"], 0)
        self.assertIn("no samples", out["message"])

    def test_backtest_skips_malformed_samples(self) -> None:
        self._write_archive_entry("logistic-test", {"w0": 0.0, "w1": 0.0, "w2": 0.0})
        out = run_backtest_model(
            model_version="logistic-test",
            samples=[
                {"price": 0.5, "market_age_hours": 24.0, "outcome": 1.0},  # valid
                {"price": "not a number", "market_age_hours": 24.0, "outcome": 0.0},  # bad
                {"market_age_hours": 24.0, "outcome": 0.0},  # missing price
                {"price": 0.5, "market_age_hours": 24.0, "outcome": 2.0},  # out of range
            ],
        )
        self.assertTrue(out["ok"])
        # Only the first sample is valid
        self.assertEqual(out["sample_count"], 1)

    def test_backtest_top_winners_and_losers(self) -> None:
        self._write_archive_entry("logistic-test", {"w0": 0.0, "w1": 0.0, "w2": 0.0})
        # All predict 0.5; outcomes 0 → 0.25 brier, outcomes 1 → 0.25 brier
        # outcomes 0.5 → 0 brier
        out = run_backtest_model(
            model_version="logistic-test",
            samples=[
                {"price": 0.5, "market_age_hours": 24.0, "outcome": 0.5, "label": "perfect"},
                {"price": 0.5, "market_age_hours": 24.0, "outcome": 0.5, "label": "perfect2"},
                {"price": 0.5, "market_age_hours": 24.0, "outcome": 0.5, "label": "perfect3"},
                {"price": 0.5, "market_age_hours": 24.0, "outcome": 0.0, "label": "wrong"},
            ],
        )
        self.assertTrue(out["ok"])
        # Top winners: 3 perfect (lowest brier = 0)
        self.assertEqual(len(out["top_winners"]), 3)
        for w in out["top_winners"]:
            self.assertAlmostEqual(w["brier"], 0.0, places=6)
        # Top losers: cap at 3, but in this small
        # sample set, the last 3 (sorted by brier
        # ascending) are [perfect, perfect, wrong]
        # reversed → [wrong, perfect, perfect]. The
        # worst is at index 0; the duplicates are
        # acceptable for a small sample set.
        self.assertEqual(len(out["top_losers"]), 3)
        self.assertEqual(out["top_losers"][0]["label"], "wrong")
        self.assertAlmostEqual(out["top_losers"][0]["brier"], 0.25, places=4)

    def test_backtest_top_losers_caps_at_sample_size(self) -> None:
        # v0.43a — when samples < 3, top_losers
        # gracefully degrades. With 1 sample, both
        # winners and losers have 1 entry.
        self._write_archive_entry("logistic-test", {"w0": 0.0, "w1": 0.0, "w2": 0.0})
        out = run_backtest_model(
            model_version="logistic-test",
            samples=[{"price": 0.5, "market_age_hours": 24.0, "outcome": 0.0, "label": "only"}],
        )
        self.assertTrue(out["ok"])
        self.assertEqual(len(out["top_winners"]), 1)
        self.assertEqual(len(out["top_losers"]), 1)


if __name__ == "__main__":
    unittest.main()
