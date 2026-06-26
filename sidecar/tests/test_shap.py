"""v0.59 SHAP（KernelExplainer）实现的测试。

覆盖：
  1. _predict_logistic 与 predict._sigmoid 一致
  2. _kernel_weight 的闭式
  3. _build_coalitions 枚举了所有 2^M 个 mask
  4. _fit_weighted_ls 能恢复出已知线性函数的梯度
  5. run_shap_explainability：efficiency 公理
     （Σφ_i ≈ f(x) - E[f(x)]，在浮点容差内）
  6. run_shap_explainability：错误路径
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
        """v0.59 —— _predict_logistic 应与
        predict._sigmoid 一致。它们都是数值稳定的
        sigmoid 实现；我们对若干 (w, x) 组合进行断言。"""
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
        """v0.59 —— w(z) = (M-1) / (C(M,|z|) * |z| *
        (M-|z|))。我们测试特殊情况
        （空 + 完整 coalition = 大常数）以及
        M=3 的中间情况。"""
        # 空 coalition：|z|=0 → 大常数
        self.assertEqual(_kernel_weight(3, 0), 1.0e6)
        # 完整 coalition：|z|=M → 大常数
        self.assertEqual(_kernel_weight(3, 3), 1.0e6)
        # 半 coalition：M=3 中 |z|=1
        # binom(3, 1) = 3, |z|=1, M-|z|=2
        # w = 2 / (3 * 1 * 2) = 1/3
        self.assertAlmostEqual(_kernel_weight(3, 1), 1.0 / 3.0, places=10)
        # M=3 中 |z|=2：由对称性等于 |z|=1
        self.assertAlmostEqual(_kernel_weight(3, 2), 1.0 / 3.0, places=10)

    def test_build_coalitions_enumerates_all(self):
        """v0.59 —— _build_coalitions 必须返回
        所有 2^M 个二元 mask。"""
        for m in [1, 2, 3, 4]:
            coalitions = _build_coalitions(m)
            self.assertEqual(len(coalitions), 2**m)
            # 每个 mask 都是 m 个 0/1 int 的列表。
            for c in coalitions:
                self.assertEqual(len(c), m)
                self.assertTrue(all(b in (0, 1) for b in c))
            # 不能有重复。
            self.assertEqual(
                len({tuple(c) for c in coalitions}),
                2**m,
            )

    def test_solve_linear(self):
        """v0.59 —— 对 LS 求解器的健全性检查。"""
        # 2 个未知数，3 个方程（超定）。
        # x + 2y = 5, 2x - y = 0, x + y = 3 → x=1, y=2。
        a = [[1.0, 2.0], [2.0, -1.0], [1.0, 1.0]]
        b = [5.0, 0.0, 3.0]
        x = _solve_linear(a, b)
        self.assertAlmostEqual(x[0], 1.0, places=6)
        self.assertAlmostEqual(x[1], 2.0, places=6)

    def test_fit_weighted_ls_recovers_gradient(self):
        """v0.59 —— 对于已知线性函数
        f(x) = a + b*x0 + c*x1 + d*x2，拟合出的
        SHAP 系数应满足：
          - phi[0]（"bias" / E[f(x)] 项）
            = f(background) = a + b*bg[0]
          - phi[1..M]（每特征贡献）
            = (b, c, d) —— 按特征差 (x_target - bg)
            缩放后的梯度
        """
        a, b, c, d = 1.0, 2.0, -3.0, 0.5
        # 为 M=3 构造 8 个 coalition，并在每个
        # imputed 输入上求 f。
        m = 3
        coalitions = _build_coalitions(m)
        # 对所有特征使用 background = (1.0, 0.0, 0.0)。
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
        # 拟合：X 的列为 [1, mask[0], mask[1], mask[2]]
        # 返回 4 个系数：E[f(x)]（baseline），
        # 然后是逐特征贡献 = w_i * (x_target_i - bg_i)。
        phi = _fit_weighted_ls(coalitions, outputs, weights, m)
        # Baseline = f(bg) = a + b*1 + c*0 + d*0 = 3
        expected_baseline = a + b * bg[0] + c * bg[1] + d * bg[2]
        self.assertAlmostEqual(phi[0], expected_baseline, places=4)
        # 逐特征：w * (x - bg)
        self.assertAlmostEqual(phi[1], b * (x_target[0] - bg[0]), places=4)
        self.assertAlmostEqual(phi[2], c * (x_target[1] - bg[1]), places=4)
        self.assertAlmostEqual(phi[3], d * (x_target[2] - bg[2]), places=4)
        # Efficiency：所有特征贡献之和
        # = f(x_target) - f(bg) = (1+4-12+2) - (1+2)
        # = -5 - 3 = -8。验证一下。
        self.assertAlmostEqual(
            sum(phi[1:]),
            (a + b*x_target[0] + c*x_target[1] + d*x_target[2])
            - expected_baseline,
            places=4,
        )


class TestShapEndToEnd(unittest.TestCase):
    """端到端测试：写入一个真实的
    active.json（匹配 v0.12+ 的 schema），
    并对其运行 run_shap_explainability。
    我们用一个很小的临时目录保存模型文件，
    以免触及真实的 ~/.polyrocket/sidecar/models/。"""

    def setUp(self):
        import tempfile
        from polyrocket_sidecar.train import ACTIVE_FILE, MODEL_DIR
        self.tmp = tempfile.TemporaryDirectory()
        self.env = {"POLYROCKET_HOME": self.tmp.name}
        # polyrocket 的 train.py 使用硬编码的
        # ~/.polyrocket/sidecar/models/ 路径。我们
        # monkey-patch MODEL_DIR 指向我们的临时目录，
        # 效果持续整个测试期间。
        import polyrocket_sidecar.train as train_mod
        self._orig_model_dir = train_mod.MODEL_DIR
        train_mod.MODEL_DIR = Path(self.tmp.name) / "models"
        train_mod.MODEL_DIR.mkdir(parents=True, exist_ok=True)
        train_mod.ACTIVE_FILE = train_mod.MODEL_DIR / "active.json"
        # 重置 active 缓存。
        from polyrocket_sidecar.active import reset_cache
        reset_cache()
        # 写入一个最小的 active.json。
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
        """v0.59 —— Σφ_i = f(x) - E[f(x)] 在浮点
        容差内成立。这是 SHAP efficiency 公理；
        如果不成立，则回归没有收敛。"""
        r = run_shap_explainability(
            model_version="logistic-test-shap",
            sample={"price": 0.5, "market_age_hours": 24.0},
        )
        self.assertTrue(r["ok"], r.get("message"))
        self.assertEqual(r["method"], "kernel_shap")
        # Efficiency：Σφ_i ≈ f(x) - E[f(x)]
        # （efficiency_diff 是拟合后的残差；应接近 0）。
        self.assertIsNotNone(r["efficiency_diff"])
        self.assertAlmostEqual(
            r["efficiency_diff"], 0.0, places=4,
            msg=f"efficiency diff {r['efficiency_diff']} too large",
        )
        # 健全性：返回了 3 个特征。
        self.assertEqual(len(r["features"]), 3)
        # 按 abs_shap 降序排列。
        for i in range(len(r["features"]) - 1):
            self.assertGreaterEqual(
                r["features"][i]["abs_shap"],
                r["features"][i + 1]["abs_shap"],
            )

    def test_efficiency_holds_at_extremes(self):
        """v0.59 —— efficiency 公理应在输入空间的
        角点（price=0、price=1、age=0、age 很大）处成立。"""
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
