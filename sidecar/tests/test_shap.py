"""Tests for the v0.59 SHAP (KernelExplainer)
implementation.

Covers:
  1. _predict_logistic matches predict._sigmoid
  2. _kernel_weight closed form
  3. _build_coalitions enumerates all 2^M masks
  4. _fit_weighted_ls recovers the gradient
     of a known linear function
  5. run_shap_explainability: efficiency axiom
     (Σφ_i ≈ f(x) - E[f(x)] within float tol)
  6. run_shap_explainability: error path
"""

import math
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(ROOT))

from polyrocket_sidecar.shap import (
    _predict_logistic,
    _kernel_weight,
    _build_coalitions,
    _fit_weighted_ls,
    _solve_linear,
    run_shap_explainability,
)


class TestShapKernelExplainer(unittest.TestCase):
    def test_predict_logistic_matches_sigmoid(self):
        """v0.59 — _predict_logistic should match
        predict._sigmoid. They're both
        numerically stable sigmoid impls; we
        assert on a few (w, x) pairs."""
        from polyrocket_sidecar.predict import _sigmoid
        for w in [(0.1, 2.4, -0.02), (0.0, 0.0, 0.0), (1.5, -3.0, 0.5)]:
            for x in [(1.0, 0.5, 24.0), (1.0, 0.0, 0.0), (1.0, 1.0, 100.0)]:
                z = sum(wi * xi for wi, xi in zip(w, x))
                expected = _sigmoid(z)
                got = _predict_logistic(w, x)
                self.assertAlmostEqual(
                    expected, got, places=10,
                    msg=f"sigmoid mismatch w={w} x={x}",
                )

    def test_kernel_weight_closed_form(self):
        """v0.59 — w(z) = (M-1) / (C(M,|z|) * |z| *
        (M-|z|)). We test the special cases
        (empty + full coalition = large
        constant) and the M=3 case in the
        middle."""
        # Empty coalition: |z|=0 → large constant
        self.assertEqual(_kernel_weight(3, 0), 1.0e6)
        # Full coalition: |z|=M → large constant
        self.assertEqual(_kernel_weight(3, 3), 1.0e6)
        # Half coalition: |z|=1 in M=3
        # binom(3, 1) = 3, |z|=1, M-|z|=2
        # w = 2 / (3 * 1 * 2) = 1/3
        self.assertAlmostEqual(_kernel_weight(3, 1), 1.0 / 3.0, places=10)
        # |z|=2 in M=3: same as |z|=1 by symmetry
        self.assertAlmostEqual(_kernel_weight(3, 2), 1.0 / 3.0, places=10)

    def test_build_coalitions_enumerates_all(self):
        """v0.59 — _build_coalitions must return
        all 2^M binary masks."""
        for m in [1, 2, 3, 4]:
            coalitions = _build_coalitions(m)
            self.assertEqual(len(coalitions), 2**m)
            # Every mask is a list of m 0/1 ints.
            for c in coalitions:
                self.assertEqual(len(c), m)
                self.assertTrue(all(b in (0, 1) for b in c))
            # No duplicates.
            self.assertEqual(
                len({tuple(c) for c in coalitions}),
                2**m,
            )

    def test_solve_linear(self):
        """v0.59 — sanity check on the LS
        solver."""
        # 2 unknowns, 3 equations (over-determined).
        # x + 2y = 5, 2x - y = 0, x + y = 3 → x=1, y=2.
        a = [[1.0, 2.0], [2.0, -1.0], [1.0, 1.0]]
        b = [5.0, 0.0, 3.0]
        x = _solve_linear(a, b)
        self.assertAlmostEqual(x[0], 1.0, places=6)
        self.assertAlmostEqual(x[1], 2.0, places=6)

    def test_fit_weighted_ls_recovers_gradient(self):
        """v0.59 — for a known linear function
        f(x) = a + b*x0 + c*x1 + d*x2, the
        fitted SHAP coefficients should
        satisfy:
          - phi[0] (the "bias" / E[f(x)] term)
            = f(background) = a + b*bg[0]
          - phi[1..M] (per-feature contributions)
            = (b, c, d) — the gradient scaled by
            the feature delta (x_target - bg)
        """
        a, b, c, d = 1.0, 2.0, -3.0, 0.5
        # Build 8 coalitions for M=3, evaluate
        # f at each imputed input.
        m = 3
        coalitions = _build_coalitions(m)
        # Use the background = (1.0, 0.0, 0.0)
        # for all features.
        bg = (1.0, 0.0, 0.0)
        x_target = (1.0, 2.0, 4.0)
        outputs = []
        for mask in coalitions:
            x = tuple(
                x_target[i] if mask[i] else bg[i]
                for i in range(m)
            )
            outputs.append(a + b * x[0] + c * x[1] + d * x[2])
        weights = [_kernel_weight(m, sum(m_)) for m_ in coalitions]
        # Fit: X has columns [1, mask[0], mask[1], mask[2]]
        # Returns 4 coefficients: E[f(x)] (the
        # baseline), then per-feature contributions
        # = w_i * (x_target_i - bg_i).
        phi = _fit_weighted_ls(coalitions, outputs, weights, m)
        # Baseline = f(bg) = a + b*1 + c*0 + d*0 = 3
        expected_baseline = a + b * bg[0] + c * bg[1] + d * bg[2]
        self.assertAlmostEqual(phi[0], expected_baseline, places=4)
        # Per-feature: w * (x - bg)
        self.assertAlmostEqual(phi[1], b * (x_target[0] - bg[0]), places=4)
        self.assertAlmostEqual(phi[2], c * (x_target[1] - bg[1]), places=4)
        self.assertAlmostEqual(phi[3], d * (x_target[2] - bg[2]), places=4)
        # Efficiency: sum of feature contributions
        # = f(x_target) - f(bg) = (1+4-12+2) - (1+2)
        # = -5 - 3 = -8. Verify.
        self.assertAlmostEqual(
            sum(phi[1:]),
            (a + b*x_target[0] + c*x_target[1] + d*x_target[2])
            - expected_baseline,
            places=4,
        )


