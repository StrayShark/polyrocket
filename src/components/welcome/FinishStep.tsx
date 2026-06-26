// v0.53b —— FinishStep (Step 6 of 6)。
//
// 对刚才所配置内容的只读摘要。
// 高亮缺失项,让用户可以返回补充。
// 此处的 Next 按钮是 "Finish" 按钮 ——
// 点击后设置 welcome.done=true 并跳转至 /dashboard。

import { CheckCircle2, XCircle } from 'lucide-react';
import { useT } from '@/lib/i18n';
import type { WelcomeState } from '@/stores/welcome-store';

export function FinishStep({
  welcome,
}: {
  welcome: WelcomeState;
}) {
  const { t, locale } = useT();
  const items: Array<{ key: string; ok: boolean; label: string }> = [
    {
      key: 'storage',
      ok: welcome.configured.storagePath,
      label: t('welcome.finish_storage'),
    },
    {
      key: 'theme',
      ok: welcome.configured.theme,
      label: t('welcome.finish_theme'),
    },
    {
      key: 'llm',
      ok: welcome.configured.llmAtLeastOne,
      label: t('welcome.finish_llm'),
    },
    {
      key: 'pm',
      ok: welcome.configured.polymarketApi,
      label: t('welcome.finish_pm'),
    },
    {
      key: 'wallet',
      ok: welcome.configured.walletPk,
      label: t('welcome.finish_wallet'),
    },
  ];

  return (
    <div className="space-y-4 py-2" data-testid="welcome-finish-summary">
      <div>
        <h2
          className="text-title-md font-semibold text-fg"
          data-testid="welcome-finish-title"
        >
          {t('welcome.finish_title')}
        </h2>
        <p className="text-[12px] text-muted mt-1">
          {t('welcome.finish_desc')}
        </p>
      </div>

      <ul className="space-y-1.5" data-testid="welcome-finish-list">
        {items.map((it) => (
          <li
            key={it.key}
            data-testid={`welcome-finish-${it.key}`}
            data-ok={it.ok}
            className="flex items-center gap-2 text-[12px]"
          >
            {it.ok ? (
              <CheckCircle2 className="w-3.5 h-3.5 text-bull" />
            ) : (
              <XCircle className="w-3.5 h-3.5 text-muted" />
            )}
            <span
              className={
                it.ok ? 'text-fg' : 'text-muted line-through'
              }
            >
              {it.label}
            </span>
          </li>
        ))}
      </ul>

      <div className="rounded-md border border-border bg-surface-2 p-3 text-[11px] text-muted">
        {t('welcome.finish_after_note', { locale: locale.toUpperCase() })}
      </div>
    </div>
  );
}
