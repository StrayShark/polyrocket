/**
 * L1 domain mirror — M5 Copy.
 * Mirrors src-tauri/src/domain/copy/mod.rs.
 */

export interface CopyTarget {
  id: string;
  address: string;
  label?: string | null;
  enabled: boolean;
  allocationCap?: string | null;
  minEdge: number;
  createdAt: number;
}

export interface CopyEvent {
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

export interface MirrorDecision {
  side: 'YES' | 'NO';
  size: string;
  flip: boolean;
}

export class CopyValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'CopyValidationError';
  }
}

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

export function shouldMirror(
  target: CopyTarget,
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

export function isDuplicateTx(events: CopyEvent[], txHash: string): boolean {
  return events.some((e) => e.txHash === txHash);
}
