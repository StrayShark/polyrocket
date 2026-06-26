"""侧车（sidecar）的测试。

运行方式：
    cd sidecar && python3 -m unittest tests.test_sidecar -v

我们使用 unittest（而不是 pytest），这样侧车运行时零依赖。
"""

import json
import os
import subprocess
import sys
import unittest
from pathlib import Path

# 当直接运行 tests/ 时，让包可被 import
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
        # 注意：wire 格式使用小写方法名以匹配 Rust。
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
        # Rust 的 SidecarResponse.error 是 Option<String>，所以我们
        # 把错误码嵌入到消息文本中，以字符串形式序列化。
        self.assertIsInstance(obj["error"], str)
        self.assertIn("-32601", obj["error"])
        self.assertIn("nope", obj["error"])

    def test_serialize_includes_ok_for_rust(self) -> None:
        # Rust 的 `parse_line` 区分请求（带 `method`）和
        # 响应（带 `ok`）。如果响应缺少 `ok`，Rust 端会拒绝。
        # 本测试防止它被意外移除。
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
    def setUp(self) -> None:
        # v0.20d —— 预先存在的测试隔离修复。之前的测试
        # （TrainJobTests、E2ESubprocessTests）可能写过
        # active.json 文件。PredictTests 在某些测试中
        # 期望"没有 active 模型"的状态（model_version =
        # "logistic-0.1.0"）。清除文件 + 重置缓存，可以
        # 给每个 Predict 测试一个干净的起点。
        from polyrocket_sidecar.active import ACTIVE_FILE as _ACTIVE_FILE, reset_cache
        if _ACTIVE_FILE.exists():
            _ACTIVE_FILE.unlink()
        reset_cache()

    def test_zero_price_zero_age_near_baseline(self) -> None:
        # price=0（最便宜）+ age=0 → 高概率
        p = predict_logic(price=0.0, market_age_hours=0.0)
        self.assertGreater(p, 0.5)

    def test_high_price(self) -> None:
        # price=1（最贵）→ 低概率
        p = predict_logic(price=1.0, market_age_hours=24.0)
        self.assertLess(p, 0.5)

    def test_probability_bounded(self) -> None:
        for price in (0.0, 0.1, 0.5, 0.9, 1.0):
            for age in (0, 1, 24, 168, 1000):
                p = predict_logic(price=price, market_age_hours=age)
                self.assertGreaterEqual(p, 0.0)
                self.assertLessEqual(p, 1.0)

    def test_predict_from_markets_rust_shape(self) -> None:
        """输出**必须**匹配 Rust 端
        `domain::lab::sidecar::parse_predict_response` 期望的结构：
        { predictions: [{ market_id, prob, confidence, rationale }],
          model_version: "logistic-..." }。
        v0.12a —— model_version 被提升到顶层。
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
        # v0.12a —— model_version 应是一个非空字符串
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
        # 默认 price = 0.5 → confidence = 0
        self.assertEqual(out[0]["confidence"], 0.0)

    def test_returns_model_version_field(self) -> None:
        """v0.12a —— predict_from_markets 在顶层返回一个
        `model_version` 字段。默认值（没有 active 模型）为
        'logistic-0.1.0'。
        """
        from polyrocket_sidecar.active import reset_cache
        reset_cache()
        result = predict_from_markets([{"market_id": "m1", "price": 0.5}])
        self.assertEqual(result["model_version"], "logistic-0.1.0")


class BenchTests(unittest.TestCase):
    """v0.11d —— 热路径足够快。

    我们不对精确时间做断言（CI 主机各异），只断言
    1 万个市场在开发机上 < 500ms 完成。如果热路径
    出现回归（例如 import 未上提、意外的全局重新读取），
    那么耗时就会远超 1s。
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
        # v0.12a —— result 现在是带 `predictions` 数组的 dict
        self.assertEqual(len(result["predictions"]), 10_000)
        self.assertLess(elapsed, 0.5, f"predict took {elapsed:.3f}s; expected <0.5s")


