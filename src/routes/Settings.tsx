import { useState } from 'react';
import { Settings as SettingsIcon, RotateCcw, Save, Database, Bell, Eye, FlaskConical } from 'lucide-react';
import { Card } from '@/components/base/Card';
import { Button } from '@/components/base/Button';
import { Input } from '@/components/base/Input';
import { Toggle } from '@/components/base/Toggle';
import { usePrefsStore } from '@/stores/prefs-store';
import { toast } from '@/stores/toast-store';

export function Settings() {
  const prefs = usePrefsStore();
  const [draft, setDraft] = useState({
    defaultMinEdgePct: prefs.defaultMinEdgePct,
    defaultAllocationCapUsdc: prefs.defaultAllocationCapUsdc,
    copyTradingEnabled: prefs.copyTradingEnabled,
    notificationsEnabled: prefs.notificationsEnabled,
    advancedStats: prefs.advancedStats,
  });

  const dirty = JSON.stringify(draft) !== JSON.stringify({
    defaultMinEdgePct: prefs.defaultMinEdgePct,
    defaultAllocationCapUsdc: prefs.defaultAllocationCapUsdc,
    copyTradingEnabled: prefs.copyTradingEnabled,
    notificationsEnabled: prefs.notificationsEnabled,
    advancedStats: prefs.advancedStats,
  });

  const save = () => {
    Object.entries(draft).forEach(([k, v]) => {
      prefs.setPref(k as keyof typeof draft, v as never);
    });
    toast.success('Settings saved');
  };

  const reset = () => {
    prefs.reset();
    setDraft({
      defaultMinEdgePct: prefs.defaultMinEdgePct,
      defaultAllocationCapUsdc: prefs.defaultAllocationCapUsdc,
      copyTradingEnabled: prefs.copyTradingEnabled,
      notificationsEnabled: prefs.notificationsEnabled,
      advancedStats: prefs.advancedStats,
    });
    toast.info('Settings reset to defaults');
  };

  return (
    <div className="space-y-4 max-w-2xl">
      <Card padding="sm">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <SettingsIcon className="w-4 h-4 text-muted" />
            <h2 className="text-[13px] font-semibold text-fg">Settings</h2>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="sm" iconLeft={<RotateCcw className="w-3 h-3" />} onClick={reset}>
              Reset
            </Button>
            <Button variant="primary" size="sm" iconLeft={<Save className="w-3 h-3" />} onClick={save} disabled={!dirty}>
              Save
            </Button>
          </div>
        </div>
      </Card>

      {/* Trading defaults */}
      <Card title="Trading defaults" description="Applied to new signals and copy target configs">
        <div className="space-y-3">
          <NumberField
            label="Default min |edge| %"
            hint="Only signals with |edge| above this are surfaced as actionable"
            value={draft.defaultMinEdgePct}
            min={1}
            max={50}
            onChange={(v) => setDraft({ ...draft, defaultMinEdgePct: v })}
          />
          <NumberField
            label="Default allocation cap (USDC)"
            hint="Per-trade max size for copy trading mirrors"
            value={draft.defaultAllocationCapUsdc}
            min={0}
            step={10}
            onChange={(v) => setDraft({ ...draft, defaultAllocationCapUsdc: v })}
          />
        </div>
      </Card>

      {/* Notifications */}
      <Card title="Notifications" description="Toast and system notifications for important events">
        <ToggleRow
          icon={Bell}
          label="In-app toasts"
          hint="Show notifications for new signals, order fills, keyring errors"
          checked={draft.notificationsEnabled}
          onChange={(v) => setDraft({ ...draft, notificationsEnabled: v })}
        />
      </Card>

      {/* Copy trading */}
      <Card title="Copy trading" description="Watch whale addresses and optionally mirror their trades">
        <ToggleRow
          icon={Database}
          label="Enable copy trading"
          hint="When off, copy targets are paused and no new events are recorded"
          checked={draft.copyTradingEnabled}
          onChange={(v) => setDraft({ ...draft, copyTradingEnabled: v })}
        />
      </Card>

      {/* Advanced */}
      <Card title="Advanced" description="Show extra stats and diagnostics">
        <ToggleRow
          icon={Eye}
          label="Show advanced stats"
          hint="Extra columns and breakdowns on PnL, Lab, and Analysis pages"
          checked={draft.advancedStats}
          onChange={(v) => setDraft({ ...draft, advancedStats: v })}
        />
      </Card>

      <Card title="Storage" description="Local data is stored in app data dir; secrets in OS keyring">
        <div className="text-[11px] text-muted space-y-1.5">
          <div className="flex items-center gap-2">
            <Database className="w-3 h-3" />
            <code className="font-mono text-fg">~/Library/Application Support/com.polyrocket.app/polyrocket.db</code>
          </div>
          <div className="flex items-center gap-2">
            <FlaskConical className="w-3 h-3" />
            <code className="font-mono text-fg">macOS Keychain / Windows Credential Manager / Linux Secret Service</code>
          </div>
          <div className="pt-2 text-[10px]">
            Note: .env is dev-only and gated by <code className="font-mono">POLYROCKET_ENV=dev</code> +{' '}
            <code className="font-mono">POLYROCKET_KEYRING_ONLY=0</code>.
          </div>
        </div>
      </Card>
    </div>
  );
}

function NumberField({
  label,
  hint,
  value,
  min,
  max,
  step,
  onChange,
}: {
  label: string;
  hint?: string;
  value: number;
  min?: number;
  max?: number;
  step?: number;
  onChange: (v: number) => void;
}) {
  return (
    <div>
      <div className="text-[12px] text-fg mb-1">{label}</div>
      {hint && <div className="text-[10px] text-muted mb-2">{hint}</div>}
      <Input
        type="number"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value) || 0)}
        className="w-32"
      />
    </div>
  );
}

function ToggleRow({
  icon: Icon,
  label,
  hint,
  checked,
  onChange,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-start gap-3 py-1">
      <Icon className="w-4 h-4 text-muted mt-0.5" />
      <div className="flex-1">
        <div className="text-[12px] text-fg">{label}</div>
        {hint && <div className="text-[10px] text-muted mt-0.5">{hint}</div>}
      </div>
      <Toggle checked={checked} onChange={onChange} />
    </div>
  );
}
