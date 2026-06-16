import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import { useThemeStore, type Theme } from '@/stores/theme-store';
import { Button } from '@/components/base/Button';
import { Card } from '@/components/base/Card';
import { useToastStore } from '@/stores/toast-store';
import { CheckCircle2, Wallet, Key, Sparkles, ChevronRight, ChevronLeft, Radar } from 'lucide-react';
import { POLYGON_MAINNET } from '@/types/wallet';

interface OnbState {
  done: boolean;
  step: number;
  setStep: (s: number) => void;
  setDone: (d: boolean) => void;
  reset: () => void;
}

export const useOnbStore = create<OnbState>()(
  persist(
    (set) => ({
      done: false,
      step: 0,
      setStep: (s) => set({ step: s }),
      setDone: (d) => set({ done: d, step: d ? 4 : 0 }),
      reset: () => set({ done: false, step: 0 }),
    }),
    {
      name: 'polyrocket.onboarding',
      storage: createJSONStorage(() => localStorage),
    },
  ),
);

const STEPS = ['welcome', 'theme', 'wallet', 'llm'] as const;

export function Onboarding() {
  const onb = useOnbStore();
  const navigate = useNavigate();
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);
  const push = useToastStore((s) => s.push);

  // Auto-redirect if already done
  useEffect(() => {
    if (onb.done) navigate('/dashboard', { replace: true });
  }, [onb.done, navigate]);

  if (onb.done) return null;

  const step = onb.step;
  const stepName = STEPS[step] ?? 'welcome';
  const isLast = step === STEPS.length - 1;
  const isFirst = step === 0;

  const next = () => {
    if (isLast) {
      onb.setDone(true);
      push({ kind: 'success', title: 'Welcome to polyrocket!', ttl: 4000 });
      navigate('/dashboard', { replace: true });
    } else {
      onb.setStep(step + 1);
    }
  };
  const back = () => onb.setStep(Math.max(0, step - 1));
  const skip = () => {
    onb.setDone(true);
    navigate('/dashboard', { replace: true });
  };

  return (
    <div className="min-h-screen flex items-center justify-center p-6 bg-bg">
      <div className="w-full max-w-2xl space-y-4">
        {/* Progress */}
        <div className="flex items-center gap-1.5">
          {STEPS.map((s, i) => (
            <div
              key={s}
              className={
                'h-1 flex-1 rounded-full transition-colors ' +
                (i <= step ? 'bg-accent' : 'bg-surface-2')
              }
            />
          ))}
        </div>

        <Card padding="lg">
          {stepName === 'welcome' && <WelcomeStep />}
          {stepName === 'theme' && <ThemeStep current={theme} onChange={setTheme} />}
          {stepName === 'wallet' && <WalletStep />}
          {stepName === 'llm' && <LlmStep />}
        </Card>

        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            {!isFirst && (
              <Button variant="ghost" size="sm" iconLeft={<ChevronLeft className="w-3 h-3" />} onClick={back}>
                Back
              </Button>
            )}
          </div>
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="sm" onClick={skip}>
              Skip
            </Button>
            <Button variant="primary" size="sm" iconRight={isLast ? <CheckCircle2 className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />} onClick={next}>
              {isLast ? 'Finish' : 'Next'}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}

function WelcomeStep() {
  return (
    <div className="space-y-4 text-center py-6">
      <div className="w-16 h-16 rounded-2xl bg-accent mx-auto grid place-items-center">
        <Radar className="w-8 h-8 text-white" />
      </div>
      <div>
        <h1 className="text-[24px] font-semibold text-fg">Welcome to polyrocket</h1>
        <p className="text-[13px] text-muted mt-2 max-w-md mx-auto">
          A Polymarket analysis desktop client with multi-LLM fan-out,
          signal detection, copy trading, and self-custodial key storage.
        </p>
      </div>
      <div className="grid grid-cols-3 gap-3 max-w-md mx-auto pt-4">
        <Feature icon={Sparkles} label="Multi-LLM" />
        <Feature icon={Wallet} label="Self-custody" />
        <Feature icon={Key} label="OS keyring" />
      </div>
    </div>
  );
}

function Feature({ icon: Icon, label }: { icon: React.ComponentType<{ className?: string }>; label: string }) {
  return (
    <div className="rounded-md border border-border bg-surface-2 p-3 flex flex-col items-center gap-1.5">
      <Icon className="w-5 h-5 text-accent" />
      <span className="text-[11px] text-fg">{label}</span>
    </div>
  );
}

