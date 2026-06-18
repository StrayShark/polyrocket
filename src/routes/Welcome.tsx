// v0.53b — Welcome route (first-run landing).
//
// The 6-step wizard lives here. v0.53b ships the
// route scaffold + StepProgress + the welcome
// step + the navigation logic. v0.53c fills in
// the remaining 5 step components.

import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Card } from '@/components/base/Card';
import { useWelcomeStore, WELCOME_STEPS, type WelcomeStep as StepName } from '@/stores/welcome-store';
import { useT, useLocaleStore } from '@/lib/i18n';
import { StepProgress } from '@/components/welcome/StepProgress';
import { WelcomeStep } from '@/components/welcome/WelcomeStep';
import { StorageStep } from '@/components/welcome/StorageStep';
import { ThemeStep } from '@/components/welcome/ThemeStep';
import { LlmStep } from '@/components/welcome/LlmStep';
import { PolymarketStep } from '@/components/welcome/PolymarketStep';
import { FinishStep } from '@/components/welcome/FinishStep';
import { CheckCircle2, ChevronRight, ChevronLeft } from 'lucide-react';

export function Welcome() {
  const welcome = useWelcomeStore();
  const navigate = useNavigate();
  const { t } = useT();
  const setLocale = useLocaleStore((s) => s.setLocale);

  // v0.53b — if the user has finished the wizard,
  // redirect to dashboard. The L1 also does this
  // in main.tsx; double-check here for the case
  // where the user navigates back to /welcome
  // intentionally (e.g. via the URL bar).
  useEffect(() => {
    if (welcome.done) {
      navigate('/dashboard', { replace: true });
    }
  }, [welcome.done, navigate]);

  const step = welcome.step;
  const stepIndex = WELCOME_STEPS.indexOf(step);
  const isFirst = stepIndex === 0;
  const isLast = stepIndex === WELCOME_STEPS.length - 1;

  const next = () => {
    if (isLast) {
      welcome.setDone(true);
      navigate('/dashboard', { replace: true });
      return;
    }
    const nextStep = WELCOME_STEPS[stepIndex + 1];
    if (nextStep) welcome.setStep(nextStep);
  };
  const back = () => {
    if (isFirst) return;
    const prevStep = WELCOME_STEPS[stepIndex - 1];
    if (prevStep) welcome.setStep(prevStep);
  };
  const skip = () => {
    welcome.setDone(true);
    navigate('/dashboard', { replace: true });
  };
  const onLocale = (l: string) => {
    welcome.setLocale(l);
    setLocale(l as 'en' | 'zh');
  };

  return (
    <div className="min-h-screen flex items-center justify-center p-6 bg-bg">
      <div className="w-full max-w-3xl space-y-4">
        <StepProgress current={step} steps={WELCOME_STEPS} />

        <Card padding="lg" data-testid={`welcome-card-${step}`}>
          {renderStep(step, { welcome, onLocale, t })}
        </Card>

        <div className="flex items-center justify-between">
          <div>
            {!isFirst && (
              <button
                type="button"
                onClick={back}
                data-testid="welcome-back"
                className="text-[12px] text-muted hover:text-fg"
              >
                <ChevronLeft className="w-3 h-3 inline mr-1" />
                {t('common.back')}
              </button>
            )}
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={skip}
              data-testid="welcome-skip"
              className="text-[12px] text-muted hover:text-fg"
            >
              {t('welcome.skip')}
            </button>
            <button
              type="button"
              onClick={next}
              data-testid="welcome-next"
              className="h-8 px-4 rounded-md text-[12px] font-medium bg-accent text-bg hover:bg-accent/90"
            >
              {isLast ? (
                <span className="flex items-center gap-1">
                  <CheckCircle2 className="w-3.5 h-3.5" />
                  {t('welcome.finish')}
                </span>
              ) : (
                <span className="flex items-center gap-1">
                  {t('common.next')}
                  <ChevronRight className="w-3.5 h-3.5" />
                </span>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function renderStep(
  step: StepName,
  ctx: {
    welcome: ReturnType<typeof useWelcomeStore.getState>;
    onLocale: (l: string) => void;
    t: (k: string, vars?: Record<string, string | number>) => string;
  },
) {
  switch (step) {
    case 'welcome':
      return <WelcomeStep onLocale={ctx.onLocale} />;
    case 'storage':
      return <StorageStep welcome={ctx.welcome} />;
    case 'theme':
      return <ThemeStep />;
    case 'llm':
      return <LlmStep welcome={ctx.welcome} />;
    case 'polymarket':
      return <PolymarketStep welcome={ctx.welcome} />;
    case 'finish':
      return <FinishStep welcome={ctx.welcome} />;
  }
}
