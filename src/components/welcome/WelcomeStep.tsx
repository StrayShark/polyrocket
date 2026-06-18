// v0.53b — WelcomeStep (Step 1 of 6).
//
// Hero + language picker. No IPC side effects on
// mount; the locale change writes to the
// locale-store + welcome-store.

import { Rocket } from 'lucide-react';
import { useT } from '@/lib/i18n';

export function WelcomeStep({
  onLocale,
}: {
  onLocale: (l: string) => void;
}) {
  const { t, locale } = useT();
  return (
    <div className="space-y-6 py-4">
      <div className="text-center space-y-3">
        <div className="w-16 h-16 rounded-2xl bg-accent mx-auto grid place-items-center">
          <Rocket className="w-8 h-8 text-white" />
        </div>
        <div>
          <h1
            className="text-[24px] font-semibold text-fg"
            data-testid="welcome-step-welcome-title"
          >
            {t('welcome.hero_title')}
          </h1>
          <p className="text-[13px] text-muted mt-2 max-w-md mx-auto">
            {t('welcome.hero_tagline')}
          </p>
        </div>
      </div>

      {/* v0.53b — language picker. The user can
          change this anytime from the topbar. We
          default to whatever navigator.language
          reports; both en and zh are bundled. */}
      <div className="max-w-md mx-auto">
        <div className="text-[10px] text-muted mb-2 uppercase tracking-wider">
          {t('welcome.locale_label')}
        </div>
        <div className="grid grid-cols-2 gap-2">
          <LocaleButton
            active={locale === 'en'}
            onClick={() => onLocale('en')}
            testid="welcome-locale-en"
            label="English"
            sub="EN"
          />
          <LocaleButton
            active={locale === 'zh'}
            onClick={() => onLocale('zh')}
            testid="welcome-locale-zh"
            label="中文"
            sub="ZH"
          />
        </div>
      </div>

      {/* v0.53b — value props. 3 short bullets that
          reinforce "local-first" / "self-custody" /
          "OS keyring". Same language as the old
          M13 v2.0 onboarding. */}
      <div className="grid grid-cols-3 gap-3 max-w-md mx-auto pt-2">
        <PropChip
          testid="welcome-step-welcome-vp-0"
          title={t('welcome.prop_local_title')}
          body={t('welcome.prop_local_body')}
        />
        <PropChip
          testid="welcome-step-welcome-vp-1"
          title={t('welcome.prop_keys_title')}
          body={t('welcome.prop_keys_body')}
        />
        <PropChip
          testid="welcome-step-welcome-vp-2"
          title={t('welcome.prop_llm_title')}
          body={t('welcome.prop_llm_body')}
        />
      </div>
    </div>
  );
}

function LocaleButton({
  active,
  onClick,
  label,
  sub,
  testid,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
  sub: string;
  testid: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      data-testid={testid}
      className={
        'h-12 rounded-md text-[13px] font-medium border transition-colors ' +
        (active
          ? 'bg-accent/10 border-accent/40 text-fg'
          : 'bg-surface-2 border-border text-muted hover:text-fg')
      }
    >
      <div className="flex items-center justify-center gap-2">
        <span className="text-[10px] text-muted">{sub}</span>
        <span>{label}</span>
      </div>
    </button>
  );
}

function PropChip({
  testid,
  title,
  body,
}: {
  testid: string;
  title: string;
  body: string;
}) {
  return (
    <div
      data-testid={testid}
      className="rounded-md border border-border bg-surface-2 p-3 text-center"
    >
      <div className="text-[11px] font-medium text-fg">{title}</div>
      <div className="text-[10px] text-muted mt-0.5">{body}</div>
    </div>
  );
}