class TestShapEndToEnd(unittest.TestCase):
    """End-to-end tests that write a real
    active.json (matching the v0.12+ schema)
    and run run_shap_explainability against
    it. We use a tiny temp dir for the model
    files so we don't touch the real ~/
    .polyrocket/sidecar/models/."""

    def setUp(self):
        import tempfile
        from polyrocket_sidecar.train import ACTIVE_FILE, MODEL_DIR
        self.tmp = tempfile.TemporaryDirectory()
        self.env = {"POLYROCKET_HOME": self.tmp.name}
        # Polyrocket's train.py uses a hard-coded
        # ~/.polyrocket/sidecar/models/ path. We
        # monkey-patch MODEL_DIR to point at our
        # temp dir for the duration of the test.
        import polyrocket_sidecar.train as train_mod
        self._orig_model_dir = train_mod.MODEL_DIR
        train_mod.MODEL_DIR = Path(self.tmp.name) / "models"
        train_mod.MODEL_DIR.mkdir(parents=True, exist_ok=True)
        train_mod.ACTIVE_FILE = train_mod.MODEL_DIR / "active.json"
        # Reset the active cache.
        from polyrocket_sidecar.active import reset_cache
        reset_cache()
        # Write a minimal active.json.
        import json
        active = {
            "model_version": "logistic-test-shap",
            "weights": {"w0": 0.1, "w1": 2.4, "w2": -0.02},
            "promoted_at_ms": 1700000000000,
        }
        with open(train_mod.ACTIVE_FILE, "w") as f:
            json.dump(active, f)

    def tearDown(self):
        import polyrocket_sidecar.train as train_mod
        train_mod.MODEL_DIR = self._orig_model_dir
        self.tmp.cleanup()
        from polyrocket_sidecar.active import reset_cache
        reset_cache()

    def test_efficiency_axiom_holds(self):
        """v0.59 — Σφ_i = f(x) - E[f(x)] within
        float tolerance. This is the SHAP
        efficiency axiom; if it doesn't hold,
        the regression didn't converge."""
        r = run_shap_explainability(
            model_version="logistic-test-shap",
            sample={"price": 0.5, "market_age_hours": 24.0},
        )
        self.assertTrue(r["ok"], r.get("message"))
        self.assertEqual(r["method"], "kernel_shap")
        # Efficiency: Σφ_i ≈ f(x) - E[f(x)]
        # (the efficiency_diff is the residual
        # after fitting; should be ~0).
        self.assertIsNotNone(r["efficiency_diff"])
        self.assertAlmostEqual(
            r["efficiency_diff"], 0.0, places=4,
            msg=f"efficiency diff {r['efficiency_diff']} too large",
        )
        # Sanity: 3 features returned.
        self.assertEqual(len(r["features"]), 3)
        # Sorted by abs_shap descending.
        for i in range(len(r["features"]) - 1):
            self.assertGreaterEqual(
                r["features"][i]["abs_shap"],
                r["features"][i + 1]["abs_shap"],
            )

    def test_efficiency_holds_at_extremes(self):
        """v0.59 — the efficiency axiom should
        hold at the corners of the input
        space (price=0, price=1, age=0, age=large)."""
        for price in [0.01, 0.5, 0.99]:
            for age in [0.1, 24, 168, 720]:
                r = run_shap_explainability(
                    model_version="logistic-test-shap",
                    sample={"price": price, "market_age_hours": age},
                )
                self.assertTrue(r["ok"], r.get("message"))
                self.assertAlmostEqual(
                    r["efficiency_diff"], 0.0, places=3,
                    msg=f"efficiency diff for p={price} age={age}: "
                        f"{r['efficiency_diff']}",
                )

    def test_unknown_model_returns_error(self):
        r = run_shap_explainability(
            model_version="logistic-does-not-exist",
            sample={"price": 0.5, "market_age_hours": 24.0},
        )
        self.assertFalse(r["ok"])
        self.assertIn("not found", r["message"])

    def test_invalid_sample_returns_error(self):
        r = run_shap_explainability(
            model_version="logistic-test-shap",
            sample={"price": 1.5, "market_age_hours": 24.0},  # > 1
        )
        self.assertFalse(r["ok"])
        self.assertIn("price", r["message"])


if __name__ == "__main__":
    unittest.main()
