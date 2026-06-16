/**
 * L1 domain mirror — M4 Wallets.
 * Mirrors src-tauri/src/domain/wallet/mod.rs.
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

export function shortAddress(addr: string): string {
  if (addr.length < 10) return addr;
  return `${addr.slice(0, 6)}…${addr.slice(-4)}`;
}
