// Mirror of src-tauri/src/commands/* DTOs (L1 ↔ L2 contract).
// Spec: docs/overview.md §1.2 (L1 → L2 IPC).

export type WalletType = 'eoa' | 'smart';

export interface Wallet {
  id: string;
  address: string;
  label: string | null;
  chain_id: number;
  wallet_type: WalletType;
  created_at: number;
  last_synced_at: number | null;
}

export interface AddWalletArgs {
  address: string;
  label?: string;
  chain_id?: number;
  wallet_type?: WalletType;
}

export const POLYGON_MAINNET = 137;
