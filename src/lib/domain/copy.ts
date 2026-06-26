/**
 * L1 领域镜像 —— M5 跟单。
 * 镜像 src-tauri/src/domain/copy/mod.rs。
 */

/** 一个被跟踪的 copy target（whale 地址）。L1 「Copy」面板展示用。
 *
 * **驼峰命名**：L1 convention 跟 Rust DTO 不一样（`minEdge` vs `min_edge`），
 * 转换由 ipc.ts wrapper 做。
 */
export interface CopyTargetInput {
  id: string;
  address: string;
  label?: string | null;
  enabled: boolean;
  allocationCap?: string | null;
  minEdge: number;
  createdAt: number;
}

/** 检测到的一次 whale 成交事件。L1 「Copy → Recent events」列表用。
 *
 * **`txHash` 唯一标识一次链上成交**：可能匹配多个 target（dedup by tx）。
 */
export interface CopyEventInput {
  id: number;
  targetId: string;
  marketId: string;
  detectedAt: number;
  side: string;
  size: string;
  price: number;
  txHash: string;
  matchedBetId?: string | null;
}

/** `shouldMirror` 的返回值。L1 「Mirror queue」面板 / Settings → Copy 用。
 *
 * **`flip = true`**：model 跟 whale 方向相反。L1 仍会按 model edge mirror，但
 * UI 可以用这个 flag 渲染「⚠️ model 跟 whale 方向相反」标签。
 */
export interface MirrorDecision {
  side: 'YES' | 'NO';
  size: string;
  flip: boolean;
}

/** Copy target 校验错误类型。L1 在表单提交时用 `instanceof CopyValidationError`
 * 区分「用户输入错」vs「系统错误」（前者显示在表单 field，后者显示 toast）。
 */
export class CopyValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'CopyValidationError';
  }
}

/** L1 校验 copy target 输入。**镜像** Rust 端 `domain::copy::validate_target_args`。
 *
 * **抛 `CopyValidationError`** 而不是返回 `{ ok, err }`：L1 form 提交路径用
 * try/catch，校验失败 → 红字显示在对应 field。
 *
 * @throws CopyValidationError 当 address / min_edge / allocation_cap 不合法
 */
export function validateTargetArgs(
  address: string,
  minEdge?: number,
  allocationCap?: string,
): void {
  if (!address.startsWith('0x') || address.length !== 42) {
    throw new CopyValidationError('address must be 0x + 40 hex chars');
  }
  if (minEdge !== undefined && (minEdge < 0 || minEdge > 1)) {
    throw new CopyValidationError(`min_edge ${minEdge} out of [0, 1]`);
  }
  if (allocationCap !== undefined && allocationCap !== '') {
    const n = Number(allocationCap);
    if (!Number.isFinite(n)) {
      throw new CopyValidationError(`allocation_cap not a number: ${allocationCap}`);
    }
    if (n < 0) {
      throw new CopyValidationError('allocation_cap cannot be negative');
    }
  }
}

/** 决定是否 mirror 一次 fill。**L1 纯函数版本**（mirror queue 在 L1 预览时也用）。
 *
 * **5 条 return null 条件**：
 *   1. target.enabled = false
 *   2. |edge| < minEdge
 *   3. fillSize 不是有限数
 *   4. fillSize ≤ 0
 *   5. (任何中间 catch-all)
 *
 * @returns MirrorDecision 或 null（不 mirror）
 */
export function shouldMirror(
  target: CopyTargetInput,
  fillSide: string,
  fillSize: string,
  targetMarketEdge: number,
): MirrorDecision | null {
  if (!target.enabled) return null;
  if (Math.abs(targetMarketEdge) < target.minEdge) return null;
  const cap = target.allocationCap
    ? Number(target.allocationCap)
    : Number.POSITIVE_INFINITY;
  const fillSizeN = Number(fillSize);
  if (!Number.isFinite(fillSizeN) || fillSizeN <= 0) return null;
  const size = Math.min(fillSizeN, cap);
  const mirrorSide: 'YES' | 'NO' = targetMarketEdge > 0 ? 'YES' : 'NO';
  return {
    side: mirrorSide,
    size: String(size),
    flip: fillSide.toUpperCase() !== mirrorSide,
  };
}

/** 检查 `txHash` 是否已在 `events` 列表里（O(n) 线性扫描）。
 *
 * **为什么不用 Set**：events 列表是按 `detected_at DESC` 排序的「最近 N 条」
 * 窗口（典型 N=50），Set 反而要全部 load + hash。
 *
 * @returns true 表示「已存在，不应重复 push」
 */
export function isDuplicateTx(events: CopyEventInput[], txHash: string): boolean {
  return events.some((e) => e.txHash === txHash);
}
