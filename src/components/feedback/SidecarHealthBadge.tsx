/**
 * Sidecar health badge (v0.10d).
 *
 * Small pill in the topbar showing the most recent probe status:
 *   - "ok"      → green dot + "Sidecar OK"
 *   - "failed"  → red dot + "Sidecar DOWN"
 *   - "unknown" → gray dot + "Sidecar —"
 *
 * Polls the IPC every 30s. Click to force a probe.
 */

import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { sidecarHealthNow, sidecarHealthSnapshot, type SidecarHealthSnapshot } from '@/ipc';

const COLORS = {
  ok: 'var(--bull)',
  failed: 'var(--bear)',
  unknown: 'var(--muted)',
};

const LABEL = {
  ok: 'Sidecar OK',
  failed: 'Sidecar DOWN',
  unknown: 'Sidecar —',
};

function dotColor(snap: SidecarHealthSnapshot | undefined): keyof typeof COLORS {
  if (!snap) return 'unknown';
  if (snap.last_success_at_ms == null && snap.last_failure_at_ms == null) return 'unknown';
  if (snap.last_failure_at_ms && (!snap.last_success_at_ms || snap.last_failure_at_ms > snap.last_success_at_ms)) {
    return 'failed';
  }
  return 'ok';
}

// Standalone helper so we can also call it from the click handler.
function statusOf(snap: SidecarHealthSnapshot | undefined): keyof typeof COLORS {
  return dotColor(snap);
}

export function SidecarHealthBadge() {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: ['sidecar-health'],
    queryFn: sidecarHealthSnapshot,
    refetchInterval: 30_000,
    staleTime: 25_000,
  });
  const [busy, setBusy] = useState(false);

  const onClick = async () => {
    if (busy) return;
    setBusy(true);
    try {
      await sidecarHealthNow();
      await queryClient.invalidateQueries({ queryKey: ['sidecar-health'] });
    } finally {
      setBusy(false);
    }
  };

  const status = statusOf(query.data);

  return (
    <button
      type="button"
      onClick={onClick}
      className="flex items-center gap-1.5 px-2 h-6 rounded-full text-[10px] font-medium hover:bg-surface-hover"
      style={{ border: '1px solid var(--border)', color: 'var(--fg-secondary)' }}
      title={`Last probe: ${status} · click to force a new probe`}
      aria-label={`Sidecar ${LABEL[status]}`}
      data-testid="sidecar-health-badge"
    >
      <span
        className="w-1.5 h-1.5 rounded-full"
        style={{ background: COLORS[status] }}
      />
      {LABEL[status]}
    </button>
  );
}
