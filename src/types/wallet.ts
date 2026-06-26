// src-tauri/src/commands/* DTO 的镜像（L1 ↔ L2 契约）。
// 设计文档:docs/overview.md §1.2（L1 → L2 IPC）。

export type WalletType = 'eoa' | 'smart';

/** L1 Wallet DTO。**镜像** `src-tauri/src/commands/wallet.rs::WalletDto`。
 *
 * **不含私钥**：私钥在 OS keyring，**永不入** SQLite。L1 拿到的只有
 * address + label + chain_id + wallet_type + 时间戳。
 *
 * **`chain_id` 137**（Polygon mainnet）= polyrocket 默认；80002 = Amoy testnet。
 */
export interface Wallet {
  id: string;
  address: string;
  label: string | null;
  chain_id: number;
  wallet_type: WalletType;
  created_at: number;
  last_synced_at: number | null;
}

/** `add_wallet` IPC 参数。
 *
 * **`chain_id` 默认 137**，`wallet_type` 默认 `'eoa'` —— 由 IPC handler
 * `commands/wallet.rs::add_wallet` 用 `unwrap_or` 兜底。
 */
export interface AddWalletArgs {
  address: string;
  label?: string;
  chain_id?: number;
  wallet_type?: WalletType;
}

export const POLYGON_MAINNET = 137;
