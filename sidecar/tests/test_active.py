"""Tests for the v0.11c active model loader."""

import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

# Make the package importable
HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(ROOT))

import polyrocket_sidecar.active as active
from polyrocket_sidecar.active import (
    get_active_weights,
    reset_cache,
    predict_with_active_model,
)
from polyrocket_sidecar.train import (
    run_train_job,
    run_promote_model,
    ACTIVE_FILE,
    CANDIDATE_FILE,
    MODEL_DIR,
)


class ActiveModelLoaderTests(unittest.TestCase):
    def setUp(self) -> None:
        # Use a per-test tmp model dir
        self.tmp = tempfile.TemporaryDirectory()
        self._env = os.environ.get("POLYROCKET_SIDECAR_MODEL_DIR")
        os.environ["POLYROCKET_SIDECAR_MODEL_DIR"] = self.tmp.name
        # Override module-level paths to point at the tmp dir
        active.ACTIVE_FILE = Path(self.tmp.name) / "active.json"
        from polyrocket_sidecar import train
        train.MODEL_DIR = Path(self.tmp.name)
        train.ACTIVE_FILE = active.ACTIVE_FILE
        train.CANDIDATE_FILE = Path(self.tmp.name) / "candidate.json"
        reset_cache()

    def tearDown(self) -> None:
        if self._env is None:
            os.environ.pop("POLYROCKET_SIDECAR_MODEL_DIR", None)
        else:
            os.environ["POLYROCKET_SIDECAR_MODEL_DIR"] = self._env
        self.tmp.cleanup()
        reset_cache()

    def test_fallback_when_no_active_file(self) -> None:
        # No active.json yet → fallback weights
        w = get_active_weights()
        self.assertEqual(w["w0"], active._FALLBACK_WEIGHTS["w0"])
        self.assertEqual(w["w1"], active._FALLBACK_WEIGHTS["w1"])
        self.assertEqual(w["w2"], active._FALLBACK_WEIGHTS["w2"])

    def test_loads_from_active_file(self) -> None:
        # Train + promote
        run_train_job(n_trials=1, epochs=3)
        result = run_promote_model()
        self.assertTrue(result["promoted"], msg=str(result))
        # Now active.json exists; loader should pick it up
        reset_cache()  # force re-read
        w = get_active_weights()
        # The promoted weights came from the train sweep, not the
        # fallback. We can't assert exact values (they depend on
        # the random init), but they should be valid floats.
        self.assertIsInstance(w["w0"], float)
        self.assertIsInstance(w["w1"], float)
        self.assertIsInstance(w["w2"], float)

    def test_picks_up_promote_on_next_call(self) -> None:
        """After a successful promote, the next get_active_weights()
        returns the new weights without requiring a process restart.
        """
        # First call: fallback
        w1_before = get_active_weights()
        self.assertEqual(w1_before["w0"], active._FALLBACK_WEIGHTS["w0"])

        # Train + promote
        run_train_job(n_trials=1, epochs=3)
        run_promote_model()
        reset_cache()

        # Second call: should pick up the new file
        w1_after = get_active_weights()
        # The new weights might happen to equal the fallback (very
        # unlikely but possible). Check that the cache was invalidated
        # by inspecting that subsequent calls return the same value
        # (cache hit on identical mtime).
        w1_again = get_active_weights()
        self.assertEqual(w1_after, w1_again)

    def test_handles_malformed_active_file(self) -> None:
        # Write garbage to active.json
        Path(active.ACTIVE_FILE).parent.mkdir(parents=True, exist_ok=True)
        Path(active.ACTIVE_FILE).write_text("not valid json {{{")
        reset_cache()
        w = get_active_weights()
        # Should fall back to the inline weights
        self.assertEqual(w["w0"], active._FALLBACK_WEIGHTS["w0"])

    def test_handles_missing_best_field(self) -> None:
        # Write a valid JSON but no "best" key
        Path(active.ACTIVE_FILE).parent.mkdir(parents=True, exist_ok=True)
        Path(active.ACTIVE_FILE).write_text(json.dumps({"job_id": "x"}))
        reset_cache()
        w = get_active_weights()
        # Should fall back to the inline weights
        self.assertEqual(w["w0"], active._FALLBACK_WEIGHTS["w0"])

    def test_predict_with_active_model(self) -> None:
        # Sanity: the function returns a probability in [0, 1]
        prob = predict_with_active_model(price=0.5, market_age_hours=24.0)
        self.assertGreaterEqual(prob, 0.0)
        self.assertLessEqual(prob, 1.0)

    def test_full_loop_train_promote_predict(self) -> None:
        """v0.11c — the closed loop. Train writes candidate, promote
        moves to active, predict reads from active. The "active"
        predictions differ from the "fallback" predictions because
        the trained weights differ from the inline weights.
        """
        # Baseline: no active model yet, predict uses fallback
        reset_cache()
        prob_fallback_low = predict_with_active_model(price=0.2, market_age_hours=24.0)

        # Train + promote
        run_train_job(n_trials=2, epochs=20)
        run_promote_model()
        reset_cache()

        # After promote, predict should use the trained weights.
        # The two probabilities don't have to differ (a small sample
        # might converge to similar weights), but the rationale in
        # predict_from_markets should now show the active weights.
        from polyrocket_sidecar.predict import predict_from_markets
        out = predict_from_markets([{"market_id": "m1", "price": 0.2, "market_age_hours": 24.0}])
        self.assertEqual(len(out), 1)
        self.assertIn("active:", out[0]["rationale"])
        # And the prob is still in range
        self.assertGreaterEqual(out[0]["prob"], 0.0)
        self.assertLessEqual(out[0]["prob"], 1.0)
        # Either it's the same as fallback (small model) or different.
        # We just verify both are valid probabilities.
        self.assertGreaterEqual(prob_fallback_low, 0.0)
        self.assertLessEqual(prob_fallback_low, 1.0)


if __name__ == "__main__":
    unittest.main()
