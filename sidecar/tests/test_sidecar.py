"""Tests for the sidecar.

Run with:
    cd sidecar && python3 -m unittest tests.test_sidecar -v

We use unittest (not pytest) so the sidecar has zero runtime deps.
"""

import json
import os
import subprocess
import sys
import unittest
from pathlib import Path

# Make the package importable when running tests/ directly
HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(ROOT))

from polyrocket_sidecar import (  # noqa: E402
    SidecarError,
    SidecarRequest,
    SidecarResponse,
    parse_line,
    predict_logic,
    predict_from_markets,
    serialize_response,
    PROTOCOL_VERSION,
)
from polyrocket_sidecar.dispatch import DISPATCH  # noqa: E402


class ProtocolTests(unittest.TestCase):
    def test_parse_ping(self) -> None:
        # NOTE: wire format is lowercase method names to match Rust.
        req = parse_line('{"id": 1, "method": "ping", "params": {}}')
        self.assertEqual(req.id, 1)
        self.assertEqual(req.method, "ping")
        self.assertEqual(req.params, {})

    def test_parse_missing_id(self) -> None:
        with self.assertRaises(ValueError):
            parse_line('{"method": "ping"}')

    def test_parse_bad_json(self) -> None:
        with self.assertRaises(ValueError):
            parse_line("not json at all")

    def test_parse_params_defaults_to_empty(self) -> None:
        req = parse_line('{"id": 7, "method": "ping"}')
        self.assertEqual(req.params, {})

    def test_serialize_response_result(self) -> None:
        line = serialize_response(SidecarResponse(id=1, result={"pong": True}))
        obj = json.loads(line)
        self.assertEqual(obj["id"], 1)
        self.assertTrue(obj["ok"])
        self.assertEqual(obj["result"], {"pong": True})

    def test_serialize_response_error(self) -> None:
        line = serialize_response(
            SidecarResponse(id=1, error=SidecarError(code=-32601, message="nope"))
        )
        obj = json.loads(line)
        self.assertEqual(obj["id"], 1)
        self.assertFalse(obj["ok"])
        # The Rust SidecarResponse.error is Option<String>, so we serialize
        # as a string with the code embedded in the message text.
        self.assertIsInstance(obj["error"], str)
        self.assertIn("-32601", obj["error"])
        self.assertIn("nope", obj["error"])

    def test_serialize_includes_ok_for_rust(self) -> None:
        # The Rust `parse_line` distinguishes requests (have `method`) from
        # responses (have `ok`). If a response is missing `ok`, the Rust side
        # rejects it. This test guards against accidental removal.
        for r in [
            SidecarResponse(id=1, result={"x": 1}),
            SidecarResponse(id=2, error=SidecarError(code=-1, message="x")),
        ]:
            obj = json.loads(serialize_response(r))
            self.assertIn("ok", obj, msg=f"missing 'ok' in {obj}")
            self.assertIsInstance(obj["ok"], bool)

    def test_request_to_json_round_trip(self) -> None:
        req = SidecarRequest(id=42, method="predict", params={"markets": [{"market_id": "m1", "price": 0.5}]})
        round_tripped = parse_line(req.to_json())
        self.assertEqual(round_tripped.id, 42)
        self.assertEqual(round_tripped.method, "predict")
        self.assertEqual(round_tripped.params, {"markets": [{"market_id": "m1", "price": 0.5}]})


