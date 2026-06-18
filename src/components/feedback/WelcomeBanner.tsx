// v0.53b — WelcomeBanner component.
//
// Renders the "Setup incomplete" banner on the
// Dashboard when the user has unfinished
// configuration. Reads from useWelcomeStore
// (configured flags) + queries secretsStatus as
// the source of truth (in case the user changed
// something outside the wizard).
//
// The "Complete" button navigates to /welcome.
// v0.53c will round-trip: when the user finishes
// a step, the banner auto-updates.

import { useQuery } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { secretsStatus, type SecretsStatus } from '@/ipc';
import { useT } from '@/lib/i18n';
import { AlertTriangle, ChevronRight } from 'lucide-react';

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
  if (s.llm_keys.length === 0) out.push('llm');
  const pmApi = s.polymarket.find((x) => x.kind === 'pm_api')?.configured;
  if (!pmApi) out.push('pm');
  if (s.wallets.length === 0) out.push('wallet');
  return out;
}