class DispatchTests(unittest.TestCase):
    def test_all_methods_registered(self) -> None:
        # 如果在 Rust 端添加了方法但没有在这里注册，
        # 本测试可以捕获到。需要与 Rust 端
        # `SidecarMethod` 枚举中的集合保持一致。
        expected = {"ping", "predict", "train_job", "promote_model", "list_promote_history", "rollback_model", "auto_promote_if_better", "promote_all_trials", "backtest_model", "explain_model", "shap_explain"}
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
        """v0.10b：train_job 运行一次真实的扫描并写入候选文件。"""
        out = DISPATCH["train_job"]({})
        self.assertEqual(out["status"], "completed", msg=f"train failed: {out}")
        self.assertIn("job_id", out)
        self.assertIn("best_brier", out)
        self.assertIn("best_params", out)
        self.assertIn("candidate_path", out)
        # 已尝试的 trial
        self.assertGreaterEqual(len(out.get("trials", [])), 1)
        # 候选文件现在应该已经存在于磁盘上
        from pathlib import Path
        self.assertTrue(Path(out["candidate_path"]).exists())

    def test_promote_model_real(self) -> None:
        """v0.10b：promote_model 需要先有候选；没有时
        返回 ok=false 并附带清晰的错误消息。
        """
        # 没有磁盘上的候选时 promote 应干净失败
        # （model_dir 可能留有 test_train_job_real 的痕迹
        # ——任意一种结果都可以接受，只要验证响应结构）。
        out = DISPATCH["promote_model"]({})
        if out.get("promoted"):
            self.assertEqual(out["status"], "ok")
            self.assertIn("active_path", out)
            self.assertIn("promoted_at_ms", out)
        else:
            # 也可能是没有候选（干净的测试环境）—— 两种都合法
            self.assertEqual(out["status"], "failed")

    def test_list_promote_history_empty_or_populated(self) -> None:
        """v0.19a：list_promote_history 返回 ok=true，其内容
        就是 active.json 中的内容。如果之前的测试做过 promote，
        可能会得到条目；否则响应为空 + 一条友好的提示。
        """
        out = DISPATCH["list_promote_history"]({})
        self.assertTrue(out["ok"])
        self.assertIn("entries", out)
        self.assertIn("count", out)
        self.assertIsInstance(out["entries"], list)
        self.assertEqual(out["count"], len(out["entries"]))
        # 每条条目（若存在）都包含审计相关字段
        for e in out["entries"]:
            self.assertIn("job_id", e)
            self.assertIn("model_version", e)
            self.assertIn("promoted_at_ms", e)
            # best_brier 和 best_params 是可选的
            # （某些条目中 best_params 可能为 null）

    def test_predict_rejects_non_list_markets(self) -> None:
        with self.assertRaises(ValueError):
            DISPATCH["predict"]({"markets": "not a list"})


