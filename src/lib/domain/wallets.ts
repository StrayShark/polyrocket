/**
 * L1 领域镜像 —— M4 钱包。
 * 镜像 src-tauri/src/domain/wallet/mod.rs。
 */

export const POLYGON_MAINNET = 137;
export const SUPPORTED_CHAINS: readonly number[] = [137, 80002];

export type WalletType = 'eoa' | 'smart';

export function parseWalletType(s: string): WalletType {
  if (s === 'eoa' || s === 'smart') return s;
  throw new Error(`unknown wallet type: ${s}`);
}

export class WalletValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'WalletValidationError';
  }
}

/** 校验 EVM 地址。**L1 镜像 Rust 端 `domain::wallet::validate_address`**。
 *
 * **3 条规则**：
 *   1. 必须以 `0x` 开头
 *   2. 总长 42 字符（0x + 40 hex）
 *   3. 后 40 字符必须是 ASCII hex
 *
 * @throws WalletValidationError 当校验失败
 */
export function validateAddress(addr: string): void {
  if (!addr.startsWith('0x')) {
    throw new WalletValidationError('address must start with 0x');
  }
  if (addr.length !== 42) {
    throw new WalletValidationError(`address must be 42 chars (got ${addr.length})`);
  }
  for (let i = 2; i < addr.length; i++) {
    if (!isHexChar(addr[i])) {
      throw new WalletValidationError('address has non-hex chars');
    }
  }
}

function isHexChar(c: string): boolean {
  return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F');
}

export function validateChain(chainId: number): void {
  if (!SUPPORTED_CHAINS.includes(chainId)) {
    throw new WalletValidationError(
      `unsupported chain_id: ${chainId}; supported: ${SUPPORTED_CHAINS.join(', ')}`,
    );
  }
}

export function validateLabel(label: string | null | undefined): void {
  if (label == null) return;
  if (label.length === 0) {
    throw new WalletValidationError('label cannot be empty');
  }
  if (label.length > 64) {
    throw new WalletValidationError('label > 64 chars');
  }
}

/** 缩短 EVM 地址显示：`0x1234567890abcdef...` → `0x1234…cdef`。
 *
 * **用途**：L1 列表 / 表格里地址列宽度太窄，用缩短版。
 * **< 10 字符**：原样返回（避免切到非 EVM 地址）。
 */
export function shortAddress(addr: string): string {
  if (addr.length < 10) return addr;
  return `${addr.slice(0, 6)}…${addr.slice(-4)}`;
}
