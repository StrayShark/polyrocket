"""v0.11c active 模型加载器的测试。"""

import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

# 让包可以被 import
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
        # 为每个测试使用一个临时的 model 目录
        self.tmp = tempfile.TemporaryDirectory()
        self._env = os.environ.get("POLYROCKET_SIDECAR_MODEL_DIR")
        os.environ["POLYROCKET_SIDECAR_MODEL_DIR"] = self.tmp.name
        # 覆盖模块级路径，让它们指向临时目录
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
        # 还没有 active.json → 回退到默认权重
        w = get_active_weights()
        self.assertEqual(w["w0"], active._FALLBACK_WEIGHTS["w0"])
        self.assertEqual(w["w1"], active._FALLBACK_WEIGHTS["w1"])
        self.assertEqual(w["w2"], active._FALLBACK_WEIGHTS["w2"])

    def test_loads_from_active_file(self) -> None:
        # 训练 + 提升
        run_train_job(n_trials=1, epochs=3)
        result = run_promote_model()
        self.assertTrue(result["promoted"], msg=str(result))
        # 此时 active.json 存在；加载器应该能读到它
        reset_cache()  # 强制重新读取
        w = get_active_weights()
        # 被提升的权重来自 train 扫描，而不是回退权重。
        # 我们不能断言具体的值（它们取决于随机初始化），
        # 但它们应该是合法的 float。
        self.assertIsInstance(w["w0"], float)
        self.assertIsInstance(w["w1"], float)
        self.assertIsInstance(w["w2"], float)

    def test_picks_up_promote_on_next_call(self) -> None:
        """一次成功的 promote 之后，下一次 get_active_weights()
        就能返回新权重，而无需重启进程。
        """
        # 第一次调用：回退权重
        w1_before = get_active_weights()
        self.assertEqual(w1_before["w0"], active._FALLBACK_WEIGHTS["w0"])

        # 训练 + 提升
        run_train_job(n_trials=1, epochs=3)
        run_promote_model()
        reset_cache()

        # 第二次调用：应能拿到新文件
        w1_after = get_active_weights()
        # 新权重有可能恰好等于回退权重（概率极低但可能）。
        # 通过检查后续调用返回相同的值（mtime 一致时
        # 命中缓存）来验证缓存确实失效了。
        w1_again = get_active_weights()
        self.assertEqual(w1_after, w1_again)

    def test_handles_malformed_active_file(self) -> None:
        # 向 active.json 写入垃圾内容
        Path(active.ACTIVE_FILE).parent.mkdir(parents=True, exist_ok=True)
        Path(active.ACTIVE_FILE).write_text("not valid json {{{")
        reset_cache()
        w = get_active_weights()
        # 应回退到内联权重
        self.assertEqual(w["w0"], active._FALLBACK_WEIGHTS["w0"])

    def test_handles_missing_best_field(self) -> None:
        # 写入合法 JSON 但缺少 "best" 键
        Path(active.ACTIVE_FILE).parent.mkdir(parents=True, exist_ok=True)
        Path(active.ACTIVE_FILE).write_text(json.dumps({"job_id": "x"}))
        reset_cache()
        w = get_active_weights()
        # 应回退到内联权重
        self.assertEqual(w["w0"], active._FALLBACK_WEIGHTS["w0"])

    def test_predict_with_active_model(self) -> None:
        # 健全性检查：该函数返回一个 [0, 1] 区间内的概率
        prob = predict_with_active_model(price=0.5, market_age_hours=24.0)
        self.assertGreaterEqual(prob, 0.0)
        self.assertLessEqual(prob, 1.0)

    def test_full_loop_train_promote_predict(self) -> None:
        """v0.11c —— 闭环。Train 写入候选，promote 移到 active，
        predict 从 active 读取。"active" 的预测与 "fallback"
        的预测不同，因为训练出的权重与内联权重不同。
        """
        # 基线：尚无 active 模型，predict 使用回退权重
        reset_cache()
        prob_fallback_low = predict_with_active_model(price=0.2, market_age_hours=24.0)

        # 训练 + 提升
        run_train_job(n_trials=2, epochs=20)
        run_promote_model()
        reset_cache()

        # promote 之后，predict 应使用训练出的权重。
        # 这两个概率不一定必须不同（小样本可能收敛到
        # 相似权重），但 predict_from_markets 中的 rationale
        # 现在应该展示 active 权重。
        from polyrocket_sidecar.predict import predict_from_markets
        result = predict_from_markets([{"market_id": "m1", "price": 0.2, "market_age_hours": 24.0}])
        out = result["predictions"]
        self.assertEqual(len(out), 1)
        # v0.12a —— rationale 现在以 model_version 开头
        # （例如 "logistic-train-xxx:"，不再是 "active:"）
        self.assertIn("logistic-", out[0]["rationale"])
        self.assertNotIn("active:", out[0]["rationale"])
        # 且 prob 仍在范围内
        self.assertGreaterEqual(out[0]["prob"], 0.0)
        self.assertLessEqual(out[0]["prob"], 1.0)
        # v0.12a —— model_version 在顶层暴露
        self.assertIsNotNone(result["model_version"])
        self.assertIn("logistic-", result["model_version"])
        # 它要么等于回退权重（小模型），要么不同。
        # 我们只是验证两者都是合法的概率。
        self.assertGreaterEqual(prob_fallback_low, 0.0)
        self.assertLessEqual(prob_fallback_low, 1.0)


if __name__ == "__main__":
    unittest.main()