class E2ESubprocessTests(unittest.TestCase):
    """启动真正的 `python3 -m polyrocket_sidecar` 并验证它。

    这些测试使用 Rust 的 wire 格式：小写方法名、
    {"markets": [...]} 参数。
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
        # m1（便宜的 YES）应当比 m2（贵的 YES）得分高
        m1 = next(p for p in preds if p["market_id"] == "m1")
        m2 = next(p for p in preds if p["market_id"] == "m2")
        self.assertGreater(m1["prob"], m2["prob"])
        # 每个预测都包含 Rust 端期望的字段
        for p in preds:
            self.assertIn("market_id", p)
            self.assertIn("prob", p)
            self.assertIn("confidence", p)
            self.assertIn("rationale", p)

    def test_e2e_unknown_method(self) -> None:
        out = self._round_trip('{"id": 3, "method": "what_is_this", "params": {}}')
        self.assertEqual(out["id"], 3)
        # error 现在是带嵌入错误码的字符串
        self.assertFalse(out["ok"])
        self.assertIsInstance(out["error"], str)
        self.assertIn("-32601", out["error"])

    def test_e2e_invalid_params(self) -> None:
        # predict 时 markets="not a list" → -32602
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
        """v0.19a：list_promote_history 是一个只读审计。
        其结构固定（ok、entries、count、message），
        无论是否发生过任何 promote。
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

    def test_e2e_rollback_model_requires_model_version(self) -> None:
        """v0.20a：rollback_model 缺少 model_version 时返回
        一个干净错误（rolled_back=false 并附带消息）。
        """
        out = self._round_trip(
            '{"id": 6, "method": "rollback_model", "params": {}}'
        )
        self.assertEqual(out["id"], 6)
        self.assertTrue(out["ok"])
        result = out["result"]
        self.assertFalse(result["rolled_back"])
        self.assertEqual(result["status"], "failed")
        self.assertIn("model_version", result["message"])

    def test_e2e_rollback_model_not_in_history(self) -> None:
        """v0.20a：回滚到 promotion_history 中不存在的
        model_version 时返回清晰的错误。
        先 promote 一个真实模型，使 active.json 存在，
        然后再尝试回滚到一个不存在的版本。
        """
        # 首先，train + promote 以获得一个 active 模型
        self._round_trip('{"id": 71, "method": "train_job", "params": {"n_trials": 1, "epochs": 5}}')
        self._round_trip('{"id": 72, "method": "promote_model", "params": {}}')
        # 现在回滚到一个不存在的版本
        out = self._round_trip(
            '{"id": 73, "method": "rollback_model", "params": {"model_version": "logistic-train-NOPE"}}'
        )
        self.assertEqual(out["id"], 73)
        self.assertTrue(out["ok"])
        result = out["result"]
        self.assertFalse(result["rolled_back"])
        self.assertIn("not found", result["message"])

    def test_e2e_auto_promote_if_better_no_active(self) -> None:
        """v0.23a：在没有 active 模型的情况下，
        auto_promote_if_better 直接提升候选（它自动是 best）。
        """
        # 先 train 以得到候选
        self._round_trip('{"id": 81, "method": "train_job", "params": {"n_trials": 1, "epochs": 5}}')
        out = self._round_trip(
            '{"id": 82, "method": "auto_promote_if_better", "params": {"brier_margin": 0.005}}'
        )
        self.assertEqual(out["id"], 82)
        self.assertTrue(out["ok"])
        result = out["result"]
        self.assertTrue(result["promoted"])
        self.assertFalse(result["skipped"])
        self.assertIsNone(result["active_brier"])
        self.assertIn("no active model", result["reason"])

    def test_e2e_auto_promote_if_better_skipped(self) -> None:
        """v0.23a：当候选没有明显优于 active 时，
        该调用 no-op，并返回清晰的 "skipped" 原因。
        """
        # 先 train + promote → active
        self._round_trip('{"id": 91, "method": "train_job", "params": {"n_trials": 1, "epochs": 5}}')
        self._round_trip('{"id": 92, "method": "promote_model", "params": {}}')
        # 第二次 train → 新的候选（brier 很可能相似）
        self._round_trip('{"id": 93, "method": "train_job", "params": {"n_trials": 1, "epochs": 5}}')
        # 用一个非常大的 margin 自动 promote（永远不会被满足）
        out = self._round_trip(
            '{"id": 94, "method": "auto_promote_if_better", "params": {"brier_margin": 1.0}}'
        )
        self.assertEqual(out["id"], 94)
        self.assertTrue(out["ok"])
        result = out["result"]
        self.assertFalse(result["promoted"])
        self.assertTrue(result["skipped"])
        self.assertIn("not at least 1.0 better", result["reason"])
        self.assertEqual(result["margin"], 1.0)

    def test_e2e_auto_promote_if_better_invalid_margin(self) -> None:
        """v0.23a：负的 brier_margin 返回一个干净错误。"""
        out = self._round_trip(
            '{"id": 95, "method": "auto_promote_if_better", "params": {"brier_margin": -0.01}}'
        )
        self.assertEqual(out["id"], 95)
        self.assertTrue(out["ok"])
        result = out["result"]
        self.assertFalse(result["promoted"])
        self.assertTrue(result["skipped"])
        self.assertIn("non-negative", result["reason"])

    def test_e2e_promote_all_trials_no_candidate(self) -> None:
        """v0.25a：磁盘上没有候选时，返回 ok=false 且
        results 为空，并附带清晰消息。
        在 e2e 测试中无法保证完全干净的状态
        （它们共享 `~/.polyrocket/sidecar/models/`），因此
        我们只检查响应结构——如果恰好存在候选，
        响应将包含 promoted=true 的条目（同样是合法行为）。
        """
        out = self._round_trip(
            '{"id": 101, "method": "promote_all_trials", "params": {}}'
        )
        self.assertEqual(out["id"], 101)
        self.assertTrue(out["ok"])
        result = out["result"]
        # 结构始终是 {ok, count, results, message}
        self.assertIn("ok", result)
        self.assertIn("count", result)
        self.assertIn("results", result)
        self.assertIsInstance(result["results"], list)
        self.assertEqual(result["count"], len(result["results"]))
        # 如果存在候选，所有结果都应该是已提升的。
        # 如果不存在，则消息中应提及 "no candidate"。
        if not result["ok"]:
            self.assertIn("no candidate", result["message"])
        else:
            for r in result["results"]:
                self.assertIn("trial_index", r)
                self.assertIn("model_version", r)
                self.assertIn("promoted", r)


if __name__ == "__main__":
    unittest.main()
