import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';

/**
 * L1 —— 全局 UI 偏好（与 DB 中的 user_brief_prefs 不同）。
 * 持久化到 localStorage；纯 UI 状态。
 */
export interface UiPrefs {
  defaultMinEdgePct: number;
  defaultAllocationCapUsdc: number;
  copyTradingEnabled: boolean;
  notificationsEnabled: boolean;
  /** 在 PnL/Lab 页面显示高级统计。 */
  advancedStats: boolean;
  /** v0.23c —— 「若更优则自动晋升」：候选模型
   * 需比当前模型好多少（更低的 Brier）才能自动晋升。
   * 默认 0.005。设为 1.0 即可视作关闭
   * 自动晋升（候选永远不会好 1.0）。 */
  autoPromoteBrierMargin: number;
  /** v0.28c —— 训练后自动晋升：当为 true 时，
   * Rust 的 `train_job` handler 会在一次成功的
   * 训练之后派发一个后台
   * `auto_promote_if_better` worker，并由 L1 监听
   * `auto_promote:finished` 事件以自动刷新
   * 历史面板。默认 false。 */
  autoPromoteAfterTrain: boolean;
  /** v0.39b —— 自动晋升的桌面通知：当为 true 时，
   * 当后台自动晋升完成时 L1 会发送真实的
   * 系统通知（macOS 通知中心 / Windows toast /
   * Linux libnotify）。应用内 toast 仍然
   * 无论此开关都会触发。默认 true
   * （当后台操作完成时，用户通常
   * 希望收到系统通知）。 */
  autoPromoteNotify: boolean;
  /** v0.42e-2 —— 在跳过的自动晋升
   * 运行中也发送通知。跳过路径是常见
   * 情况（大多数训练运行不会超过
   * 配置的 Brier 阈值），因此默认关闭
   * —— 大多数用户不希望每次点 Train
   * 都收到「训练没有提升」的系统
   * 通知。
   *
   * 开启时，L1 会在
   * `auto_promote:finished` 事件中
   * `promoted === false`（例如
   * 「候选没有胜过当前模型」）时发送系统
   * 通知。默认 false。 */
  autoPromoteSkippedNotify: boolean;
  /** v0.42c —— 可选启用的生命周期遥测。当为 true
   * 时，每个 Rust 生命周期事件（训练开始 /
   * 完成 / 失败、晋升完成、scheduler
   * tick 等）都会向 stderr 写一行 NDJSON。
   * 默认 false。用 `polyrocket 2>
   * telemetry.log` 捕获。用户可以在
   * 运行时通过 Settings 中的遥测开关修改
   * —— 修改会通过
   * `setTelemetryEnabled` IPC 推送给 Rust。 */
  telemetryEnabled: boolean;
  /** v0.44c —— 模拟盘模式。当为 true 时，
   * mirror executor 命中的订单会写入
   * `paper_fills` 表而非 `bets`
   * 表，并跳过 CLOB sign_order 步骤。
   * 决策逻辑不变 —— 模拟盘
   * 仅改变写入路径。允许用户在没有
   * 真实资金风险的情况下验证其配置
   * （仓位、风控、频率）。默认 false。 */
  mirrorPaperMode: boolean;
  /** v0.48b —— 模型衰退时发送系统
   * 通知。当第 7 个 scheduler loop
   * 发出 `alert=true`（实时 Brier 比
   * 训练 Brier 高出阈值）的
   * ModelDegradation 事件时，若此开关为 on，
   * L1 会触发系统通知。默认 true。 */
  degradationAlertNotify: boolean;
}

interface PrefsState extends UiPrefs {
  setPref: <K extends keyof UiPrefs>(k: K, v: UiPrefs[K]) => void;
  reset: () => void;
}

const DEFAULT: UiPrefs = {
  defaultMinEdgePct: 5,
  defaultAllocationCapUsdc: 100,
  copyTradingEnabled: false,
  notificationsEnabled: true,
  advancedStats: false,
  autoPromoteBrierMargin: 0.005,
  autoPromoteAfterTrain: false,
  autoPromoteNotify: true,
  autoPromoteSkippedNotify: false,
  telemetryEnabled: false,
  mirrorPaperMode: false,
  degradationAlertNotify: true,
};

export const usePrefsStore = create<PrefsState>()(
  persist(
    (set) => ({
      ...DEFAULT,
      setPref: (k, v) => set({ [k]: v } as Partial<UiPrefs>),
      reset: () => set({ ...DEFAULT }),
    }),
    {
      name: 'polyrocket.prefs',
      storage: createJSONStorage(() => localStorage),
    },
  ),
);
