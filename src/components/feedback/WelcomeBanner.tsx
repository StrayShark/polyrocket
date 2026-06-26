// v0.53b —— WelcomeBanner 组件。
//
// 当用户有未完成的配置项时,在 Dashboard 渲染
// "Setup incomplete" 横幅。读取 useWelcomeStore
// (configured 标志)+ 调 secretsStatus 作为
// 事实来源(以防用户绕过 wizard 直接改了配置)。
//
// "Complete" 按钮跳转到 /welcome。
// v0.54c —— 修复 v0.53b 已有的 bug:
// computeMissing 之前读了错误的字段名
// (llm_keys.length / polymarket.find /
// wallets.length),实际的 SecretsStatus shape
// 是 { llm_keys, pm_api, pm_passphrase,
// pm_secret, wallet_pk }。

import { useQuery } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { secretsStatus } from '@/ipc';
import type { SecretsStatus } from '@/types/shared';
import { useT } from '@/lib/i18n';
import { AlertTriangle, ChevronRight } from 'lucide-react';

/**
 * `WelcomeBanner` —— Dashboard 顶部「Setup incomplete」横幅。
 *
 * **显示条件**：调 `computeMissing` 算还有哪些 sub-config 缺（`llmKeys > 0` /
 * `pmApi` / `walletPk > 0`）。**全部 OK** 时不渲染（return null）。
 *
 * **数据流**：
 *   1. 挂载时调用 `secretsStatus()` IPC
 *   2. 同时读 `useWelcomeStore.configured` 标志
 *   3. 两者 union → `computeMissing` → 列表
 *   4. 渲染 alert 卡片 + 各项状态 + 「Complete」按钮 → navigate('/welcome')
 *
 * **v0.54c fix**：`computeMissing` 之前读 `SecretsStatus` 字段名错（`llm_keys.length` /
 * `polymarket.find`），实际 shape 是 `{ llm_keys: count, pm_api: bool, pm_passphrase, pm_secret, wallet_pk: count }`。
 */
export function WelcomeBanner() {
  const { t } = useT();
  const navigate = useNavigate();
  const { data } = useQuery({
    queryKey: ['secrets-status'],
    queryFn: () => secretsStatus(),
    refetchInterval: 60_000,
  });
  if (!data) return null;
  const missing = computeMissing(data);
  if (missing.length === 0) return null;
  return (
    <div
      data-testid="welcome-banner"
      data-missing={missing.join(',')}
      className="rounded-md border border-warn/40 bg-warn/5 px-3 py-2.5 flex items-center gap-3"
    >
      <AlertTriangle className="w-4 h-4 text-warn shrink-0" />
      <div className="flex-1 min-w-0">
        <div className="text-[12px] text-fg font-medium">
          {t('welcome.banner_title', { n: missing.length })}
        </div>
        <div className="text-[10px] text-muted mt-0.5">
          {missing
            .map((m) => t(`welcome.banner_missing_${m}`))
            .join(' · ')}
        </div>
      </div>
      <button
        type="button"
        onClick={() => navigate('/welcome')}
        data-testid="welcome-banner-complete"
        className="h-7 px-3 rounded-md text-[11px] font-medium bg-accent text-bg hover:bg-accent/90 flex items-center gap-1"
      >
        {t('welcome.banner_complete')}
        <ChevronRight className="w-3 h-3" />
      </button>
    </div>
  );
}

function computeMissing(s: SecretsStatus): Array<'llm' | 'pm' | 'wallet'> {
  const out: Array<'llm' | 'pm' | 'wallet'> = [];
  // 当 llm_keys > 0 时,用户至少配了一个 LLM key。
  // (IPC 返回的是数量,不是列表,所以我们不需要
  // 知道具体配了哪些 key,只要知道是否配了。)
  if (s.llm_keys === 0) out.push('llm');
  // Polymarket CLOB 需要全部 3 项凭据(api key、
  // secret、passphrase)。任一缺失就视为 PM 未配。
  if (!s.pm_api || !s.pm_secret || !s.pm_passphrase) out.push('pm');
  // Wallet:至少存有一个 pk。
  if (s.wallet_pk === 0) out.push('wallet');
  return out;
}
