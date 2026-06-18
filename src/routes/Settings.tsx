import { useState, useEffect, useRef } from 'react';
import { Settings as SettingsIcon, RotateCcw, Save, Database, Bell, Eye, FlaskConical, Trash2, Download, Upload, Activity } from 'lucide-react';
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
  setAutoPromoteConfig,
  setTelemetryEnabled,
  getTelemetryEnabled,
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

      {/* v0.42c — opt-in lifecycle telemetry */}
      <TelemetryCard />

      {/* v0.44c — paper trading mode toggle */}
      <PaperModeCard />

      {/* v0.48b — model degradation alert toggle */}
      <DegradationAlertCard />
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
      </div>
    </Card>
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