class PredictTests(unittest.TestCase):
    def test_zero_price_zero_age_near_baseline(self) -> None:
        # price=0 (max cheap) + age=0 → high prob
        p = predict_logic(price=0.0, market_age_hours=0.0)
        self.assertGreater(p, 0.5)

    def test_high_price(self) -> None:
        # price=1 (max expensive) → low prob
        p = predict_logic(price=1.0, market_age_hours=24.0)
        self.assertLess(p, 0.5)

    def test_probability_bounded(self) -> None:
        for price in (0.0, 0.1, 0.5, 0.9, 1.0):
            for age in (0, 1, 24, 168, 1000):
                p = predict_logic(price=price, market_age_hours=age)
                self.assertGreaterEqual(p, 0.0)
                self.assertLessEqual(p, 1.0)

    def test_predict_from_markets_rust_shape(self) -> None:
        """The output MUST match what `domain::lab::sidecar::parse_predict_response`
        expects on the Rust side: { predictions: [{ market_id, prob, confidence, rationale }],
                                     model_version: "logistic-..." }.
        v0.12a — model_version is hoisted to the top level.
        """
        result = predict_from_markets([{"market_id": "m1", "price": 0.5, "market_age_hours": 24}])
        self.assertIn("predictions", result)
        self.assertIn("model_version", result)
        out = result["predictions"]
        self.assertEqual(len(out), 1)
        p = out[0]
        self.assertEqual(p["market_id"], "m1")
        self.assertIn("prob", p)
        self.assertIn("confidence", p)
        self.assertIn("rationale", p)
        self.assertGreaterEqual(p["prob"], 0.0)
        self.assertLessEqual(p["prob"], 1.0)
        self.assertGreaterEqual(p["confidence"], 0.0)
        self.assertLessEqual(p["confidence"], 1.0)
        # v0.12a — model_version should be a non-empty string
        self.assertIsInstance(result["model_version"], str)
        self.assertGreater(len(result["model_version"]), 0)

    def test_empty_markets(self) -> None:
        result = predict_from_markets([])
        self.assertEqual(result["predictions"], [])
        self.assertIn("model_version", result)

    def test_skips_market_with_no_id(self) -> None:
        result = predict_from_markets([{"price": 0.5}, {"market_id": "m1", "price": 0.5}])
        out = result["predictions"]
        self.assertEqual(len(out), 1)
        self.assertEqual(out[0]["market_id"], "m1")

    def test_garbage_price_uses_default(self) -> None:
        result = predict_from_markets([{"market_id": "m1", "price": "lol"}])
        out = result["predictions"]
        self.assertEqual(len(out), 1)
        # default price = 0.5 → confidence = 0
        self.assertEqual(out[0]["confidence"], 0.0)

    def test_returns_model_version_field(self) -> None:
        """v0.12a — predict_from_markets returns a `model_version` field
        at the top level. The default (no active model) is
        'logistic-0.1.0'.
        """
        from polyrocket_sidecar.active import reset_cache
        reset_cache()
        result = predict_from_markets([{"market_id": "m1", "price": 0.5}])
        self.assertEqual(result["model_version"], "logistic-0.1.0")


class BenchTests(unittest.TestCase):
    """v0.11d — the hot path is hot enough.

    We don't assert on an exact timing (CI hosts vary), just that
    10k markets finish in <500ms on a dev machine. A regression
    to the slow path (e.g. unhoisted import, accidental global
    re-read) would push this well over 1s.
    """
    def test_10k_markets_under_500ms(self) -> None:
        from polyrocket_sidecar.predict import predict_from_markets
        import time
        markets = [
            {"market_id": f"m{i}", "price": (i * 0.0001) % 1.0, "market_age_hours": i * 0.1}
            for i in range(10_000)
        ]
        started = time.perf_counter()
        result = predict_from_markets(markets)
        elapsed = time.perf_counter() - started
        # v0.12a — result is now a dict with `predictions` array
        self.assertEqual(len(result["predictions"]), 10_000)
        self.assertLess(elapsed, 0.5, f"predict took {elapsed:.3f}s; expected <0.5s")


class DispatchTests(unittest.TestCase):
    def test_all_methods_registered(self) -> None:
        # If a method is added on the Rust side without registering here, this
        # catches it. Mirror the set in `SidecarMethod` enum on Rust.
        expected = {"ping", "predict", "train_job", "promote_model", "list_promote_history"}
        self.assertEqual(set(DISPATCH.keys()), expected)

    def test_ping_returns_pong(self) -> None:
        out = DISPATCH["ping"]({})
        self.assertTrue(out["pong"])
        self.assertIsInstance(out["ts_ms"], int)
        self.assertGreater(out["ts_ms"], 0)

    def test_predict_dispatches(self) -> None:
        out = DISPATCH["predict"]({"markets": [{"market_id": "m1", "price": 0.0}]})
        self.assertIn("predictions", out)
        self.assertEqual(len(out["predictions"]), 1)
        self.assertGreater(out["predictions"][0]["prob"], 0.5)

    def test_train_job_real(self) -> None:
        """v0.10b: train_job runs a real sweep and writes a candidate file."""
        out = DISPATCH["train_job"]({})
        self.assertEqual(out["status"], "completed", msg=f"train failed: {out}")
        self.assertIn("job_id", out)
        self.assertIn("best_brier", out)
        self.assertIn("best_params", out)
        self.assertIn("candidate_path", out)
        # Trials were attempted
        self.assertGreaterEqual(len(out.get("trials", [])), 1)
        # The candidate file should now exist on disk
        from pathlib import Path
        self.assertTrue(Path(out["candidate_path"]).exists())

    def test_promote_model_real(self) -> None:
        """v0.10b: promote_model requires a candidate first; without one
        it returns ok=false with a clear error message.
        """
        # Promote with no candidate on disk should fail cleanly
        # (the model_dir may have a leftover from test_train_job_real —
        # accept either outcome, just verify the shape).
        out = DISPATCH["promote_model"]({})
        if out.get("promoted"):
            self.assertEqual(out["status"], "ok")
            self.assertIn("active_path", out)
            self.assertIn("promoted_at_ms", out)
        else:
            # Either no candidate (from a clean test env) — both are valid
            self.assertEqual(out["status"], "failed")

    def test_list_promote_history_empty_or_populated(self) -> None:
        """v0.19a: list_promote_history returns ok=true with whatever
        is in active.json. If a previous test promoted, we may have
        entries; otherwise the response is empty + a helpful message.
        """
        out = DISPATCH["list_promote_history"]({})
        self.assertTrue(out["ok"])
        self.assertIn("entries", out)
        self.assertIn("count", out)
        self.assertIsInstance(out["entries"], list)
        self.assertEqual(out["count"], len(out["entries"]))
        # Each entry, if present, has the audit-relevant fields
        for e in out["entries"]:
            self.assertIn("job_id", e)
            self.assertIn("model_version", e)
            self.assertIn("promoted_at_ms", e)
            # best_brier and best_params are optional
            # (best_params may be null in some entries)

    def test_predict_rejects_non_list_markets(self) -> None:
        with self.assertRaises(ValueError):
            DISPATCH["predict"]({"markets": "not a list"})


