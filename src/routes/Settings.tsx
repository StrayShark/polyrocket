import { useState, useEffect, useRef, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { Settings as SettingsIcon, RotateCcw, Save, Database, Bell, Eye, FlaskConical, Trash2, Download, Upload, Copy as CopyIcon, Globe } from 'lucide-react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Card } from '@/components/base/Card';
import { Button } from '@/components/base/Button';
import { Input } from '@/components/base/Input';
import { Toggle } from '@/components/base/Toggle';
import { usePrefsStore } from '@/stores/prefs-store';
import { toast } from '@/stores/toast-store';
import { cn } from '@/lib/cn';
import { useWelcomeStore } from '@/stores/welcome-store';
import { useLocaleStore, LOCALE_LABEL, SUPPORTED_LOCALES, type Locale } from '@/lib/i18n';
import { ThemeSwitcher } from '@/components/theme/ThemeSwitcher';
import {
  getAuditRetention,
  setAuditRetention,
  purgeAuditLogNow,
  setAutoPromoteConfig,
  setTelemetryEnabled,
  getTelemetryEnabled,
  listTelemetryLogs, // v0.49a
  purgeTelemetryLogs, // v0.49a
  getActiveModel, // v0.49b
  schedulerSelfTestNow, // v0.49c
  clobFeedStatus, // v0.51a
  getStorageInfo, // v0.54b
  migrateStoragePath, // v0.54b
  explainModel, // v0.55
  shapExplain, // v0.59
  getProxyConfig, // v0.56
  setProxyConfig, // v0.56
  clearProxyConfig, // v0.56
  setMirrorPaperMode,
  getMirrorPaperMode,
  type AuditRetentionView,
  type SetAuditRetentionArgs,
} from '@/ipc';
import { formatRetentionAge } from '@/lib/format';
import { useT } from '@/lib/i18n';
import {
  downloadPrefsAsFile,
  parsePrefsFromString,
  readFileAsText,
} from '@/lib/prefs-io';
import type { UiPrefs } from '@/stores/prefs-store';

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

  // v0.28c — push the auto-promote config to Rust on
  // mount of the Settings page. This is the bridge
  // between the L1 zustand store (source of truth for
  // the UI) and the Rust `AppState.auto_promote` field
  // (consumer for `train_job`).
  //
  // We also re-push whenever the user changes the
  // config (handled in the AutoPromoteCard). On mount
  // alone is enough to cover the common case: "user
  // opens Settings for the first time, the L1 store
  // has the persisted value, we push it to Rust".
  useEffect(() => {
    setAutoPromoteConfig({
      enabled: prefs.autoPromoteAfterTrain,
      brier_margin: prefs.autoPromoteBrierMargin,
    }).catch(() => {
      // Sidecar is not always available; the L1 store
      // is the source of truth, Rust will re-read on
      // the next Settings mount.
    });
    // Run once on mount. Re-runs would re-push the
    // same values; harmless but wasteful.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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

      {/* v0.74f — appearance settings (theme + language).
       *       Moved from sidebar footer (AppShell) per user feedback
       *       "language switch, theme switch should be in Settings".
       *       Positioned right after the page title so users
       *       find the most basic UI preference immediately. */}
      <AppearanceCard />

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

      {/* v0.23c — auto-promote margin */}
      <AutoPromoteCard />

      {/* v0.36b — export/import of UI prefs */}
      <BackupRestoreCard />

      {/* v0.53b — re-run setup. Lets the user
          revisit /welcome at any time to finish
          or reconfigure. Useful when secrets were
          rotated or the user wants to switch
          storage path. */}
      <RerunSetupCard />

      {/* v0.42c — opt-in lifecycle telemetry */}
      <TelemetryCard />

      {/* v0.49b — active model summary card. Shows
          the current active model's training metrics
          (version, Brier, mtime) and surfaces the
          on-disk path. Goes through `getActiveModel`
          IPC, so it's the same view Rust sees. */}
      <ActiveModelCard />

      {/* v0.55 — model explainability. Lets the
          user pick a sample (price + age) and see
          the per-feature contribution to the
          active model's prediction. Surfaces the
          SHAP-like decomposition of the 3-feature
          logistic model. */}
      <ExplainabilityCard />

      {/* v0.56 — network proxy / Tor support.
          Configures `POLYROCKET_PROXY` for the
          shared reqwest::Client + the sidecar
          child process. Restart required for
          changes to take effect. */}
      <NetworkCard />

      {/* v0.49c — scheduler self-test. Row of
          green/red dots per loop. Calls
          `schedulerSelfTestNow` IPC which reads
          the per-loop atomic last-tick counters. */}
      <SchedulerSelfTestCard />

      {/* v0.51a — CLOB feed status. Reads env
          var credentials + DB row counts; shows
          whether the real CLOB feed is
          configured and how many snapshots are
          cached locally. */}
      <ClobFeedCard />

      {/* v0.44c — paper trading mode toggle */}
      <PaperModeCard />

      {/* v0.48b — model degradation alert toggle */}
      <DegradationAlertCard />

      {/* v0.54b — storage path migration tool. Lets
          the user copy polyrocket.db + logs/ to a
          new path before restart. The "Restart
          required" flag from getStorageInfo gates
          the visibility of the migration button. */}
      <StorageMigrationCard />
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
function AppearanceCard() {
  // v0.74f — moved from sidebar (AppShell.tsx footer). The user
  // feedback was: "language switch, theme switch should be in
  // Settings" — these are not ambient context, they're
  // preferences the user wants to find and change deliberately.
  // Putting them in the sidebar footer implied "you might want
  // to switch this right now" which is the wrong call to action.
  //
  // **Two independent settings, one card**:
  //   - Theme: 3-way segmented (Dark / Light / Matrix), persisted
  //   - Language: 2-way dropdown (English / 简体中文), persisted
  // Both go through their own zustand stores with persist
  // middleware; the page never needs Save — changes are
  // immediate. The card is therefore "stateless" (no
  // draft / save / reset).
  const { t } = useT();
  return (
    <Card title={t('settings.appearance.title')} description={t('settings.appearance.desc')}>
      <div className="space-y-4">
        {/* Theme picker */}
        <div>
          <div className="text-[11px] text-muted mb-1.5">{t('settings.appearance.theme_label')}</div>
          <ThemeSwitcher />
        </div>
        {/* Language picker — inline buttons (we only have 2 locales) */}
        <div>
          <div className="text-[11px] text-muted mb-1.5">{t('settings.appearance.language_label')}</div>
          <LocalePicker />
        </div>
      </div>
    </Card>
  );
}

/**
 * `LocalePicker` — internal 2-way locale picker. Uses inline
 * buttons instead of a dropdown because we only support 2
 * locales (`en`, `zh`) and segmented control beats dropdown
 * on click count at 2 options. Mirrors the pattern from
 * `ThemeSwitcher`.
 */
function LocalePicker() {
  const locale = useLocaleStore((s) => s.locale);
  const setLocale = useLocaleStore((s) => s.setLocale);
  return (
    <div
      data-testid="locale-picker"
      className="inline-flex items-center gap-0.5 p-0.5 rounded-md border bg-surface-2 border-border"
    >
      {SUPPORTED_LOCALES.map((l: Locale) => {
        const active = l === locale;
        return (
          <button
            key={l}
            type="button"
            onClick={() => setLocale(l)}
            aria-pressed={active}
            aria-label={`Switch language to ${LOCALE_LABEL[l]}`}
            className={cn(
              'inline-flex items-center gap-1 px-2 py-1 text-xs rounded transition-colors',
              active
                ? 'bg-surface text-fg shadow-card'
                : 'text-muted hover:text-fg hover:bg-surface-hover',
            )}
          >
            <Globe className="w-3 h-3" />
            <span>{LOCALE_LABEL[l]}</span>
          </button>
        );
      })}
    </div>
  );
}

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

// =================================================================
// =============== v0.23c — Auto-promote margin panel ===============
// =================================================================

/** Card that lets the user configure the Brier margin
 *  for the auto-promote-if-better action.
 *
 *  The margin is stored in the UI prefs store (zustand +
 *  localStorage). It's a simple float; no IPC needed.
 *  Range: 0.001 (very aggressive) to 1.0 (effectively
 *  disabled). Default 0.005. */
function AutoPromoteCard() {
  const { t } = useT();
  const margin = usePrefsStore((s) => s.autoPromoteBrierMargin);
  const afterTrain = usePrefsStore((s) => s.autoPromoteAfterTrain);
  const notifyOnAutoPromote = usePrefsStore((s) => s.autoPromoteNotify);
  // v0.42e-2 — opt-in OS notification for the
  // "skipped" branch (i.e. the candidate wasn't
  // better than the active model). Most users
  // don't want this — it's the common case —
  // so default is off.
  const notifyOnAutoPromoteSkipped = usePrefsStore(
    (s) => s.autoPromoteSkippedNotify,
  );
  const setPref = usePrefsStore((s) => s.setPref);
  const [value, setValue] = useState<number>(margin);
  const [saved, setSaved] = useState(false);

  // Sync local form state when the persisted margin changes
  // (e.g. on mount, or after a reset). useState with a
  // function-form initializer is intentional: it runs only
  // on the first render, not on every state change.
  useState(() => {
    setValue(margin);
  });

  // v0.28c — when the user toggles "Auto-run after train",
  // push the new value to Rust immediately (no Save
  // button needed for a binary toggle). The Rust side
  // reads `auto_promote.enabled` inside `train_job` to
  // decide whether to spawn the worker.
  const onAfterTrainToggle = (next: boolean) => {
    setPref('autoPromoteAfterTrain', next);
    setAutoPromoteConfig({ enabled: next }).catch(() => {
      // Sidecar is not always available; the L1 store
      // is the source of truth, Rust will re-read on
      // the next Settings mount.
    });
  };

  // v0.39b — when the user toggles the desktop notification
  // flag, update the prefs store only (no Rust push
  // needed — the notification is L1-only, Rust doesn't
  // know about it).
  const onNotifyToggle = (next: boolean) => {
    setPref('autoPromoteNotify', next);
  };

  // v0.42e-2 — opt-in OS notification on the
  // "skipped" branch. Off by default.
  const onNotifySkippedToggle = (next: boolean) => {
    setPref('autoPromoteSkippedNotify', next);
  };

  const onSave = () => {
    if (!Number.isFinite(value) || value < 0) {
      setValue(margin);
      return;
    }
    setPref('autoPromoteBrierMargin', value);
    // v0.28c — also push the new margin to Rust so the
    // next train's auto-promote worker uses it.
    setAutoPromoteConfig({ brier_margin: value }).catch(() => {
      // Same as above: best-effort push.
    });
    setSaved(true);
    toast.success(t('auto_promote.margin.saved'));
    setTimeout(() => setSaved(false), 1500);
  };

  return (
    <Card
      title={t('auto_promote.title')}
      description={t('auto_promote.desc')}
    >
      <div className="space-y-3">
        <div>
          <Toggle
            data-testid="auto-promote-after-train-toggle"
            label={t('auto_promote.after_train.label')}
            checked={afterTrain}
            onChange={onAfterTrainToggle}
          />
          <p className="text-[10px] text-muted mt-1 ml-1">
            {t('auto_promote.after_train.desc')}
          </p>
        </div>
        {/* v0.39b — desktop notification toggle. Sits
            below "Auto-run after train" so the user
            sees the related options together. */}
        <div>
          <Toggle
            data-testid="auto-promote-notify-toggle"
            label={t('auto_promote.notify.label')}
            checked={notifyOnAutoPromote}
            onChange={onNotifyToggle}
          />
          <p className="text-[10px] text-muted mt-1 ml-1">
            {t('auto_promote.notify.desc')}
          </p>
        </div>
        {/* v0.42e-2 — opt-in OS notification for the
            "skipped" branch. Off by default. Sits
            below the main notify toggle so the two
            are visually grouped. */}
        <div>
          <Toggle
            data-testid="auto-promote-notify-skipped-toggle"
            label={t('auto_promote.notify_skipped.label')}
            checked={notifyOnAutoPromoteSkipped}
            onChange={onNotifySkippedToggle}
          />
          <p className="text-[10px] text-muted mt-1 ml-1">
            {t('auto_promote.notify_skipped.desc')}
          </p>
        </div>
        <NumberField
          label={t('auto_promote.margin.label')}
          hint={t('auto_promote.margin.hint')}
          value={value}
          onChange={setValue}
          min={0}
          max={1}
          step={0.001}
        />
        <div className="flex items-center gap-2">
          <Button
            data-testid="auto-promote-save-btn"
            size="sm"
            iconLeft={<Save className="w-3 h-3" />}
            onClick={onSave}
          >
            {t('settings.btn.save')}
          </Button>
          {saved && (
            <span
              data-testid="auto-promote-saved-badge"
              className="text-[10px] text-bull"
            >
              ✓ {t('auto_promote.margin.saved')}
            </span>
          )}
        </div>
      </div>
    </Card>
  );
}

/** v0.36b — Backup & restore card. Lets the user
 *  export their UI prefs to a JSON file, and
 *  import from a JSON file. Useful for:
 *   - Sharing a preferred config with other users
 *   - Backup before a re-install
 *   - Replicating the same config across machines
 *
 *  The export triggers a browser download. The
 *  import opens a file picker. On import, the
 *  prefs are validated; on success, the prefs
 *  store is updated and a toast is shown. On
 *  failure, an error toast with the message is
 *  shown.
 */
function RerunSetupCard() {
  const { t } = useT();
  const navigate = useNavigate();
  const reset = useWelcomeStore((s) => s.reset);
  return (
    <Card title={t('welcome.rerun_title')} description={t('welcome.rerun_desc')}>
      <div className="space-y-2">
        <p className="text-[12px] text-muted">
          {t('welcome.rerun_body')}
        </p>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => navigate('/welcome')}
            data-testid="rerun-setup"
            className="h-8 px-3 rounded-md text-[12px] font-medium bg-accent text-bg hover:bg-accent/90"
          >
            {t('welcome.rerun_button')}
          </button>
          <button
            type="button"
            onClick={() => {
              reset();
              navigate('/welcome');
            }}
            data-testid="rerun-setup-reset"
            className="h-8 px-3 rounded-md text-[12px] text-muted hover:text-fg"
          >
            {t('welcome.rerun_reset')}
          </button>
        </div>
      </div>
    </Card>
  );
}

function BackupRestoreCard() {
  const { t } = useT();
  const prefs = usePrefsStore();
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [importing, setImporting] = useState(false);

  const onExport = () => {
    // Snapshot the current prefs (excluding the
    // setPref/reset functions). The download utility
    // serializes the rest.
    const snapshot: UiPrefs = {
      defaultMinEdgePct: prefs.defaultMinEdgePct,
      defaultAllocationCapUsdc: prefs.defaultAllocationCapUsdc,
      copyTradingEnabled: prefs.copyTradingEnabled,
      notificationsEnabled: prefs.notificationsEnabled,
      advancedStats: prefs.advancedStats,
      autoPromoteBrierMargin: prefs.autoPromoteBrierMargin,
      autoPromoteAfterTrain: prefs.autoPromoteAfterTrain,
      autoPromoteNotify: prefs.autoPromoteNotify,
      autoPromoteSkippedNotify: prefs.autoPromoteSkippedNotify,
      telemetryEnabled: prefs.telemetryEnabled,
      mirrorPaperMode: prefs.mirrorPaperMode,
      degradationAlertNotify: prefs.degradationAlertNotify,
    };
    downloadPrefsAsFile(snapshot);
    toast.success(t('prefs.backup.exported'));
  };

  const onImportClick = () => {
    // Trigger the hidden file input
    fileInputRef.current?.click();
  };

  const onFileSelected = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setImporting(true);
    try {
      const text = await readFileAsText(file);
      const newPrefs = parsePrefsFromString(text);
      // Apply each pref. We use setPref for each
      // field so the store's zustand middleware
      // persists them to localStorage.
      for (const k of Object.keys(newPrefs) as (keyof UiPrefs)[]) {
        prefs.setPref(k, newPrefs[k]);
      }
      // v0.36b — also push the auto-promote config
      // to Rust so the next train's auto-promote
      // worker uses the imported values.
      setAutoPromoteConfig({
        enabled: newPrefs.autoPromoteAfterTrain,
        brier_margin: newPrefs.autoPromoteBrierMargin,
      }).catch(() => {
        // Best-effort; the L1 store is the source
        // of truth, Rust re-reads on next mount.
      });
      toast.success(t('prefs.backup.imported'));
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      toast.error(t('prefs.backup.import_failed'), message);
    } finally {
      setImporting(false);
      // Clear the input so the user can re-select
      // the same file (the change event would
      // otherwise not fire)
      if (fileInputRef.current) fileInputRef.current.value = '';
    }
  };

  return (
    <Card
      title={t('prefs.backup.title')}
      description={t('prefs.backup.desc')}
    >
      <div className="flex items-center gap-2" data-testid="backup-restore-card">
        <Button
          size="sm"
          variant="secondary"
          iconLeft={<Download className="w-3 h-3" />}
          onClick={onExport}
          data-testid="backup-export-btn"
        >
          {t('prefs.backup.export')}
        </Button>
        <Button
          size="sm"
          variant="secondary"
          iconLeft={<Upload className="w-3 h-3" />}
          onClick={onImportClick}
          loading={importing}
          data-testid="backup-import-btn"
        >
          {t('prefs.backup.import')}
        </Button>
        {/* Hidden file input; clicking the Import
            button triggers a click on this input. */}
        <input
          ref={fileInputRef}
          type="file"
          accept="application/json,.json"
          onChange={onFileSelected}
          style={{ display: 'none' }}
          data-testid="backup-import-input"
        />
      </div>
    </Card>
  );
}

// =================================================================
// ============== v0.42c — Telemetry opt-in card ===================
// =================================================================

/** v0.42c — opt-in lifecycle telemetry.
 *
 *  When ON, every Rust lifecycle event (train started /
 *  completed / failed, promote completed, scheduler
 *  tick, etc.) writes one NDJSON line to stderr. Capture
 *  with `polyrocket 2> telemetry.log`.
 *
 *  Default OFF. The user can flip this in Settings and
 *  the change takes effect immediately (the L1 pushes
 *  the new value to Rust via `setTelemetryEnabled`).
 *
 *  No PII, no model weights, no secrets. The events
 *  are coarse-grained lifecycle markers (job_id,
 *  loop_name, latency_ms) — see
 *  `src-tauri/src/infra/telemetry.rs` for the full
 *  event schema.
 */
function TelemetryCard() {
  const { t } = useT();
  const enabled = usePrefsStore((s) => s.telemetryEnabled);
  const setPref = usePrefsStore((s) => s.setPref);
  const [pushed, setPushed] = useState(false);

  // v0.42c — on mount, ask Rust what the current
  // effective state is. This handles the case where
  // the env var POLYROCKET_TELEMETRY=1 was set at
  // startup (the L1 store starts as false; Rust
  // starts as true; the toggle should reflect that).
  useEffect(() => {
    getTelemetryEnabled()
      .then((v) => {
        if (v !== usePrefsStore.getState().telemetryEnabled) {
          setPref('telemetryEnabled', v);
        }
      })
      .catch(() => {
        // Sidecar may be down during boot; default
        // stays as the L1 store value.
      });
  }, [setPref]);

  const onToggle = (next: boolean) => {
    setPref('telemetryEnabled', next);
    setTelemetryEnabled({ enabled: next })
      .then(() => {
        setPushed(true);
        setTimeout(() => setPushed(false), 1500);
      })
      .catch(() => {
        // best-effort; user can re-toggle.
        toast.error(t('telemetry.push_failed'));
      });
  };

  return (
    <Card
      title={t('telemetry.title')}
      description={t('telemetry.desc')}
    >
      <div className="space-y-3">
        <div>
          <Toggle
            data-testid="telemetry-toggle"
            label={t('telemetry.label')}
            checked={enabled}
            onChange={onToggle}
          />
          <p className="text-[10px] text-muted mt-1 ml-1">
            {t('telemetry.hint')}
          </p>
        </div>
        {/* v0.42c — capture hint. The user has to
            know how to actually capture the stream
            once telemetry is on. The text is in the
            i18n catalog (telemetry.capture_hint). */}
        <div className="text-[10px] text-muted bg-surface-2 rounded px-2 py-1.5 font-mono">
          {t('telemetry.capture_hint')}
        </div>
        {pushed && (
          <span
            data-testid="telemetry-pushed-badge"
            className="text-[10px] text-bull"
          >
            ✓ {t('telemetry.pushed')}
          </span>
        )}

        {/* v0.49a — on-disk session files. v0.49a
            persists events to a per-session JSONL file
            (one per process start). The user can see
            the inventory and purge old files. */}
        <TelemetryLogList enabled={enabled} />
      </div>
    </Card>
  );
}

/** v0.49a — sub-component: list the on-disk telemetry
 *  session files and let the user purge the old ones.
 *
 *  We always render the section, even when telemetry is
 *  off, because the file might still be there from a
 *  previous session (the log dir is created on first
 *  emit; before that the list is empty).
 *
 *  v0.62c — exported so tests can render it in
 *  isolation. Default: `enabled=true`.
 */
export function TelemetryLogList({ enabled = true }: { enabled?: boolean }) {
  const { t } = useT();
  const [logs, setLogs] = useState<
    Array<{
      name: string;
      path: string;
      sizeBytes: number;
      modifiedUnix: number;
      isCurrent: boolean;
    }>
  >([]);
  const [purged, setPurged] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(() => {
    listTelemetryLogs()
      .then(setLogs)
      .catch(() => setLogs([]));
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh, enabled]);

  const onPurge = () => {
    setBusy(true);
    purgeTelemetryLogs()
      .then((n) => {
        setPurged(n);
        setTimeout(() => setPurged(null), 2000);
        refresh();
      })
      .catch(() => {
        toast.error(t('telemetry.purge_failed'));
      })
      .finally(() => setBusy(false));
  };

  const fmtBytes = (n: number) => {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1024 / 1024).toFixed(2)} MB`;
  };

  const fmtDate = (unix: number) => {
    if (!unix) return '—';
    const d = new Date(unix * 1000);
    return d.toISOString().slice(0, 16).replace('T', ' ');
  };

  // v0.62c — total size + count summary at the top
  // of the list. Helps the user gauge how much disk
  // telemetry is using without scrolling through the
  // full list.
  const totalBytes = logs.reduce((s, l) => s + l.sizeBytes, 0);
  const currentCount = logs.filter((l) => l.isCurrent).length;

  return (
    <div className="border-t border-border pt-2 space-y-2">
      <div className="flex items-center justify-between">
        <h4 className="text-xs font-semibold text-fg">
          {t('telemetry.logs_title')}
        </h4>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={refresh}
            className="text-[10px] text-muted hover:text-fg"
            data-testid="telemetry-logs-refresh"
          >
            {t('common.refresh')}
          </button>
          <button
            type="button"
            onClick={onPurge}
            disabled={busy || logs.length === 0}
            className="text-[10px] text-bear hover:text-fg disabled:opacity-50"
            data-testid="telemetry-logs-purge"
          >
            {t('telemetry.purge_old')}
          </button>
        </div>
      </div>
      {/* v0.62c — disk usage summary. The user can
          see at a glance how much telemetry is on
          disk without scrolling. */}
      <div
        className="text-[10px] text-muted"
        data-testid="telemetry-logs-summary"
      >
        {logs.length} session{logs.length === 1 ? '' : 's'}
        {currentCount > 0 ? ` (${currentCount} current)` : ''}
        {' · '}
        {fmtBytes(totalBytes)} total
      </div>
      {logs.length === 0 ? (
        <p className="text-[10px] text-muted">
          {t('telemetry.logs_empty')}
        </p>
      ) : (
        <ul className="space-y-1" data-testid="telemetry-logs-list">
          {logs.map((l) => (
            <li
              key={l.path}
              className="text-[10px] text-muted flex items-center justify-between bg-surface-2 rounded px-2 py-1"
            >
              <span className="font-mono truncate flex-1">
                {l.name}
                {l.isCurrent && (
                  <span
                    data-testid="telemetry-log-current"
                    className="ml-2 text-bull"
                  >
                    ●
                  </span>
                )}
              </span>
              <span className="ml-2 whitespace-nowrap">
                {fmtBytes(l.sizeBytes)} · {fmtDate(l.modifiedUnix)}
              </span>
            </li>
          ))}
        </ul>
      )}
      {purged !== null && (
        <span
          data-testid="telemetry-purged-badge"
          className="text-[10px] text-bull"
        >
          ✓ {t('telemetry.purged', { n: purged })}
        </span>
      )}
    </div>
  );
}

// =================================================================
// ============== v0.44c — Paper trading mode card =================
// =================================================================

/** v0.44c — paper trading mode toggle.
 *
 *  When ON, the mirror executor's picked orders
 *  go to the `paper_fills` table instead of `bets`,
 *  and the CLOB sign_order step is skipped. The
 *  decision logic (sizing, exposure caps,
 *  frequency) is unchanged. The user can validate
 *  their config without risking real money.
 *
 *  Default OFF. The L1 pushes the value to Rust
 *  on Settings mount and on every toggle via
 *  `setMirrorPaperMode`.
 */

// ================================================================
// ============ v0.49b — Active model summary card =================
// ================================================================

/** v0.51a — CLOB feed status card. Shows whether
 *  the real order-book feed is configured
 *  (POLYROCKET_CLOB_API_KEY + SECRET + PASSPHRASE
 *  env vars) and how many snapshots are cached
 *  locally. The live WebSocket listener lands
 *  in v0.51+; v0.51a only lays down the schema +
 *  IPCs + this card.
 */
function ClobFeedCard() {
  const { t } = useT();
  type Status = {
    state: 'not_configured' | 'configured' | 'connected';
    totalSnapshots: number;
    marketsWithSnapshots: number;
  };
  const [status, setStatus] = useState<Status | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const refresh = useCallback(() => {
    clobFeedStatus()
      .then((s) => {
        setStatus(s);
        setErr(null);
      })
      .catch((e) => setErr(String(e)));
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return (
    <Card
      title={t('clob.title')}
      description={t('clob.desc')}
    >
      <div className="space-y-2" data-testid="clob-feed-card">
        {err ? (
          <p
            data-testid="clob-feed-error"
            className="text-[10px] text-bear"
          >
            {err}
          </p>
        ) : !status ? (
          <p className="text-[10px] text-muted">{t('common.loading')}</p>
        ) : (
          <>
            <div className="flex items-baseline gap-2">
              <span
                data-testid="clob-feed-state"
                className={
                  'text-[11px] ' +
                  (status.state === 'connected'
                    ? 'text-bull'
                    : status.state === 'configured'
                      ? 'text-warn'
                      : 'text-muted')
                }
              >
                ● {t(`clob.${status.state}`)}
              </span>
              <span className="text-[10px] text-muted">
                {t('clob.snapshots_count', {
                  n: status.totalSnapshots,
                  markets: status.marketsWithSnapshots,
                })}
              </span>
            </div>
          </>
        )}
        <button
          type="button"
          onClick={refresh}
          className="text-[10px] text-muted hover:text-fg"
          data-testid="clob-feed-refresh"
        >
          {t('common.refresh')}
        </button>
      </div>
    </Card>
  );
}


/** v0.49c — scheduler self-test. Reads the
 *  process-global atomic counters in
 *  `infra::scheduler::self_test` and renders a row
 *  of green/red dots per loop. Refresh button
 *  forces a re-poll. Loops that have never
 *  ticked (still in their initial stagger sleep)
 *  are rendered yellow with "starting...".
 */
function SchedulerSelfTestCard() {
  const { t } = useT();
  type Loop = {
    name: string;
    lastTickUnixMs: number;
    ageMs: number | null;
    healthy: boolean;
  };
  type Snapshot = {
    processStartedAtUnix: number;
    checkedAtUnixMs: number;
    allHealthy: boolean;
    loops: Loop[];
  };
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(() => {
    setBusy(true);
    schedulerSelfTestNow()
      .then((s) => setSnap(s))
      .catch(() => setSnap(null))
      .finally(() => setBusy(false));
  }, []);

  useEffect(() => {
    refresh();
    // Auto-poll every 30s so the dots update without
    // a manual click. Cheap (atomic reads).
    const id = setInterval(refresh, 30_000);
    return () => clearInterval(id);
  }, [refresh]);

  const fmtAge = (ms: number | null) => {
    if (ms == null) return t('scheduler.never');
    if (ms < 1000) return `${ms}ms`;
    if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
    if (ms < 3_600_000) return `${(ms / 60_000).toFixed(1)}m`;
    return `${(ms / 3_600_000).toFixed(1)}h`;
  };

  return (
    <Card
      title={t('scheduler.self_test_title')}
      description={t('scheduler.self_test_desc')}
    >
      <div className="space-y-3" data-testid="scheduler-self-test">
        <div className="flex items-center justify-between">
          <span
            className={
              'text-[11px] ' +
              (snap?.allHealthy ? 'text-bull' : snap ? 'text-warn' : 'text-muted')
            }
            data-testid="scheduler-overall"
          >
            {snap
              ? snap.allHealthy
                ? '● ' + t('scheduler.all_healthy')
                : '● ' + t('scheduler.some_unhealthy')
              : t('common.loading')}
          </span>
          <button
            type="button"
            onClick={refresh}
            disabled={busy}
            className="text-[10px] text-muted hover:text-fg disabled:opacity-50"
            data-testid="scheduler-self-test-refresh"
          >
            {t('common.refresh')}
          </button>
        </div>
        {snap && (
          <ul className="space-y-1" data-testid="scheduler-loops-list">
            {snap.loops.map((l) => (
              <li
                key={l.name}
                className="text-[10px] flex items-center gap-2 bg-surface-2 rounded px-2 py-1"
                data-testid={`scheduler-loop-${l.name}`}
              >
                <span
                  className={
                    'shrink-0 ' +
                    (l.ageMs == null
                      ? 'text-warn'
                      : l.healthy
                        ? 'text-bull'
                        : 'text-bear')
                  }
                >
                  ●
                </span>
                <span className="font-mono flex-1 truncate">{l.name}</span>
                <span className="text-muted whitespace-nowrap">
                  {fmtAge(l.ageMs)}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </Card>
  );
}

/** v0.49b — read-only card showing what the
 *  Rust side considers the "current" model.
 *
 *  The data comes from `getActiveModel` IPC, which
 *  reads `<sidecar model dir>/active.json`. This is
 *  the same view the v0.48a degradation detector
 *  sees (v0.49b refactored both to share the helper).
 *
 *  Pre-first-promote state: `null` is shown with a
 *  "no model promoted yet" hint.
 *
 *  The card is NOT a control — there's no promote
 *  action here. Promotion happens on the Model Lab
 *  page; this card is just an at-a-glance summary.
 */
function ActiveModelCard() {
  const { t } = useT();
  type ModelInfo = {
    modelVersion: string;
    bestBrier: number | null;
    bestParams: Record<string, unknown> | null;
    promotedAtMs: number | null;
    weights: number[] | null;
    sourcePath: string;
  };
  const [model, setModel] = useState<ModelInfo | null | undefined>(undefined);
  const [err, setErr] = useState<string | null>(null);

  const refresh = useCallback(() => {
    setModel(undefined);
    getActiveModel()
      .then((m) => {
        setModel(m);
        setErr(null);
      })
      .catch((e) => {
        setErr(String(e));
        setModel(null);
      });
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const fmtDate = (ms: number | null) => {
    if (!ms) return '—';
    const d = new Date(ms);
    return d.toISOString().slice(0, 16).replace('T', ' ');
  };

  return (
    <Card
      title={t('active_model.title')}
      description={t('active_model.desc')}
    >
      {model === undefined ? (
        <p className="text-[10px] text-muted">{t('common.loading')}</p>
      ) : err ? (
        <p
          data-testid="active-model-error"
          className="text-[10px] text-bear"
        >
          {t('active_model.error', { msg: err })}
        </p>
      ) : model === null ? (
        <div className="space-y-2">
          <p
            data-testid="active-model-empty"
            className="text-[10px] text-muted"
          >
            {t('active_model.empty')}
          </p>
          <button
            type="button"
            onClick={refresh}
            className="text-[10px] text-muted hover:text-fg"
            data-testid="active-model-refresh"
          >
            {t('common.refresh')}
          </button>
        </div>
      ) : (
        <div className="space-y-2" data-testid="active-model-summary">
          <Row label={t('active_model.version')} value={model.modelVersion} mono />
          <Row
            label={t('active_model.brier')}
            value={
              model.bestBrier == null
                ? '—'
                : model.bestBrier.toFixed(4)
            }
          />
          <Row
            label={t('active_model.promoted_at')}
            value={fmtDate(model.promotedAtMs)}
          />
          <Row
            label={t('active_model.source')}
            value={model.sourcePath}
            mono
            truncate
          />
          <div className="flex items-center gap-2 pt-1">
            <button
              type="button"
              onClick={refresh}
              className="text-[10px] text-muted hover:text-fg"
              data-testid="active-model-refresh"
            >
              {t('common.refresh')}
            </button>
          </div>
        </div>
      )}
    </Card>
  );
}

function Row({
  label,
  value,
  mono,
  truncate,
}: {
  label: string;
  value: string;
  mono?: boolean;
  truncate?: boolean;
}) {
  return (
    <div className="flex items-baseline gap-2 text-[11px]">
      <span className="text-muted shrink-0">{label}</span>
      <span
        className={
          (mono ? 'font-mono ' : '') +
          (truncate ? 'truncate ' : '') +
          'text-fg'
        }
        title={value}
      >
        {value}
      </span>
    </div>
  );
}

function PaperModeCard() {
  const { t } = useT();
  const enabled = usePrefsStore((s) => s.mirrorPaperMode);
  const setPref = usePrefsStore((s) => s.setPref);
  const [pushed, setPushed] = useState(false);

  // v0.44c — on mount, ask Rust what the current
  // effective state is (in case the env var set
  // it at startup).
  useEffect(() => {
    getMirrorPaperMode()
      .then((v) => {
        if (v !== usePrefsStore.getState().mirrorPaperMode) {
          setPref('mirrorPaperMode', v);
        }
      })
      .catch(() => {
        // best-effort
      });
  }, [setPref]);

  const onToggle = (next: boolean) => {
    setPref('mirrorPaperMode', next);
    setMirrorPaperMode({ enabled: next })
      .then(() => {
        setPushed(true);
        setTimeout(() => setPushed(false), 1500);
      })
      .catch(() => {
        toast.error(t('paper_mode.push_failed'));
      });
  };

  return (
    <Card
      title={t('paper_mode.title')}
      description={t('paper_mode.desc')}
    >
      <div className="space-y-3">
        <div>
          <Toggle
            data-testid="mirror-paper-mode-toggle"
            label={t('paper_mode.label')}
            checked={enabled}
            onChange={onToggle}
          />
          <p className="text-[10px] text-muted mt-1 ml-1">
            {t('paper_mode.hint')}
          </p>
        </div>
        {pushed && (
          <span
            data-testid="paper-mode-pushed-badge"
            className="text-[10px] text-bull"
          >
            ✓ {t('paper_mode.pushed')}
          </span>
        )}
      </div>
    </Card>
  );
}

// =================================================================
// ============== v0.48b — Model degradation alert card ============
// =================================================================

/** v0.48b — opt-in OS notification for model
 *  degradation alerts. The 7th scheduler loop
 *  runs every hour, computes the live Brier of
 *  the FALLBACK model on recent resolved
 *  markets, and emits a telemetry event with
 *  `alert=true` when live > train + threshold.
 *  When this pref is on, the L1 fires a real OS
 *  notification on those events. Default ON.
 */
function DegradationAlertCard() {
  const { t } = useT();
  const enabled = usePrefsStore((s) => s.degradationAlertNotify);
  const setPref = usePrefsStore((s) => s.setPref);

  return (
    <Card
      title={t('degradation.title')}
      description={t('degradation.desc')}
    >
      <div className="space-y-3">
        <Toggle
          data-testid="degradation-alert-toggle"
          label={t('degradation.label')}
          checked={enabled}
          onChange={(next) => setPref('degradationAlertNotify', next)}
        />
        <p className="text-[10px] text-muted mt-1 ml-1">
          {t('degradation.hint')}
        </p>
      </div>
    </Card>
  );
}

// v0.54b — storage migration tool. Surfaces the
// "Restart required" state from getStorageInfo
// and gives the user a one-click "Copy existing
// data to new path" button. After the migration
// completes, the next launch already finds the
// data at the new location (no empty-DB surprise).
function StorageMigrationCard() {
  const { t } = useT();
  const qc = useQueryClient();
  const info = useQuery({
    queryKey: ['storage-info'],
    queryFn: () => getStorageInfo(),
    staleTime: 0,
  });
  const [busy, setBusy] = useState(false);
  const [overwrite, setOverwrite] = useState(false);

  const onMigrate = useCallback(async () => {
    if (!info.data) return;
    setBusy(true);
    try {
      const res = await migrateStoragePath(
        info.data.currentPath,
        overwrite,
      );
      if (res.noop) {
        toast.info(t('storage.migrate_noop'));
      } else {
        toast.success(
          t('storage.migrate_ok', {
            files: res.filesCopied,
            bytes: res.bytesCopied,
          }),
        );
      }
      qc.invalidateQueries({ queryKey: ['storage-info'] });
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }, [info.data, overwrite, qc, t]);

  // The card is only relevant when the user has
  // picked a custom path AND the active session
  // is still on the default path (restart
  // required). Otherwise there's nothing to
  // migrate.
  const visible = info.data?.restartRequired ?? false;
  if (!visible) return null;

  return (
    <Card
      title={t('storage.migrate_title')}
      description={t('storage.migrate_desc')}
    >
      <div className="space-y-3">
        <div className="text-[11px] text-muted space-y-1">
          <div className="flex items-center gap-2">
            <span className="text-fg">{t('storage.migrate_from')}</span>
            <code className="font-mono text-[10px]">{info.data?.defaultPath}</code>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-fg">{t('storage.migrate_to')}</span>
            <code className="font-mono text-[10px]">{info.data?.currentPath}</code>
          </div>
        </div>
        <Toggle
          data-testid="storage-migrate-overwrite"
          label={t('storage.migrate_overwrite')}
          checked={overwrite}
          onChange={setOverwrite}
        />
        <button
          type="button"
          onClick={onMigrate}
          disabled={busy}
          data-testid="storage-migrate-now"
          className="h-8 px-3 rounded-md text-[12px] font-medium bg-accent text-bg hover:bg-accent/90 disabled:opacity-50 flex items-center gap-1.5"
        >
          <CopyIcon className="w-3.5 h-3.5" />
          {busy ? t('storage.migrate_busy') : t('storage.migrate_now')}
        </button>
        <p className="text-[10px] text-muted">
          {t('storage.migrate_hint')}
        </p>
      </div>
    </Card>
  );
}

// v0.55 — model explainability. Lets the user
// pick a sample (price + age) and see the
// per-feature contribution to the active model's
// prediction. Surfaces the SHAP-like
// decomposition of the 3-feature logistic model.
// v0.59 — ExplainabilityCard is exported for
// the dedicated test (src/routes/
// ExplainabilityCard.test.tsx). It's also
// used internally by Settings as before.
export function ExplainabilityCard() {
  const { t } = useT();
  const active = useQuery({
    queryKey: ['active-model'],
    queryFn: () => getActiveModel(),
    staleTime: 30_000,
  });
  const [price, setPrice] = useState(0.5);
  const [age, setAge] = useState(24);
  // v0.59 — toggle between SHAP and the v0.55
  // exact-decomposition. SHAP is the default
  // because it satisfies the efficiency axiom
  // and is what users coming from
  // shap-library / interpret-ml expect.
  const [useShap, setUseShap] = useState(true);
  const [busy, setBusy] = useState(false);
  const [derivResult, setDerivResult] = useState<Awaited<
    ReturnType<typeof explainModel>
  > | null>(null);
  const [shapResult, setShapResult] = useState<Awaited<
    ReturnType<typeof shapExplain>
  > | null>(null);

  const onExplain = useCallback(async () => {
    if (!active.data) return;
    setBusy(true);
    try {
      if (useShap) {
        const r = await shapExplain(active.data.modelVersion, {
          price,
          market_age_hours: age,
        });
        setShapResult(r);
      } else {
        const r = await explainModel(active.data.modelVersion, {
          price,
          market_age_hours: age,
        });
        setDerivResult(r);
      }
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }, [active.data, price, age, useShap]);

  if (!active.data) return null;
  // Pick the active result based on the toggle.
  const result = useShap ? shapResult : derivResult;
  return (
    <Card
      title={t('explain.title')}
      description={t('explain.desc')}
    >
      <div className="space-y-3">
        <div className="grid grid-cols-2 gap-2">
          <div>
            <label className="text-[10px] text-muted">
              {t('explain.price_label')}
            </label>
            <input
              type="number"
              step="0.01"
              min={0}
              max={1}
              value={price}
              onChange={(e) => setPrice(Number(e.target.value))}
              data-testid="explain-price-input"
              className="w-full h-8 px-2.5 rounded-md text-[12px] bg-surface text-fg border border-border focus:outline-none focus:ring-2 focus:ring-accent font-mono"
            />
          </div>
          <div>
            <label className="text-[10px] text-muted">
              {t('explain.age_label')}
            </label>
            <input
              type="number"
              step="0.5"
              min={0}
              value={age}
              onChange={(e) => setAge(Number(e.target.value))}
              data-testid="explain-age-input"
              className="w-full h-8 px-2.5 rounded-md text-[12px] bg-surface text-fg border border-border focus:outline-none focus:ring-2 focus:ring-accent font-mono"
            />
          </div>
        </div>
        {/* v0.59 — SHAP vs derivative toggle. */}
        <div className="flex items-center gap-1 p-0.5 bg-surface-2 rounded-md w-fit">
          <button
            type="button"
            onClick={() => setUseShap(true)}
            data-testid="explain-method-shap"
            className={cn(
              'h-6 px-3 rounded text-[11px] font-medium transition-colors',
              useShap
                ? 'bg-accent text-bg'
                : 'text-muted hover:text-fg',
            )}
          >
            {t('explain.use_shap')}
          </button>
          <button
            type="button"
            onClick={() => setUseShap(false)}
            data-testid="explain-method-deriv"
            className={cn(
              'h-6 px-3 rounded text-[11px] font-medium transition-colors',
              !useShap
                ? 'bg-accent text-bg'
                : 'text-muted hover:text-fg',
            )}
          >
            {t('explain.use_deriv')}
          </button>
        </div>
        <button
          type="button"
          onClick={onExplain}
          disabled={busy}
          data-testid="explain-run"
          className="h-8 px-3 rounded-md text-[12px] font-medium bg-accent text-bg hover:bg-accent/90 disabled:opacity-50"
        >
          {busy ? t('explain.busy') : t('explain.run')}
        </button>
        {result && (
          <div
            data-testid="explain-result"
            data-method={useShap ? 'shap' : 'derivative'}
            className="space-y-1.5 pt-2 border-t border-border"
          >
            {/* v0.59 — when SHAP, show baseline +
               target + efficiency diff. The
               derivative view only shows the
               prediction. */}
            {useShap && shapResult && (
              <div className="text-[10px] text-muted flex items-center gap-3">
                <span>
                  {t('explain.method')}: <code className="text-fg">{shapResult.method}</code>
                </span>
                <span>
                  {t('explain.shap_baseline')}:{' '}
                  <code className="text-fg font-mono">
                    {shapResult.baseline_prediction?.toFixed(4) ?? '—'}
                  </code>
                </span>
                <span>
                  {t('explain.prediction', { p: shapResult.target_prediction?.toFixed(4) ?? '—' })}
                </span>
              </div>
            )}
            {!useShap && derivResult && (
              <div className="text-[11px] text-fg font-medium">
                {t('explain.prediction', { p: derivResult.prediction?.toFixed(4) ?? '—' })}
              </div>
            )}
            <div className="space-y-1">
              {result.features.map((f) => {
                // SHAP features use `shap_value`/
                // `abs_shap`; derivative features
                // use `contribution`/`abs_contribution`.
                // Normalize to a single (value,
                // abs) pair for rendering.
                const fAny = f as unknown as {
                  feature: string;
                  value: number;
                  weight: number;
                  contribution?: number;
                  abs_contribution?: number;
                  shap_value?: number;
                  abs_shap?: number;
                };
                const v = fAny.shap_value ?? fAny.contribution ?? 0;
                const av = fAny.abs_shap ?? fAny.abs_contribution ?? Math.abs(v);
                return (
                  <div
                    key={fAny.feature}
                    data-testid={`explain-row-${fAny.feature}`}
                    className="flex items-center gap-2 text-[10px]"
                  >
                    <span className="w-32 truncate text-muted">
                      {fAny.feature}
                    </span>
                    <div className="flex-1 h-3 bg-surface-2 rounded-sm overflow-hidden relative">
                      {/* Centered bar: positive right, negative left. */}
                      <div
                        className={cn(
                          'absolute top-0 h-full',
                          v >= 0
                            ? 'bg-bull left-1/2'
                            : 'bg-bear right-1/2',
                        )}
                        style={{
                          width: `${Math.min(50, av * 200)}%`,
                        }}
                      />
                      <div className="absolute left-1/2 top-0 bottom-0 w-px bg-border" />
                    </div>
                    <span
                      className={cn(
                        'w-16 text-right font-mono',
                        v >= 0 ? 'text-bull' : 'text-bear',
                      )}
                    >
                      {v >= 0 ? '+' : ''}
                      {v.toFixed(4)}
                    </span>
                  </div>
                );
              })}
            </div>
            {/* v0.59 — surface the SHAP efficiency
                residual so the user can sanity-
                check the math. */}
            {useShap && shapResult && (
              <p className="text-[10px] text-muted pt-1">
                {t('explain.shap_efficiency', {
                  diff: shapResult.efficiency_diff?.toFixed(6) ?? '—',
                })}
              </p>
            )}
            <p className="text-[10px] text-muted pt-1">
              {t('explain.hint')}
            </p>
          </div>
        )}
      </div>
    </Card>
  );
}

// v0.56 — network proxy / Tor configuration.
// Lets the user route all outbound HTTP
// (LLM clients, Polymarket CLOB, sidecar HTTP
// if any) through a proxy. The two supported
// schemes are http:// (HTTP CONNECT) and
// socks5:// (e.g. Tor SOCKS5 on
// 127.0.0.1:9050). Restart required for
// changes to take effect (the shared
// reqwest::Client is built at startup).
function NetworkCard() {
  const { t } = useT();
  const qc = useQueryClient();
  const cfg = useQuery({
    queryKey: ['proxy-config'],
    queryFn: () => getProxyConfig(),
    staleTime: 0,
  });
  const [url, setUrl] = useState('');
  const [enabled, setEnabled] = useState(false);
  const [busy, setBusy] = useState(false);
  // Sync form when the IPC returns.
  useEffect(() => {
    if (cfg.data) {
      setUrl(cfg.data.url ?? '');
      setEnabled(cfg.data.enabled);
    }
  }, [cfg.data]);
  const onSave = useCallback(async () => {
    setBusy(true);
    try {
      await setProxyConfig(enabled, url.trim() || null);
      toast.success(t('network.saved'));
      qc.invalidateQueries({ queryKey: ['proxy-config'] });
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }, [enabled, url, qc, t]);
  const onClear = useCallback(async () => {
    setBusy(true);
    try {
      await clearProxyConfig();
      setUrl('');
      setEnabled(false);
      toast.success(t('network.cleared'));
      qc.invalidateQueries({ queryKey: ['proxy-config'] });
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }, [qc, t]);
  return (
    <Card
      title={t('network.title')}
      description={t('network.desc')}
    >
      <div className="space-y-3">
        <Toggle
          data-testid="network-proxy-enabled"
          label={t('network.enabled_label')}
          checked={enabled}
          onChange={setEnabled}
        />
        <div>
          <label className="text-[10px] text-muted">
            {t('network.url_label')}
          </label>
          <input
            type="text"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="socks5://127.0.0.1:9050"
            data-testid="network-proxy-url"
            className="w-full h-8 px-2.5 rounded-md text-[12px] bg-surface text-fg border border-border focus:outline-none focus:ring-2 focus:ring-accent font-mono"
          />
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={onSave}
            disabled={busy}
            data-testid="network-proxy-save"
            className="h-8 px-3 rounded-md text-[12px] font-medium bg-accent text-bg hover:bg-accent/90 disabled:opacity-50 flex items-center gap-1.5"
          >
            <Globe className="w-3.5 h-3.5" />
            {busy ? t('network.busy') : t('network.save')}
          </button>
          <button
            type="button"
            onClick={onClear}
            disabled={busy}
            data-testid="network-proxy-clear"
            className="h-8 px-3 rounded-md text-[12px] font-medium border border-border bg-surface-2 text-fg hover:bg-surface-hover disabled:opacity-50"
          >
            {t('network.clear')}
          </button>
        </div>
        <p className="text-[10px] text-muted">
          {t('network.hint')}
        </p>
      </div>
    </Card>
  );
}
