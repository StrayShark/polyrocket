import { useState } from 'react';
import { Settings as SettingsIcon, RotateCcw, Save, Database, Bell, Eye, FlaskConical, Trash2 } from 'lucide-react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Card } from '@/components/base/Card';
import { Button } from '@/components/base/Button';
import { Input } from '@/components/base/Input';
import { Toggle } from '@/components/base/Toggle';
import { usePrefsStore } from '@/stores/prefs-store';
import { toast } from '@/stores/toast-store';
import {
  getAuditRetention,
  setAuditRetention,
  purgeAuditLogNow,
  type AuditRetentionView,
  type SetAuditRetentionArgs,
} from '@/ipc';
import { formatRetentionAge } from '@/lib/format';
import { useT } from '@/lib/i18n';

export function Settings() {
  const { t } = useT();
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
    toast.success(t('settings.btn.save_toast'));
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
    toast.info(t('settings.btn.reset_toast'));
  };

  return (
    <div className="space-y-4 max-w-2xl">
      <Card padding="sm">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <SettingsIcon className="w-4 h-4 text-muted" />
            <h2 className="text-[13px] font-semibold text-fg">{t('settings.title')}</h2>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="sm" iconLeft={<RotateCcw className="w-3 h-3" />} onClick={reset}>
              {t('settings.btn.reset')}
            </Button>
            <Button variant="primary" size="sm" iconLeft={<Save className="w-3 h-3" />} onClick={save} disabled={!dirty}>
              {t('settings.btn.save')}
            </Button>
          </div>
        </div>
      </Card>

      {/* Trading defaults */}
      <Card title={t('settings.section.trading')} description={t('settings.section.trading_desc')}>
        <div className="space-y-3">
          <NumberField
            label={t('settings.field.min_edge')}
            hint={t('settings.field.min_edge_hint')}
            value={draft.defaultMinEdgePct}
            min={1}
            max={50}
            onChange={(v) => setDraft({ ...draft, defaultMinEdgePct: v })}
          />
          <NumberField
            label={t('settings.field.allocation_cap')}
            hint={t('settings.field.allocation_cap_hint')}
            value={draft.defaultAllocationCapUsdc}
            min={0}
            step={10}
            onChange={(v) => setDraft({ ...draft, defaultAllocationCapUsdc: v })}
          />
        </div>
      </Card>

      {/* Notifications */}
      <Card title={t('settings.section.notifications')} description={t('settings.section.notifications_desc')}>
        <ToggleRow
          icon={Bell}
          label={t('settings.field.toasts')}
          hint={t('settings.field.toasts_hint')}
          checked={draft.notificationsEnabled}
          onChange={(v) => setDraft({ ...draft, notificationsEnabled: v })}
        />
      </Card>

      {/* Copy trading */}
      <Card title={t('settings.section.copy')} description={t('settings.section.copy_desc')}>
        <ToggleRow
          icon={Database}
          label={t('settings.field.copy_enabled')}
          hint={t('settings.field.copy_enabled_hint')}
          checked={draft.copyTradingEnabled}
          onChange={(v) => setDraft({ ...draft, copyTradingEnabled: v })}
        />
      </Card>

      {/* Advanced */}
      <Card title={t('settings.section.advanced')} description={t('settings.section.advanced_desc')}>
        <ToggleRow
          icon={Eye}
          label={t('settings.field.advanced_stats')}
          hint={t('settings.field.advanced_stats_hint')}
          checked={draft.advancedStats}
          onChange={(v) => setDraft({ ...draft, advancedStats: v })}
        />
      </Card>

      <Card title={t('settings.section.storage')} description={t('settings.section.storage_desc')}>
        <div className="text-[11px] text-muted space-y-1.5">
          <div className="flex items-center gap-2">
            <Database className="w-3 h-3" />
            <code className="font-mono text-fg">{t('settings.storage.db_path')}</code>
          </div>
          <div className="flex items-center gap-2">
            <FlaskConical className="w-3 h-3" />
            <code className="font-mono text-fg">{t('settings.storage.keyring')}</code>
          </div>
          <div
            className="pt-2 text-[10px]"
            // v0.14b — the env_note string embeds two `<code>` tags
            // (POLYROCKET_ENV=dev, POLYROCKET_KEYRING_ONLY=0). The
            // markup is identical in en and zh so dangerouslySetInnerHTML
            // keeps the locales in sync.
            dangerouslySetInnerHTML={{ __html: t('settings.storage.env_note') }}
          />
        </div>
      </Card>

      {/* v0.13c — audit retention policy (per-user override) */}
      <RetentionCard />
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

// =================================================================
// ============== v0.13c — Audit retention panel ===================
// =================================================================

/** Card that lets the user override the audit-log retention
 *  policy. Defaults (from `RetentionPolicy::default()`) are shown
 *  with `(default)` tag in the hint; any non-default value triggers
 *  a Save that calls `set_audit_retention` IPC, which immediately
 *  purges under the new policy. */
function RetentionCard() {
  const qc = useQueryClient();
  const query = useQuery({
    queryKey: ['audit-retention'],
    queryFn: () => getAuditRetention(),
    staleTime: 30_000,
  });
  const [retainDays, setRetainDays] = useState<string>('90');
  const [maxRowsK, setMaxRowsK] = useState<string>('50');
  const [minKeep, setMinKeep] = useState<string>('1000');
  const [initialized, setInitialized] = useState(false);

  // Sync local form state once the query resolves
  if (query.data && !initialized) {
    const r = query.data;
    setRetainDays(String(Math.round(r.retain_recent_ms / 86_400_000)));
    setMaxRowsK(String(Math.round(r.max_rows / 1000)));
    setMinKeep(String(r.min_keep_rows));
    setInitialized(true);
  }

  const saveMut = useMutation({
    mutationFn: (args: SetAuditRetentionArgs) => setAuditRetention(args),
    onSuccess: (n) => {
      toast.success(`Retention updated (purged ${n} row${n === 1 ? '' : 's'})`);
      qc.invalidateQueries({ queryKey: ['audit-retention'] });
    },
    onError: (e: unknown) => {
      toast.error(`Retention update failed: ${(e as Error).message ?? e}`);
    },
  });

  const purgeNowMut = useMutation({
    mutationFn: () => purgeAuditLogNow(),
    onSuccess: (n) => {
      toast.info(`Purged ${n} row${n === 1 ? '' : 's'}`);
    },
  });

  const handleSave = () => {
    const days = Math.max(1, Math.min(3650, Number(retainDays) || 90));
    const maxK = Math.max(1, Math.min(10_000, Number(maxRowsK) || 50));
    const min = Math.max(0, Math.min(100_000, Number(minKeep) || 1000));
    saveMut.mutate({
      retain_recent_ms: days * 86_400_000,
      max_rows: maxK * 1000,
      min_keep_rows: min,
    });
  };

  const defaults: AuditRetentionView | undefined = query.data;

  return (
    <Card
      title="Audit retention"
      description="Auto-purge the audit_log table on a 10-min tick. Floor of min_keep_rows is always kept."
      data-testid="audit-retention-card"
    >
      <div className="space-y-3">
        <NumberHintField
          label="Retain recent (days)"
          hint={`Default: 90 (${formatRetentionAge(90 * 86_400_000)}). Older rows are eligible for purge.`}
          value={retainDays}
          onChange={setRetainDays}
          min={1}
          max={3650}
          isDefault={defaults ? Number(retainDays) === Math.round(defaults.retain_recent_ms / 86_400_000) : false}
        />
        <NumberHintField
          label="Max rows (×1000)"
          hint={`Default: 50 (50,000). Hard cap on total row count.`}
          value={maxRowsK}
          onChange={setMaxRowsK}
          min={1}
          max={10_000}
          isDefault={defaults ? Number(maxRowsK) === Math.round(defaults.max_rows / 1000) : false}
        />
        <NumberHintField
          label="Min keep rows"
          hint={`Default: 1,000. Safety floor — never auto-purge below this many rows.`}
          value={minKeep}
          onChange={setMinKeep}
          min={0}
          max={100_000}
          isDefault={defaults ? Number(minKeep) === defaults.min_keep_rows : false}
        />
        <div className="flex items-center gap-2 pt-2">
          <Button
            variant="primary"
            size="sm"
            iconLeft={<Save className="w-3 h-3" />}
            onClick={handleSave}
            disabled={saveMut.isPending}
            data-testid="retention-save"
          >
            {saveMut.isPending ? 'Saving…' : 'Save & purge now'}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            iconLeft={<Trash2 className="w-3 h-3" />}
            onClick={() => purgeNowMut.mutate()}
            disabled={purgeNowMut.isPending}
            data-testid="retention-purge-now"
          >
            Purge now
          </Button>
          {query.isLoading && (
            <span className="text-[10px] text-muted">Loading current policy…</span>
          )}
          {query.isError && (
            <span className="text-[10px] text-danger">
              Failed to load: {(query.error as Error).message}
            </span>
          )}
        </div>
      </div>
    </Card>
  );
}

function NumberHintField({
  label,
  hint,
  value,
  onChange,
  min,
  max,
  isDefault,
}: {
  label: string;
  hint?: string;
  value: string;
  onChange: (v: string) => void;
  min?: number;
  max?: number;
  isDefault?: boolean;
}) {
  return (
    <div>
      <div className="flex items-center gap-2 mb-1">
        <span className="text-[12px] text-fg">{label}</span>
        {isDefault && (
          <span
            data-testid="retention-default-badge"
            className="text-[9px] uppercase tracking-wide text-muted px-1.5 py-0.5 rounded"
            style={{ background: 'var(--surface-2)' }}
          >
            default
          </span>
        )}
      </div>
      {hint && <div className="text-[10px] text-muted mb-2">{hint}</div>}
      <Input
        type="number"
        min={min}
        max={max}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-32"
      />
    </div>
  );
}