class E2ESubprocessTests(unittest.TestCase):
    """Spawn the actual `python3 -m polyrocket_sidecar` and verify it.

    These use the Rust wire format: lowercase methods, {"markets": [...]} params.
    """

    @classmethod
    def setUpClass(cls) -> None:
        cls.proc = subprocess.Popen(
            [sys.executable, "-m", "polyrocket_sidecar"],
            cwd=str(ROOT),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )

    @classmethod
    def tearDownClass(cls) -> None:
        if cls.proc.poll() is None:
            cls.proc.terminate()
            try:
                cls.proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                cls.proc.kill()

    def _round_trip(self, payload: str) -> dict:
        assert self.proc.stdin is not None and self.proc.stdout is not None
        self.proc.stdin.write(payload + "\n")
        self.proc.stdin.flush()
        line = self.proc.stdout.readline()
        return json.loads(line)

    def test_e2e_ping(self) -> None:
        out = self._round_trip('{"id": 1, "method": "ping", "params": {}}')
        self.assertEqual(out["id"], 1)
        self.assertTrue(out["result"]["pong"])

    def test_e2e_predict_rust_shape(self) -> None:
        out = self._round_trip(
            json.dumps({
                "id": 2,
                "method": "predict",
                "params": {
                    "markets": [
                        {"market_id": "m1", "price": 0.2, "market_age_hours": 24},
                        {"market_id": "m2", "price": 0.8, "market_age_hours": 24},
                    ]
                },
            })
        )
        self.assertEqual(out["id"], 2)
        self.assertIn("result", out)
        self.assertIn("predictions", out["result"])
        preds = out["result"]["predictions"]
        self.assertEqual(len(preds), 2)
        # m1 (cheap YES) should score higher than m2 (expensive YES)
        m1 = next(p for p in preds if p["market_id"] == "m1")
        m2 = next(p for p in preds if p["market_id"] == "m2")
        self.assertGreater(m1["prob"], m2["prob"])
        # Each prediction has the Rust-expected fields
        for p in preds:
            self.assertIn("market_id", p)
            self.assertIn("prob", p)
            self.assertIn("confidence", p)
            self.assertIn("rationale", p)

    def test_e2e_unknown_method(self) -> None:
        out = self._round_trip('{"id": 3, "method": "what_is_this", "params": {}}')
        self.assertEqual(out["id"], 3)
        # error is now a string with code embedded
        self.assertFalse(out["ok"])
        self.assertIsInstance(out["error"], str)
        self.assertIn("-32601", out["error"])

    def test_e2e_invalid_params(self) -> None:
        # predict with markets="not a list" → -32602
        out = self._round_trip('{"id": 4, "method": "predict", "params": {"markets": "not a list"}}')
        self.assertEqual(out["id"], 4)
        self.assertFalse(out["ok"])
        self.assertIsInstance(out["error"], str)
        self.assertIn("-32602", out["error"])

    def test_e2e_malformed_json(self) -> None:
        out = self._round_trip("not json at all")
        # parse error → id=-1, code=-32700
        self.assertEqual(out["id"], -1)
        self.assertFalse(out["ok"])
        self.assertIn("-32700", out["error"])

    def test_e2e_list_promote_history(self) -> None:
        """v0.19a: list_promote_history is a read-only audit.
        The shape is fixed (ok, entries, count, message) regardless
        of whether any promotes have happened.
        """
        out = self._round_trip(
            '{"id": 5, "method": "list_promote_history", "params": {}}'
        )
        self.assertEqual(out["id"], 5)
        self.assertTrue(out["ok"])
        self.assertIn("result", out)
        result = out["result"]
        self.assertIn("ok", result)
        self.assertIn("entries", result)
        self.assertIn("count", result)
        self.assertIsInstance(result["entries"], list)
        self.assertEqual(result["count"], len(result["entries"]))


if __name__ == "__main__":
    unittest.main()
