import { Link } from 'react-router-dom';
import { Bell, CheckCircle2, X, Trash2 } from 'lucide-react';
import { Card } from '@/components/base/Card';
import { Button } from '@/components/base/Button';
import { Pill } from '@/components/base/Pill';
import { EmptyState } from '@/components/feedback/EmptyState';
import { useToastStore } from '@/stores/toast-store';

const KIND_LABEL = {
  info: 'Info',
  success: 'Success',
  warning: 'Warning',
  error: 'Error',
} as const;

const KIND_CLS = {
  info: 'border-accent/40 bg-accent/10 text-fg',
  success: 'border-bull/40 bg-bull/10 text-fg',
  warning: 'border-warning/40 bg-warning/10 text-fg',
  error: 'border-bear/40 bg-bear/10 text-fg',
} as const;

export function Notifications() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);
  const clear = useToastStore((s) => s.clear);

  return (
    <div className="space-y-4 max-w-2xl">
      <Card padding="sm">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Bell className="w-4 h-4 text-muted" />
            <div>
              <h2 className="text-[13px] font-semibold text-fg">Notifications</h2>
              <p className="text-[11px] text-muted mt-0.5">
                Live toast queue. {toasts.length} active.
              </p>
            </div>
          </div>
          {toasts.length > 0 && (
            <Button variant="ghost" size="sm" iconLeft={<Trash2 className="w-3 h-3" />} onClick={clear}>
              Clear all
            </Button>
          )}
        </div>
      </Card>

      {toasts.length === 0 ? (
        <EmptyState
          icon={<Bell className="w-5 h-5" />}
          title="No notifications"
          description="Important events will appear here (new signals, order fills, errors)."
        />
      ) : (
        <div className="space-y-2">
          {toasts.map((t) => (
            <div
              key={t.id}
              className={'rounded-md border px-3 py-2.5 flex items-start gap-2 ' + KIND_CLS[t.kind]}
            >
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <Pill kind={t.kind === 'success' ? 'bull' : t.kind === 'error' ? 'bear' : t.kind === 'warning' ? 'warning' : 'accent'}>
                    {KIND_LABEL[t.kind]}
                  </Pill>
                  <span className="text-[10px] text-muted">just now</span>
                </div>
                <div className="text-[12px] font-medium text-fg mt-1">{t.title}</div>
                {t.body && (
                  <div className="text-[11px] text-fg-secondary mt-0.5 break-words">{t.body}</div>
                )}
              </div>
              <button
                onClick={() => dismiss(t.id)}
                className="text-muted hover:text-fg shrink-0"
                aria-label="dismiss"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
          ))}
        </div>
      )}

      <Card title="Event types" description="What generates a notification">
        <div className="space-y-2 text-[12px]">
          <Row label="New active signal" hint="When |edge| crosses your threshold (default 5%)" />
          <Row label="Order fill" hint="When a Mode B signed order settles" />
          <Row label="Keyring access" hint="When a secret is read or denied" />
          <Row label="Auto-disable" hint="When 3 consecutive probe failures disable a provider" />
          <Row label="Daily brief" hint="When 00:00 UTC refresh runs (silent on success)" />
        </div>
        <div className="pt-3 border-t border-border">
          <Link to="/settings">
            <Button variant="secondary" size="sm">Notification settings</Button>
          </Link>
        </div>
      </Card>
    </div>
  );
}

function Row({ label, hint }: { label: string; hint: string }) {
  return (
    <div className="flex items-center gap-2">
      <CheckCircle2 className="w-3.5 h-3.5 text-accent shrink-0" />
      <span className="text-fg">{label}</span>
      <span className="text-muted text-[10px]">— {hint}</span>
    </div>
  );
}