function ThemeStep({ current, onChange }: { current: Theme; onChange: (t: Theme) => void }) {
  const themes: Array<{ id: Theme; name: string; desc: string }> = [
    { id: 'dark', name: 'Dark', desc: 'Cursor / VS Code Dark+' },
    { id: 'light', name: 'Light', desc: 'Clean and bright' },
    { id: 'matrix', name: 'Matrix', desc: 'Green-on-black hacker' },
  ];
  return (
    <div className="space-y-4 py-4">
      <div>
        <h2 className="text-[18px] font-semibold text-fg">Pick a theme</h2>
        <p className="text-[12px] text-muted mt-1">
          You can change this anytime from the theme switcher in the topbar.
        </p>
      </div>
      <div className="grid grid-cols-3 gap-3">
        {themes.map((t) => (
          <button
            key={t.id}
            onClick={() => onChange(t.id)}
            className={
              'rounded-md border p-4 text-left transition-colors ' +
              (current === t.id
                ? 'bg-accent/10 border-accent/40'
                : 'bg-surface-2 border-border hover:bg-surface-hover')
            }
          >
            <div className="flex items-center gap-2 mb-2">
              <div
                className="w-4 h-4 rounded-full"
                style={{
                  background:
                    t.id === 'dark' ? '#1E1E1E' :
                    t.id === 'light' ? '#FFFFFF' :
                    '#10A37F',
                  border: '1px solid var(--border)',
                }}
              />
              <span className="text-[13px] font-medium text-fg">{t.name}</span>
              {current === t.id && <CheckCircle2 className="w-3.5 h-3.5 text-accent ml-auto" />}
            </div>
            <div className="text-[11px] text-muted">{t.desc}</div>
          </button>
        ))}
      </div>
    </div>
  );
}

function WalletStep() {
  return (
    <div className="space-y-4 py-4">
      <div>
        <h2 className="text-[18px] font-semibold text-fg">Add a wallet</h2>
        <p className="text-[12px] text-muted mt-1">
          Register a Polygon wallet (chain {POLYGON_MAINNET}). The private key
          is stored in the OS keyring — never in SQLite or any plain file.
        </p>
      </div>
      <div className="rounded-md border border-border bg-surface-2 p-4 space-y-2 text-[12px]">
        <div className="flex items-center gap-2 text-fg">
          <CheckCircle2 className="w-4 h-4 text-bull" />
          <span>Address-only registration: no key needed at this step</span>
        </div>
        <div className="flex items-center gap-2 text-fg">
          <CheckCircle2 className="w-4 h-4 text-bull" />
          <span>Private key (when added) lives in macOS Keychain / Win Cred Mgr / Linux SS</span>
        </div>
        <div className="flex items-center gap-2 text-fg">
          <CheckCircle2 className="w-4 h-4 text-bull" />
          <span>You can add more wallets later from /wallets</span>
        </div>
      </div>
      <div className="flex justify-end pt-2">
        <a href="/wallets">
          <Button variant="secondary" size="sm" iconLeft={<Wallet className="w-3 h-3" />}>
            Open Wallets page
          </Button>
        </a>
      </div>
    </div>
  );
}

function LlmStep() {
  return (
    <div className="space-y-4 py-4">
      <div>
        <h2 className="text-[18px] font-semibold text-fg">Configure LLM providers</h2>
        <p className="text-[12px] text-muted mt-1">
          Add API keys for OpenAI / Anthropic / Google / DeepSeek / OpenAI-compatible
          endpoints. Each key is stored in the OS keyring under
          <code className="font-mono text-fg ml-1">polyrocket/llm/&lt;provider&gt;/&lt;alias&gt;</code>.
        </p>
      </div>
      <div className="rounded-md border border-border bg-surface-2 p-4 space-y-2 text-[12px]">
        <div className="flex items-center gap-2 text-fg">
          <CheckCircle2 className="w-4 h-4 text-bull" />
          <span>5 provider types: openai / anthropic / google / deepseek / custom</span>
        </div>
        <div className="flex items-center gap-2 text-fg">
          <CheckCircle2 className="w-4 h-4 text-bull" />
          <span>3 prompt templates for market analysis (M10)</span>
        </div>
        <div className="flex items-center gap-2 text-fg">
          <CheckCircle2 className="w-4 h-4 text-bull" />
          <span>Key rotation with priority + auto-failover on rate-limit</span>
        </div>
      </div>
      <div className="flex justify-end pt-2">
        <a href="/llm-mgmt">
          <Button variant="secondary" size="sm" iconLeft={<Key className="w-3 h-3" />}>
            Open LLM Management
          </Button>
        </a>
      </div>
    </div>
  );
}
