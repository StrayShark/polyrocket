import { Link } from 'react-router-dom';
import { Bell, CheckCircle2, X, Trash2 } from 'lucide-react';
import { Card } from '@/components/base/Card';
import { Button } from '@/components/base/Button';
import { Pill } from '@/components/base/Pill';
import { EmptyState } from '@/components/feedback/EmptyState';
import { useToastStore } from '@/stores/toast-store';
import { useT } from '@/lib/i18n';

const KIND_LABEL_KEY = {
  info: 'notifications.toast.kind.info',
  success: 'notifications.toast.kind.success',
  warning: 'notifications.toast.kind.warning',
  error: 'notifications.toast.kind.error',
} as const;

const KIND_CLS = {
  info: 'border-accent/40 bg-accent/10 text-fg',
  success: 'border-bull/40 bg-bull/10 text-fg',
  warning: 'border-warning/40 bg-warning/10 text-fg',
  error: 'border-bear/40 bg-bear/10 text-fg',
} as const;

/**
 * `/notifications` 路由 —— 当前 toast 列表 + 清空按钮。
 *
 * **数据来源**：`useToastStore`（zustand）—— 跟 `Toast` 容器共享同一 store。
 *
 * **4 种 kind 颜色**：
 *   - `info` / `success` — 蓝 / 绿
 *   - `warning` / `error` — 黄 / 红
 *
 * **状态机**：toast 是 transient（`staleTime` 在 toast 自身，不用这里管）。
 * 「Clear」按钮调 `useToastStore.clear()` 全部 dismiss。
 */
export function Notifications() {
  const { t } = useT();
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
              <h2 className="text-title-sm font-semibold text-fg">{t('notifications.title')}</h2>
              <p className="text-[11px] text-muted mt-0.5">
                {t('notifications.toast.subtitle', { n: toasts.length })}
              </p>
            </div>
          </div>
          {toasts.length > 0 && (
            <Button variant="ghost" size="sm" iconLeft={<Trash2 className="w-3 h-3" />} onClick={clear}>
              {t('notifications.toast.clear')}
            </Button>
          )}
        </div>
      </Card>

      {toasts.length === 0 ? (
        <EmptyState
          icon={<Bell className="w-5 h-5" />}
          title={t('notifications.empty.title')}
          description={t('notifications.toast.empty_desc')}
        />
      ) : (
        <div className="space-y-2">
          {toasts.map((t2) => (
            <div
              key={t2.id}
              className={'rounded-md border px-3 py-2.5 flex items-start gap-2 ' + KIND_CLS[t2.kind]}
            >
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <Pill kind={t2.kind === 'success' ? 'bull' : t2.kind === 'error' ? 'bear' : t2.kind === 'warning' ? 'warning' : 'accent'}>
                    {t(KIND_LABEL_KEY[t2.kind])}
                  </Pill>
                  <span className="text-[10px] text-muted">{t('notifications.toast.just_now')}</span>
                </div>
                <div className="text-[12px] font-medium text-fg mt-1">{t2.title}</div>
                {t2.body && (
                  <div className="text-[11px] text-fg-secondary mt-0.5 break-words">{t2.body}</div>
                )}
              </div>
              <button
                onClick={() => dismiss(t2.id)}
                className="text-muted hover:text-fg shrink-0"
                aria-label={t('notifications.toast.dismiss_aria')}
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
          ))}
        </div>
      )}

      <Card title={t('notifications.event_types.title')} description={t('notifications.event_types.desc')}>
        <div className="space-y-2 text-[12px]">
          <Row label={t('notifications.event.signal')} hint={t('notifications.event.signal_hint')} />
          <Row label={t('notifications.event.fill')} hint={t('notifications.event.fill_hint')} />
          <Row label={t('notifications.event.keyring')} hint={t('notifications.event.keyring_hint')} />
          <Row label={t('notifications.event.auto_disable')} hint={t('notifications.event.auto_disable_hint')} />
          <Row label={t('notifications.event.brief')} hint={t('notifications.event.brief_hint')} />
        </div>
        <div className="pt-3 border-t border-border">
          <Link to="/settings">
            <Button variant="secondary" size="sm">{t('notifications.event.settings_link')}</Button>
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
