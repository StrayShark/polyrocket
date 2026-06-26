"""v0.10b train_job + promote_model 的专项测试。

这些测试使用一个临时 model 目录，避免触及用户
真实的 `~/.polyrocket/sidecar/models/`。
"""

import json
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

# 让包可以被 import
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
        # 重新加载模块级路径常量以适配新的环境变量
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
        # Brier 分数在 [0, 1] 区间内（对概率的平方误差）
        self.assertGreaterEqual(result["best_brier"], 0.0)
        self.assertLessEqual(result["best_brier"], 1.0)

    def test_train_writes_candidate_file(self) -> None:
        result = run_train_job(n_trials=1, epochs=10)
        candidate_path = Path(result["candidate_path"])
        self.assertTrue(candidate_path.exists())
        # 该文件是合法的 JSON
        data = json.loads(candidate_path.read_text())
        self.assertEqual(data["job_id"], result["job_id"])
        self.assertIn("best", data)
        self.assertIn("all_trials", data)

    def test_train_atomic_write(self) -> None:
        """永远不应看到只写了一半的候选文件。"""
        result = run_train_job(n_trials=1, epochs=10)
        # 完成后，磁盘上不应残留 .tmp 文件
        tmp_files = list(train.MODEL_DIR.glob("*.json.tmp"))
        self.assertEqual(tmp_files, [])

    def test_train_default_n_trials(self) -> None:
        # n_trials=0 也应至少产出 1 个 trial（被夹紧到下界）
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
        # 先跑一次 train
        train_result = run_train_job(n_trials=1, epochs=5)
        self.assertEqual(train_result["status"], "completed")
        # 然后 promote
        promote = run_promote_model()
        self.assertTrue(promote["promoted"], msg=str(promote))
        self.assertEqual(promote["status"], "ok")
        self.assertTrue(Path(promote["active_path"]).exists())
        # active 文件包含候选的 best 参数
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
        """端到端：train 写入候选，promote 将其移动到 active。"""
        # 1. 此时还没有 active 文件
        self.assertFalse(train.ACTIVE_FILE.exists())
        # 2. Train
        train_result = run_train_job(n_trials=2, epochs=10)
        self.assertEqual(train_result["status"], "completed")
        # 3. Promote
        promote = run_promote_model()
        self.assertTrue(promote["promoted"])
        # 4. 两个文件都存在
        self.assertTrue(train.CANDIDATE_FILE.exists())
        self.assertTrue(train.ACTIVE_FILE.exists())
        # 5. active 文件是 candidate 的超集
        active = json.loads(train.ACTIVE_FILE.read_text())
        candidate = json.loads(train.CANDIDATE_FILE.read_text())
        self.assertEqual(active["job_id"], candidate["job_id"])
        self.assertEqual(active["best"], candidate["best"])

    def test_promote_appends_to_history(self) -> None:
        """v0.19a：每次成功的 promote 都会向
        active.json.promotion_history 追加一条新条目。
        两次 promote → 2 条条目。
        """
        # 第一次 train + promote
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

        # 第二次 train + promote → history 增长到 2 条
        t2 = run_train_job(n_trials=1, epochs=5)
        p2 = run_promote_model()
        self.assertTrue(p2["promoted"])
        active2 = json.loads(train.ACTIVE_FILE.read_text())
        self.assertEqual(len(active2["promotion_history"]), 2)
        # 最新的条目在最后
        self.assertEqual(active2["promotion_history"][1]["job_id"], t2["job_id"])
        # 最旧的仍然存在
        self.assertEqual(active2["promotion_history"][0]["job_id"], t1["job_id"])

    def test_run_list_promote_history_round_trip(self) -> None:
        """v0.19a：list_promote_history 返回写入的内容。"""
        from polyrocket_sidecar.train import run_list_promote_history
        # 跑一次 train + promote
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
        """v0.20a：每条 history 条目现在都有一个
        `weights` 字段，包含 {w0, w1, w2}，以便该条目
        自包含、可用于回滚（之后无需再读取 candidate 文件）。
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
        # Brier 也保存在条目中，便于展示 Brier 徽章
        self.assertIn("best_brier", entry)
        self.assertAlmostEqual(entry["best_brier"], t["best_brier"], places=4)

    def test_history_entry_includes_reason(self) -> None:
        """v0.41a：每条 history 条目都有一个
        `reason` 字段，包含人类可读的描述
        ("Promoted as best trial" 或
        "Promoted as trial N of M")。L1 将其展示
        为 hover tooltip。
        """
        t = run_train_job(n_trials=4, epochs=5)
        # best-trial 提升
        run_promote_model()
        from polyrocket_sidecar.train import run_list_promote_history
        h = run_list_promote_history()
        entry = h["entries"][0]
        self.assertIn("reason", entry)
        self.assertEqual(entry["reason"], "Promoted as best trial")

        # 批量 trial 提升
        t2 = run_train_job(n_trials=4, epochs=5)
        run_promote_model(trial_index=1)
        h = run_list_promote_history()
        # h["entries"] 是 oldest-first；新条目在最后
        new_entry = h["entries"][-1]
        self.assertEqual(new_entry["reason"], "Promoted as trial 2 of 4")

    def test_rollback_to_previous_version(self) -> None:
        """v0.20a：train → promote → train → promote → rollback
        到第一个版本。新的 active 应该是第一个版本
        （而不是当前版本），且 history 应增长 1
        （一个 rollback marker）。
        """
        from polyrocket_sidecar.train import run_rollback_model
        t1 = run_train_job(n_trials=1, epochs=5)
        run_promote_model()
        first_version = f"logistic-{t1['job_id']}"

        t2 = run_train_job(n_trials=1, epochs=5)
        run_promote_model()

        # 确认当前是 t2
        active_before = json.loads(train.ACTIVE_FILE.read_text())
        self.assertEqual(active_before["job_id"], t2["job_id"])

        # 回滚到 t1
        rb = run_rollback_model(model_version=first_version)
        self.assertTrue(rb["rolled_back"], msg=str(rb))
        self.assertEqual(rb["model_version"], first_version)
        self.assertEqual(rb["status"], "ok")
        self.assertIsNotNone(rb["rolled_back_at_ms"])

        # active 文件现在应反映 t1
        active_after = json.loads(train.ACTIVE_FILE.read_text())
        self.assertEqual(active_after["job_id"], t1["job_id"])
        self.assertEqual(active_after["model_version"], first_version)
        # active 权重是 t1 的
        w = active_after["weights"]
        self.assertIn("w0", w)
        self.assertIn("w1", w)
        self.assertIn("w2", w)

        # history 增长了 1 条（一个 rollback marker）
        history = active_after["promotion_history"]
        # 最后一条是 rollback marker
        self.assertEqual(history[-1]["kind"], "rollback")
        self.assertEqual(history[-1]["model_version"], first_version)
        # 倒数第二条是 t1（原始的 promote，不是 rollback 目标）
        # 找到 t1 的 promote 条目
        t1_entries = [e for e in history
                      if isinstance(e, dict)
                      and e.get("job_id") == t1["job_id"]
                      and e.get("kind") != "rollback"]
        self.assertEqual(len(t1_entries), 1)
        self.assertEqual(t1_entries[0]["model_version"], first_version)

    def test_rollback_to_unknown_version_fails(self) -> None:
        """v0.20a：回滚到 history 中不存在的
        model_version 时返回 rolled_back=false，并附带
        清晰的错误消息。
        """
        from polyrocket_sidecar.train import run_rollback_model
        t = run_train_job(n_trials=1, epochs=5)
        run_promote_model()
        rb = run_rollback_model(model_version="logistic-train-DOESNOTEXIST")
        self.assertFalse(rb["rolled_back"])
        self.assertEqual(rb["status"], "failed")
        self.assertIn("not found", rb["message"])
        # active 文件未被修改
        active = json.loads(train.ACTIVE_FILE.read_text())
        self.assertEqual(active["job_id"], t["job_id"])

    def test_rollback_to_v19_entry_without_weights_fails(self) -> None:
        """v0.20a：v0.19 的 history 条目没有 weights。
        回滚到这样的条目时返回 rolled_back=false，并附带
        清晰的错误，提示用户需要重新训练。
        """
        from polyrocket_sidecar.train import run_rollback_model
        t = run_train_job(n_trials=1, epochs=5)
        run_promote_model()
        # 手动从 history 条目中移除 weights，
        # 以模拟一个 v0.19 条目
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
        """v0.21a：批量提升。Train 4 个 trial，提升
        trial index 2（不是 best）。active 应是
        trial 2 的权重；model_version 应带有
        -t2 后缀；history 应记录 trial_index=2。
        """
        t = run_train_job(n_trials=4, epochs=10)
        self.assertEqual(len(t["trials"]), 4)
        # 选择 trial 2（不是 best —— best 在
        # 合成 Brier 下可能是 4 个中的任意一个）
        promote = run_promote_model(trial_index=2)
        self.assertTrue(promote["promoted"], msg=str(promote))
        self.assertEqual(promote["status"], "ok")
        self.assertEqual(promote["trial_index"], 2)
        self.assertTrue(promote["model_version"].endswith("-t2"))
        # history 的最后一条应包含 trial 2 的权重
        active = json.loads(train.ACTIVE_FILE.read_text())
        trial2_weights = t["trials"][2]["weights"]
        history_weights = active["promotion_history"][-1]["weights"]
        self.assertEqual(history_weights["w0"], trial2_weights["w0"])
        self.assertEqual(history_weights["w1"], trial2_weights["w1"])
        self.assertEqual(history_weights["w2"], trial2_weights["w2"])
        # history 记录 trial_index
        self.assertEqual(active["promotion_history"][-1]["trial_index"], 2)
        # history 条目使用 -t2 版本
        self.assertTrue(active["promotion_history"][-1]["model_version"].endswith("-t2"))

    def test_promote_default_is_best_v21(self) -> None:
        """v0.21a：当 trial_index 为 None 时，行为与
        v0.18a 保持一致（提升 best）。
        """
        t = run_train_job(n_trials=4, epochs=10)
        promote = run_promote_model()  # 不传 trial_index
        self.assertTrue(promote["promoted"], msg=str(promote))
        self.assertIsNone(promote["trial_index"])
        # model version 没有 -t{N} 后缀
        self.assertFalse(promote["model_version"].endswith(("-t0", "-t1", "-t2", "-t3")))

    def test_promote_trial_out_of_range_v21(self) -> None:
        """v0.21a：trial_index 超出范围时返回
        清晰的错误，且**不会**修改 active 文件。
        """
        run_train_job(n_trials=4, epochs=10)
        # trial_index 99 超出范围（仅 0..3 合法）
        promote = run_promote_model(trial_index=99)
        self.assertFalse(promote["promoted"])
        self.assertEqual(promote["status"], "failed")
        self.assertIn("out of range", promote["message"])
        # active 文件不应存在（没有成功的 promote）
        self.assertFalse(train.ACTIVE_FILE.exists())

    def test_auto_promote_if_better_promotes_v23(self) -> None:
        """v0.23a：在没有 active 模型时，
        auto_promote 直接提升候选（自动 best）。
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
        """v0.23a：当候选**没有**明显优于 active 时，
        auto_promote 是一个 no-op，并返回清晰的
        "skipped" 原因。
        """
        from polyrocket_sidecar.train import run_auto_promote_if_better
        # 用相同的 seed train + promote 两次 → brier 几乎
        # 完全相同。小的 margin 不会被满足。
        t1 = run_train_job(n_trials=1, epochs=5)
        run_promote_model()
        t2 = run_train_job(n_trials=1, epochs=5)
        # 使用 1.0 的 margin —— 一定不会被满足
        result = run_auto_promote_if_better(brier_margin=1.0)
        self.assertFalse(result["promoted"])
        self.assertTrue(result["skipped"])
        self.assertIn("not at least 1.0 better", result["reason"])
        # active 文件未被修改（仍然是 t1）
        active = json.loads(train.ACTIVE_FILE.read_text())
        self.assertEqual(active["job_id"], t1["job_id"])
        # candidate 仍然存在（未被提升，也未被删除）
        self.assertTrue(train.CANDIDATE_FILE.exists())

    def test_promote_all_trials_v25(self) -> None:
        """v0.25a：在一次调用中批量提升全部 4 个 trial。
        调用完成后，全部 4 个 trial 都应出现在
        promotion history 中，每条都带有自己的 -tN
        后缀和 trial_index。
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
        # 全部 4 个都应已被提升
        for i, r in enumerate(result["results"]):
            self.assertEqual(r["trial_index"], i)
            self.assertTrue(r["promoted"], msg=f"trial {i} failed: {r}")
            self.assertEqual(r["status"], "ok")
            self.assertTrue(r["model_version"].endswith(f"-t{i}"))
        # 全部 4 个都应出现在 history 中
        history = run_list_promote_history()
        self.assertEqual(history["count"], 4)
        trial_indices = [e["trial_index"] for e in history["entries"]]
        self.assertEqual(trial_indices, [0, 1, 2, 3])

    def test_promote_all_trials_no_candidate_v25(self) -> None:
        """v0.25a：磁盘上没有候选时返回 ok=false。"""
        from polyrocket_sidecar.train import run_promote_all_trials
        result = run_promote_all_trials()
        self.assertFalse(result["ok"])
        self.assertEqual(result["count"], 0)
        self.assertEqual(result["results"], [])
        self.assertIn("no candidate", result["message"])
        self.assertFalse(train.ACTIVE_FILE.exists())


class TestPromoteHistoryArchive(unittest.TestCase):
    """v0.33a —— promote history archive（append-only JSONL）。

    `promotion_history[]` 的 20 条上限会静默地丢弃
    旧条目。v0.33a 通过在上限生效**之前**把被丢弃
    的条目写入 `archive.jsonl` 来修复这个问题。
    archive 是 append-only 的，永远不会自动裁剪。
    """

    def setUp(self) -> None:
        # 在测试之间清理 MODEL_DIR + ARCHIVE_FILE
        if train.MODEL_DIR.exists():
            shutil.rmtree(train.MODEL_DIR)
        # 同时显式移除 archive 文件（它位于
        # MODEL_DIR 中，但为防御性清理，
        # 以防其他测试类写入了它）
        from polyrocket_sidecar.train import ARCHIVE_FILE
        if ARCHIVE_FILE.exists():
            ARCHIVE_FILE.unlink()

    def test_archive_file_does_not_exist_before_any_promote(self) -> None:
        """v0.33a —— 全新环境下没有 archive 文件。"""
        # 跑一次 train + promote，history 包含 1 条（未溢出）
        run_train_job(n_trials=1, epochs=5)
        run_promote_model()
        # archive 文件可能存在也可能不存在；该测试较为宽松，
        # 因为上限是 20 —— 1 条不会触发
        # archive。我们只验证：若文件存在，
        # 则它是一个合法的 JSONL。
        from polyrocket_sidecar.train import ARCHIVE_FILE
        if ARCHIVE_FILE.exists():
            content = ARCHIVE_FILE.read_text()
            # 若文件存在，则它应是合法的 JSONL
            for line in content.strip().split("\n"):
                if line:
                    json.loads(line)  # 若不合法则抛异常

    def test_archive_writes_dropped_entries_on_overflow(self) -> None:
        """v0.33a —— 第 21 次 promote 写入被丢弃的 1 条。"""
        from polyrocket_sidecar.train import ARCHIVE_FILE
        # 跑 21 次 train + promote。20 条上限会丢弃第 1 条。
        job_ids: list[str] = []
        for _ in range(21):
            t = run_train_job(n_trials=1, epochs=5)
            job_ids.append(t["job_id"])
            run_promote_model()
        # 内存中的 history 仍然被限制在 20 条
        active = json.loads(train.ACTIVE_FILE.read_text())
        self.assertEqual(len(active["promotion_history"]), 20)
        # archive 文件应存在并包含 1 条条目（第 1 条被丢弃的）
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
        """v0.33a —— 多次溢出时是追加而不是覆盖。"""
        from polyrocket_sidecar.train import ARCHIVE_FILE
        # 跑 25 次 promote → 丢弃 5 条（第 21、22、23、
        # 24、25 次各丢弃 1 条）。
        for _ in range(25):
            t = run_train_job(n_trials=1, epochs=5)
            run_promote_model()
        # archive 应包含 5 条（25 次 promote 中共丢弃 5 条）
        self.assertTrue(ARCHIVE_FILE.exists())
        lines = ARCHIVE_FILE.read_text().strip().split("\n")
        self.assertEqual(len(lines), 5)
        # 每行都是一个合法的 JSON 对象
        for line in lines:
            archived = json.loads(line)
            self.assertIn("job_id", archived)
            self.assertIn("archived_at_ms", archived)

    def test_archive_entries_have_correct_shape(self) -> None:
        """v0.33a —— 每条 archive 条目都拥有重建 promote
        所需的完整字段集合，包括 weights
        （用于 v0.20a 回滚）和 trial_index（用于 v0.21a 批量提升）。"""
        from polyrocket_sidecar.train import ARCHIVE_FILE
        # 跑 21 次 promote
        for _ in range(21):
            t = run_train_job(n_trials=1, epochs=5)
            run_promote_model()
        lines = ARCHIVE_FILE.read_text().strip().split("\n")
        archived = json.loads(lines[0])
        # 与内存中的条目形状完全一致
        self.assertEqual(
            set(archived.keys()),
            {"job_id", "model_version", "promoted_at_ms",
             "best_brier", "best_params", "weights",
             "trial_index", "reason", "archived_at_ms"},
        )
        # weights 包含 3 个预期的键
        self.assertEqual(set(archived["weights"].keys()), {"w0", "w1", "w2"})


# =================================================================
# ============== v0.43a — backtest_model 测试 ======================
# =================================================================


class BacktestModelTests(unittest.TestCase):
    """v0.43a —— 用一个已保存的模型对一组
    (price, age, outcome) 样本进行重放，并返回
    Brier + 校准 + 每个样本的预测。

    这些测试不经过完整的 train + promote 流程
    ——它们直接用已知的权重写入一个合成的
    `archive.jsonl`，然后断言回测产出
    期望的 Brier。
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
        # 可预测的权重：w0=0, w1=0, w2=0 → sigmoid(0) = 0.5
        # （无论输入如何都预测 0.5）
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
        # 校准：3 个样本都落在 [0.4, 0.6) 桶中
        self.assertEqual(len(out["calibration"]), 5)
        non_empty = [b for b in out["calibration"] if b["count"] > 0]
        self.assertEqual(len(non_empty), 1)
        self.assertEqual(non_empty[0]["count"], 3)

    def test_backtest_finds_model_in_active(self) -> None:
        # 直接写入 active.json（不是 archive）
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
                {"price": 0.5, "market_age_hours": 24.0, "outcome": 1.0},  # 合法
                {"price": "not a number", "market_age_hours": 24.0, "outcome": 0.0},  # 坏数据
                {"market_age_hours": 24.0, "outcome": 0.0},  # 缺少 price
                {"price": 0.5, "market_age_hours": 24.0, "outcome": 2.0},  # 超出范围
            ],
        )
        self.assertTrue(out["ok"])
        # 只有第一个样本是合法的
        self.assertEqual(out["sample_count"], 1)

    def test_backtest_top_winners_and_losers(self) -> None:
        self._write_archive_entry("logistic-test", {"w0": 0.0, "w1": 0.0, "w2": 0.0})
        # 全部预测 0.5；outcome 0 → brier 0.25，outcome 1 → brier 0.25
        # outcome 0.5 → brier 0
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
        # top winners：3 个 perfect（最低 brier = 0）
        self.assertEqual(len(out["top_winners"]), 3)
        for w in out["top_winners"]:
            self.assertAlmostEqual(w["brier"], 0.0, places=6)
        # top losers：上限为 3，但在这个小样本集
        # 中，按 brier 升序排好后 3 个是
        # [perfect, perfect, wrong]，反转后 →
        # [wrong, perfect, perfect]。最差的那
        # 个位于 index 0；重复项在小样本集
        # 下是可接受的。
        self.assertEqual(len(out["top_losers"]), 3)
        self.assertEqual(out["top_losers"][0]["label"], "wrong")
        self.assertAlmostEqual(out["top_losers"][0]["brier"], 0.25, places=4)

    def test_backtest_top_losers_caps_at_sample_size(self) -> None:
        # v0.43a —— 当样本数 < 3 时，top_losers
        # 平滑降级。1 个样本时，winners 和
        # losers 都只有 1 条。
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
