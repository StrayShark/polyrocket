/**
 * L1 domain mirror — M7 ModelLab.
 * Mirrors src-tauri/src/domain/lab/mod.rs.
 */

export interface LabRun {
  id: string;
  modelVersion: string;
  startedAt: number;
  finishedAt?: number | null;
  paramsJson: string;
  metricsJson?: string | null;
  status: string;
}

export type RunStatus = 'queued' | 'running' | 'done' | 'error';

export function parseRunStatus(s: string): RunStatus | null {
  if (s === 'queued' || s === 'running' || s === 'done' || s === 'error') return s;
  return null;
}

/** Legal transitions: Queued→Running, Running→Done|Error. */
export function canTransitionTo(from: RunStatus, to: RunStatus): boolean {
  if (from === 'queued' && to === 'running') return true;
  if (from === 'running' && (to === 'done' || to === 'error')) return true;
  return false;
}

export class VersionValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'VersionValidationError';
  }
}

/** vMAJOR.MINOR[.PATCH][-tag] */
export function validateVersion(v: string): void {
  if (!v.startsWith('v')) {
    throw new VersionValidationError(`version must start with 'v': ${v}`);
  }
  const rest = v.slice(1);
  const parts = rest.split('.');
  if (parts.length < 2 || parts.length > 3) {
    throw new VersionValidationError(`version must be vMAJOR.MINOR[.PATCH]: ${v}`);
  }
  for (const p of parts.slice(0, -1)) {
    if (!/^\d+$/.test(p)) {
      throw new VersionValidationError(`non-numeric version component: ${p}`);
    }
  }
  const last = parts[parts.length - 1];
  let num = last;
  let tag: string | null = null;
  const dash = last.indexOf('-');
  if (dash >= 0) {
    num = last.slice(0, dash);
    tag = last.slice(dash + 1);
  }
  if (!/^\d+$/.test(num)) {
    throw new VersionValidationError(`non-numeric PATCH: ${last}`);
  }
  if (tag !== null && tag === '') {
    throw new VersionValidationError('empty tag after dash');
  }
}

function parseTuple(v: string): [number, number, number] {
  let stripped = v.startsWith('v') ? v.slice(1) : v;
  const dash = stripped.indexOf('-');
  if (dash >= 0) stripped = stripped.slice(0, dash);
  const parts = stripped.split('.').map((p) => Number(p));
  return [
    Number.isFinite(parts[0]) ? parts[0] : 0,
    Number.isFinite(parts[1]) ? parts[1] : 0,
    Number.isFinite(parts[2]) ? parts[2] : 0,
  ];
}

export function isOlder(a: string, b: string): boolean {
  const [aMaj, aMin, aPat] = parseTuple(a);
  const [bMaj, bMin, bPat] = parseTuple(b);
  if (aMaj !== bMaj) return aMaj < bMaj;
  if (aMin !== bMin) return aMin < bMin;
  return aPat < bPat;
}

export interface ModelPerf {
  modelVersion: string;
  nPredictions: number;
  winRate: number;
  brierScore: number;
  logLoss: number;
  avgEdge: number;
}

/**
 * Promotion rule: lower brier wins; tiebreak by higher win rate;
 * final tiebreak by more predictions.
 */
export function isBetter(candidate: ModelPerf, incumbent: ModelPerf): boolean {
  if (Math.abs(candidate.brierScore - incumbent.brierScore) > 1e-9) {
    return candidate.brierScore < incumbent.brierScore;
  }
  if (Math.abs(candidate.winRate - incumbent.winRate) > 1e-9) {
    return candidate.winRate > incumbent.winRate;
  }
  return candidate.nPredictions > incumbent.nPredictions;
}
