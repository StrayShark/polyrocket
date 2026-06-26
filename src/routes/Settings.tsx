import { useState, useEffect, useRef, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { Settings as SettingsIcon, RotateCcw, Save, Database, Bell, Eye, FlaskConical, Trash2, Download, Upload, Copy as CopyIcon, Globe } from 'lucide-react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Card } from '@/components/base/Card';
import { Button } from '@/components/base/Button';
import { BadgePill } from '@/components/base/BadgePill';
import { Input } from '@/components/base/Input';
import { Toggle } from '@/components/base/Toggle';
import { Skeleton } from '@/components/feedback/Skeleton';
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
  listWallets, // v0.79c — bankroll config card
  getStorageInfo, // v0.54b
  migrateStoragePath, // v0.54b
  explainModel, // v0.55
  shapExplain, // v0.59
  getProxyConfig, // v0.56
  setProxyConfig, // v0.56
  clearProxyConfig, // v0.56
  setMirrorPaperMode,
  getMirrorPaperMode,
  // v0.78 —— bankroll 分配(M11)
  getBankrollConfig,
  type AuditRetentionView,
  type SetAuditRetentionArgs,
} from '@/ipc';
import { formatRetentionAge, fmtPct } from '@/lib/format';
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

  // v0.28c — 在 Settings 页挂载时将 auto-promote 配置
  // 推送到 Rust。这是 L1 zustand store（UI 的单一
  // 数据源）与 Rust 的 `AppState.auto_promote` 字段
  // （`train_job` 的消费者）之间的桥接。
  //
  // 我们也会在用户更改配置时重新推送（在 AutoPromoteCard
  // 中处理）。仅在挂载时推送已足够覆盖常见场景：
  // 「用户首次打开 Settings，L1 store 持有持久化的
  // 值，我们将其推送到 Rust」。
  useEffect(() => {
    setAutoPromoteConfig({
      enabled: prefs.autoPromoteAfterTrain,
      brier_margin: prefs.autoPromoteBrierMargin,
    }).catch(() => {
      // Sidecar 并不总是可用；L1 store 是单一数据源，
      // Rust 会在下次 Settings 挂载时重新读取。
    });
    // 仅在挂载时运行一次。重新运行只会重新推送相同的
    // 值；无害但浪费。
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
            <h2 className="text-title-sm font-semibold text-fg">{t('settings.title')}</h2>
          </div>
          <div className="flex items-center gap-2">
            <Button data-testid="prefs-reset-btn" variant="ghost" size="sm" iconLeft={<RotateCcw className="w-3 h-3" />} onClick={reset}>
              {t('settings.btn.reset')}
            </Button>
            <Button data-testid="prefs-save-btn" variant="primary" size="sm" iconLeft={<Save className="w-3 h-3" />} onClick={save} disabled={!dirty}>
              {t('settings.btn.save')}
            </Button>
          </div>
        </div>
      </Card>

      {/* v0.74f — 外观设置（主题 + 语言）。
       *       根据用户反馈「语言切换、主题切换应该在 Settings 中」，
       *       从侧边栏底部（AppShell）迁移至此。
       *       位置紧跟页面标题之后，让用户能够
       *       立即找到最基本的 UI 偏好。 */}
      <AppearanceCard />

      {/* 交易默认值 */}
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

      {/* 通知 */}
      <Card title={t('settings.section.notifications')} description={t('settings.section.notifications_desc')}>
        <ToggleRow
          icon={Bell}
          label={t('settings.field.toasts')}
          hint={t('settings.field.toasts_hint')}
          checked={draft.notificationsEnabled}
          onChange={(v) => setDraft({ ...draft, notificationsEnabled: v })}
        />
      </Card>

      {/* Copy trading（跟单交易） */}
      <Card title={t('settings.section.copy')} description={t('settings.section.copy_desc')}>
        <ToggleRow
          data-testid="copy-trading-toggle"
          icon={Database}
          label={t('settings.field.copy_enabled')}
          hint={t('settings.field.copy_enabled_hint')}
          checked={draft.copyTradingEnabled}
          onChange={(v) => setDraft({ ...draft, copyTradingEnabled: v })}
        />
      </Card>

      {/* 高级 */}
      <Card title={t('settings.section.advanced')} description={t('settings.section.advanced_desc')}>
        <ToggleRow
          data-testid="advanced-stats-toggle"
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
            // v0.14b —— env_note 字符串嵌入两个 `<code>` 标签
            //（POLYROCKET_ENV=dev、POLYROCKET_KEYRING_ONLY=0）。
            // 中英文 markup 完全相同，因此使用
            // dangerouslySetInnerHTML 以保持 locale 同步。
            dangerouslySetInnerHTML={{ __html: t('settings.storage.env_note') }}
          />
        </div>
      </Card>

      {/* v0.13c — 审计保留策略（用户级覆盖） */}
      <RetentionCard />

      {/* v0.23c — 自动提升 margin */}
      <AutoPromoteCard />

      {/* v0.36b — UI 偏好的导入/导出 */}
      <BackupRestoreCard />

      {/* v0.53b — 重新运行 setup。允许用户随时
          重新访问 /welcome 以完成或重新配置。
          当密钥被轮换或用户想要切换存储路径
          时非常有用。 */}
      <RerunSetupCard />

      {/* v0.42c — opt-in 生命周期遥测 */}
      <TelemetryCard />

      {/* v0.49b — active model 摘要卡片。展示
          当前 active model 的训练指标
          （version、Brier、mtime）并显示磁盘
          路径。通过 `getActiveModel` IPC 获取，
          与 Rust 看到的视图一致。 */}
      <ActiveModelCard />

      {/* v0.55 — model 可解释性。让用户选择一个
          样本（price + age）并查看对 active model
          预测的逐特征贡献。展示 3 特征 logistic
          model 的类 SHAP 分解。 */}
      <ExplainabilityCard />

      {/* v0.56 — 网络代理 / Tor 支持。
          为共享的 reqwest::Client 和 sidecar
          子进程配置 `POLYROCKET_PROXY`。
          更改需要重启才能生效。 */}
      <NetworkCard />

      {/* v0.49c — 调度器自检。每个 loop 一行
          绿/红点。调用 `schedulerSelfTestNow`
          IPC 读取每个 loop 的原子 last-tick
          计数器。 */}
      <SchedulerSelfTestCard />

      {/* v0.51a — CLOB feed 状态。读取环境
          变量凭据 + DB 行数；展示真实 CLOB feed
          是否已配置以及本地缓存了多少快照。 */}
      <ClobFeedCard />

      {/* v0.44c — paper trading 模式切换 */}
      <PaperModeCard />

      {/* v0.48b — model 降级告警切换 */}
      <DegradationAlertCard />

      {/* v0.54b — 存储路径迁移工具。让用户在
          重启前将 polyrocket.db + logs/ 复制到
          新路径。getStorageInfo 返回的
          「Restart required」标志决定迁移按钮
          是否可见。 */}
      <StorageMigrationCard />

      {/* v0.79c — bankroll 分配配置。只读摘要，
         链接到 /bankroll 进行编辑。我们不在此处
         复制滑块 UI，因为 Bankroll 页面已有完整
         的编辑器 + apply 流程。 */}
      <BankrollConfigCard />
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
  'data-testid': testId,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  'data-testid'?: string;
}) {
  return (
    <div className="flex items-start gap-3 py-1" data-testid={testId}>
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
// ============== v0.13c — Audit retention 面板 ===================
// =================================================================

/** 允许用户覆盖 audit-log 保留策略的卡片。
 *  默认值（来自 `RetentionPolicy::default()`）在 hint 中以
 *  `(default)` 标签展示；任何非默认值都会触发 Save，
 *  进而调用 `set_audit_retention` IPC，该调用会立即按新
 *  策略执行清理。 */
function AppearanceCard() {
  // v0.74f — 从侧边栏（AppShell.tsx 底部）迁移而来。
  // 用户反馈：「语言切换、主题切换应该在 Settings 中」
  // —— 这些不是环境背景，而是用户希望找到并主动
  // 修改的偏好。放在侧边栏底部暗示「你现在可能想
  // 切换」，这并非正确的行动号召。
  //
  // **两个独立设置，一张卡片**：
  //   - Theme：3 段分段控件（Dark / Light / Matrix），持久化
  //   - Language：2 向下拉（English / 简体中文），持久化
  // 两者各自通过带 persist middleware 的 zustand store；
  // 页面永远不需要 Save —— 更改立即生效。因此该卡片
  // 是「无状态」的（无 draft / save / reset）。
  const { t } = useT();
  return (
    <Card title={t('settings.appearance.title')} description={t('settings.appearance.desc')}>
      <div className="space-y-4">
        {/* Theme 选择器 */}
        <div>
          <div className="text-[11px] text-muted mb-1.5">{t('settings.appearance.theme_label')}</div>
          <ThemeSwitcher />
        </div>
        {/* Language 选择器 —— 内联按钮（仅 2 种 locale） */}
        <div>
          <div className="text-[11px] text-muted mb-1.5">{t('settings.appearance.language_label')}</div>
          <LocalePicker />
        </div>
      </div>
    </Card>
  );
}

/**
 * `LocalePicker` —— 内部 2 向 locale 选择器。使用内联按钮
 * 而非下拉，因为仅支持 2 种 locale（`en`、`zh`），且
 * 2 个选项时分段控件比下拉点击次数更少。复刻
 * `ThemeSwitcher` 的模式。
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
              'inline-flex items-center gap-1 px-2 py-1 text-xs rounded transition-colors duration-base ease-out-cubic',
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

  // 查询解析完成后同步本地表单状态
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
          <BadgePill
            variant="neutral"
            data-testid="retention-default-badge"
          >
            default
          </BadgePill>
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
// =============== v0.23c — Auto-promote margin 面板 ===============
// =================================================================

/** 允许用户配置 auto-promote-if-better 操作的 Brier margin
 *  的卡片。
 *
 *  该 margin 存储在 UI prefs store（zustand + localStorage）中。
 *  它是一个简单浮点数；无需 IPC。范围：0.001（非常激进）到
 *  1.0（实际上禁用）。默认 0.005。 */
function AutoPromoteCard() {
  const { t } = useT();
  const margin = usePrefsStore((s) => s.autoPromoteBrierMargin);
  const afterTrain = usePrefsStore((s) => s.autoPromoteAfterTrain);
  const notifyOnAutoPromote = usePrefsStore((s) => s.autoPromoteNotify);
  // v0.42e-2 — 「skipped」分支（即 candidate 不优于
  // active model）的可选 OS 通知。大多数用户不需要
  // 此通知 —— 这是常见情况 —— 因此默认关闭。
  const notifyOnAutoPromoteSkipped = usePrefsStore(
    (s) => s.autoPromoteSkippedNotify,
  );
  const setPref = usePrefsStore((s) => s.setPref);
  const [value, setValue] = useState<number>(margin);
  const [saved, setSaved] = useState(false);

  // 当持久化的 margin 改变时同步本地表单状态
  // （例如挂载时或重置后）。使用函数形式初始化
  // 的 useState 是有意的：它仅在首次渲染时运行，
  // 而非每次状态变化。
  useState(() => {
    setValue(margin);
  });

  // v0.28c — 当用户切换「Auto-run after train」时，
  // 立即将新值推送到 Rust（二值切换不需要 Save
  // 按钮）。Rust 端在 `train_job` 内部读取
  // `auto_promote.enabled` 以决定是否启动 worker。
  const onAfterTrainToggle = (next: boolean) => {
    setPref('autoPromoteAfterTrain', next);
    setAutoPromoteConfig({ enabled: next }).catch(() => {
      // Sidecar 并不总是可用；L1 store 是单一数据源，
      // Rust 会在下次 Settings 挂载时重新读取。
    });
  };

  // v0.39b — 当用户切换桌面通知标志时，仅更新
  // prefs store（无需推送到 Rust —— 通知仅限 L1，
  // Rust 不知道它）。
  const onNotifyToggle = (next: boolean) => {
    setPref('autoPromoteNotify', next);
  };

  // v0.42e-2 — 「skipped」分支的可选 OS 通知。
  // 默认关闭。
  const onNotifySkippedToggle = (next: boolean) => {
    setPref('autoPromoteSkippedNotify', next);
  };

  const onSave = () => {
    if (!Number.isFinite(value) || value < 0) {
      setValue(margin);
      return;
    }
    setPref('autoPromoteBrierMargin', value);
    // v0.28c — 同时将新的 margin 推送到 Rust，以便
    // 下次 train 的 auto-promote worker 使用它。
    setAutoPromoteConfig({ brier_margin: value }).catch(() => {
      // 同上：best-effort 推送。
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
        {/* v0.39b — 桌面通知切换。位于「Auto-run after train」
            之下，以便用户同时看到相关选项。 */}
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
        {/* v0.42e-2 — 「skipped」分支的可选 OS 通知。
            默认关闭。位于主通知开关之下，使两者
            在视觉上分组。 */}
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

/** v0.36b — 备份与恢复卡片。允许用户将 UI 偏好导出为
 *  JSON 文件，并从 JSON 文件导入。用于：
 *   - 与其他用户分享首选配置
 *   - 重装前的备份
 *   - 在多台机器间复制相同配置
 *
 *  导出触发浏览器下载。导入打开文件选择器。导入时
 *  偏好会被验证；成功时更新 prefs store 并显示 toast；
 *  失败时显示带错误信息的 error toast。
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
    // 快照当前偏好（不包括 setPref/reset 函数）。
    // 下载工具会序列化其余字段。
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
    // 触发隐藏的文件输入
    fileInputRef.current?.click();
  };

  const onFileSelected = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setImporting(true);
    try {
      const text = await readFileAsText(file);
      const newPrefs = parsePrefsFromString(text);
      // 应用每个偏好。我们对每个字段使用 setPref，
      // 以便 store 的 zustand middleware 将它们
      // 持久化到 localStorage。
      for (const k of Object.keys(newPrefs) as (keyof UiPrefs)[]) {
        prefs.setPref(k, newPrefs[k]);
      }
      // v0.36b — 同时将 auto-promote 配置推送到
      // Rust，以便下次 train 的 auto-promote worker
      // 使用导入的值。
      setAutoPromoteConfig({
        enabled: newPrefs.autoPromoteAfterTrain,
        brier_margin: newPrefs.autoPromoteBrierMargin,
      }).catch(() => {
        // Best-effort；L1 store 是单一数据源，
        // Rust 在下次挂载时重新读取。
      });
      toast.success(t('prefs.backup.imported'));
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      toast.error(t('prefs.backup.import_failed'), message);
    } finally {
      setImporting(false);
      // 清空 input 以便用户重新选择同一文件
      // （否则 change 事件不会再次触发）
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
        {/* 隐藏的文件 input；点击 Import 按钮
            会在此 input 上触发一次 click。 */}
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
// ============== v0.42c — Telemetry opt-in 卡片 ===================
// =================================================================

/** v0.42c — opt-in 生命周期遥测。
 *
 *  开启时，每个 Rust 生命周期事件（train started /
 *  completed / failed、promote completed、scheduler tick 等）
 *  都会向 stderr 写入一行 NDJSON。可通过
 *  `polyrocket 2> telemetry.log` 捕获。
 *
 *  默认关闭。用户可在 Settings 中切换，更改会立即生效
 *  （L1 通过 `setTelemetryEnabled` 将新值推送到 Rust）。
 *
 *  无 PII，无 model weights，无 secrets。事件是粗粒度的
 *  生命周期标记（job_id、loop_name、latency_ms）——
 *  完整事件 schema 见 `src-tauri/src/infra/telemetry.rs`。
 */
function TelemetryCard() {
  const { t } = useT();
  const enabled = usePrefsStore((s) => s.telemetryEnabled);
  const setPref = usePrefsStore((s) => s.setPref);
  const [pushed, setPushed] = useState(false);

  // v0.42c — 挂载时询问 Rust 当前生效的状态。这用于
  // 处理启动时设置了环境变量 POLYROCKET_TELEMETRY=1
  // 的情况（L1 store 启动为 false；Rust 启动为 true；
  // 切换控件应反映这一情况）。
  useEffect(() => {
    getTelemetryEnabled()
      .then((v) => {
        if (v !== usePrefsStore.getState().telemetryEnabled) {
          setPref('telemetryEnabled', v);
        }
      })
      .catch(() => {
        // 启动时 sidecar 可能宕机；默认值
        // 保持为 L1 store 的值。
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
        // 尽力而为；用户可重新切换。
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
        {/* v0.42c — capture 提示。一旦开启遥测，用户必须
            知道如何实际捕获流。文案位于 i18n catalog
            （telemetry.capture_hint）中。 */}
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

        {/* v0.49a — 磁盘上的会话文件。v0.49a 将事件
            持久化到每个会话的 JSONL 文件（每次进程
            启动一个文件）。用户可以查看清单并清除
            旧文件。 */}
        <TelemetryLogList enabled={enabled} />
      </div>
    </Card>
  );
}

/** v0.49a — 子组件：列出磁盘上的遥测会话文件并允许
 *  用户清除旧文件。
 *
 *  我们始终渲染该 section，即使遥测关闭，因为文件可能
 *  仍存在于上一次会话（log 目录在首次 emit 时创建；
 *  此前列表为空）。
 *
 *  v0.62c — 导出以便测试可独立渲染。默认 `enabled=true`。
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

  // v0.62c — 列表顶部的总大小 + 数量摘要。帮助用户
  // 估算遥测占用了多少磁盘空间，无需滚动查看完整列表。
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
      {/* v0.62c — 磁盘用量摘要。用户可一眼看出遥测
          在磁盘上占用了多少空间，无需滚动。 */}
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
// ============== v0.44c — Paper trading mode 卡片 =================
// =================================================================

/** v0.44c — paper trading mode 切换。
 *
 *  开启时，mirror executor 选择的订单会写入 `paper_fills`
 *  表而非 `bets`，且跳过 CLOB sign_order 步骤。决策逻辑
 *  （sizing、exposure caps、frequency）不变。用户可
 *  在不冒真实资金风险的情况下验证其配置。
 *
 *  默认关闭。L1 在 Settings 挂载时以及每次切换时通过
 *  `setMirrorPaperMode` 将值推送到 Rust。
 */

// ================================================================
// ============ v0.49b — Active model 摘要卡片 =====================
// ================================================================

/** v0.51a — CLOB feed 状态卡片。展示真实订单簿 feed
 *  是否已配置（POLYROCKET_CLOB_API_KEY + SECRET +
 *  PASSPHRASE 环境变量）以及本地缓存了多少快照。
 *  v0.51+ 中将上线实时 WebSocket 监听器；v0.51a 仅
 *  落地 schema + IPC + 此卡片。
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


/** v0.49c — 调度器自检。读取 `infra::scheduler::self_test`
 *  中的进程全局原子计数器，并为每个 loop 渲染一行
 *  绿/红点。Refresh 按钮强制重新拉取。尚未 tick 的
 *  loop（仍处于初始 stagger 休眠）以黄色渲染并显示
 *  「starting...」。
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
    // 每 30s 自动轮询，使圆点无需手动点击即可更新。
    // 廉价（原子读）。
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

/** v0.49b — 只读卡片，展示 Rust 端视为「当前」的 model。
 *
 *  数据来自 `getActiveModel` IPC，该 IPC 读取
 *  `<sidecar model dir>/active.json`。这与 v0.48a
 *  降级检测器看到的视图一致（v0.49b 重构两者共享
 *  该 helper）。
 *
 *  首次 promote 之前的状态：`null` 与「no model promoted
 *  yet」提示一起显示。
 *
 *  该卡片不是控件 —— 此处没有 promote 操作。
 *  Promote 在 Model Lab 页面进行；该卡片仅为快速摘要。
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

  // v0.44c — 挂载时询问 Rust 当前生效的状态（以防
  // 环境变量在启动时设置了它）。
  useEffect(() => {
    getMirrorPaperMode()
      .then((v) => {
        if (v !== usePrefsStore.getState().mirrorPaperMode) {
          setPref('mirrorPaperMode', v);
        }
      })
      .catch(() => {
        // 尽力而为
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
// ============== v0.48b — Model 降级告警卡片 =====================
// =================================================================

/** v0.48b — model 降级告警的可选 OS 通知。第 7 个调度
 *  loop 每小时运行一次，计算 FALLBACK model 在最近
 *  已结算 market 上的实时 Brier，并在 live > train +
 *  阈值时发出 `alert=true` 的遥测事件。当此偏好开启时，
 *  L1 在这些事件上触发真正的 OS 通知。默认开启。
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

// v0.54b — 存储迁移工具。展示 getStorageInfo 中的
// 「Restart required」状态，并为用户提供一键「Copy
// existing data to new path」按钮。迁移完成后，下次
// 启动即可在新位置找到数据（不会出现空白 DB 的意外）。
// v0.79c — bankroll 分配配置卡片。只读摘要，
// 调用 `get_bankroll_config` 并显示当前值。
// 实际编辑在 /bankroll（拥有滑块 UI + apply-allocation
// 流程）进行。该卡片是「我的配置在哪里」的指引。
export function BankrollConfigCard() {
  const { t } = useT();
  const walletsQuery = useQuery({
    queryKey: ['wallets'],
    queryFn: () => listWallets(),
    staleTime: 30_000,
  });
  const firstWalletId = (walletsQuery.data ?? [])[0]?.id ?? 'default';
  const configQuery = useQuery({
    queryKey: ['bankroll-config', firstWalletId],
    queryFn: () => getBankrollConfig(firstWalletId),
    enabled: !!firstWalletId && firstWalletId !== 'default',
  });
  const navigate = useNavigate();
  return (
    <Card
      title={t('settings.bankroll.title', { default: 'Bankroll Allocation' })}
      description={t('settings.bankroll.desc', {
        default:
          'AI-driven Kelly-based bet allocation. Edit in /bankroll.',
      })}
    >
      <div className="space-y-3">
        {configQuery.isLoading ? (
          <Skeleton className="h-20" />
        ) : configQuery.data ? (
          <div className="grid grid-cols-2 gap-2 text-[12px]" data-testid="bankroll-config-card">
            <ConfigItem label="Kelly multiplier" value={fmtPct(configQuery.data.kelly_multiplier)} />
            <ConfigItem label="Max per signal" value={fmtPct(configQuery.data.max_per_signal_pct)} />
            <ConfigItem label="Reserve" value={fmtPct(configQuery.data.reserve_pct)} />
            <ConfigItem label="Min |edge|" value={fmtPct(configQuery.data.min_edge_pct)} />
            <ConfigItem
              label="Max total exposure"
              value={fmtPct(configQuery.data.max_total_exposure_pct)}
            />
            <ConfigItem
              label="Min confidence"
              value={fmtPct(configQuery.data.min_confidence)}
            />
          </div>
        ) : (
          <div className="text-[12px] text-muted">
            Using default config. Configure in /bankroll to persist.
          </div>
        )}
        <Button
          size="sm"
          variant="secondary"
          onClick={() => navigate('/bankroll')}
          data-testid="bankroll-config-go"
        >
          Open /bankroll
        </Button>
      </div>
    </Card>
  );
}

function ConfigItem({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between px-2 py-1 rounded bg-surface-2">
      <span className="text-muted">{label}</span>
      <span className="font-mono text-fg">{value}</span>
    </div>
  );
}

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

  // 该卡片仅在用户选择了自定义路径且当前会话
  // 仍在默认路径上（需要重启）时才有意义。
  // 否则没有需要迁移的内容。
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

// v0.55 — model 可解释性。让用户选择一个样本
// （price + age）并查看对 active model 预测的
// 逐特征贡献。展示 3 特征 logistic model 的
// 类 SHAP 分解。
// v0.59 — ExplainabilityCard 导出供专门的测试
// （src/routes/ExplainabilityCard.test.tsx）使用。
// 仍像之前一样在 Settings 内部使用。
export function ExplainabilityCard() {
  const { t } = useT();
  const active = useQuery({
    queryKey: ['active-model'],
    queryFn: () => getActiveModel(),
    staleTime: 30_000,
  });
  const [price, setPrice] = useState(0.5);
  const [age, setAge] = useState(24);
  // v0.59 — 在 SHAP 和 v0.55 精确分解之间切换。
  // SHAP 是默认值，因为它满足效率公理，
  // 也是来自 shap-library / interpret-ml 的用户所期待的。
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
  // 根据切换选择当前结果。
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
        {/* v0.59 — SHAP vs 导数 切换。 */}
        <div className="flex items-center gap-1 p-0.5 bg-surface-2 rounded-md w-fit">
          <button
            type="button"
            onClick={() => setUseShap(true)}
            data-testid="explain-method-shap"
            className={cn(
              'h-6 px-3 rounded text-[11px] font-medium transition-colors duration-base ease-out-cubic',
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
              'h-6 px-3 rounded text-[11px] font-medium transition-colors duration-base ease-out-cubic',
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
            {/* v0.59 — 使用 SHAP 时，显示 baseline +
               target + efficiency diff。导数视图
               仅显示 prediction。 */}
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
                // SHAP features 使用 `shap_value`/
                // `abs_shap`；导数 features 使用
                // `contribution`/`abs_contribution`。
                // 归一化为单个 (value, abs) 对以便渲染。
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
                      {/* 居中条：正值向右，负值向左。 */}
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
            {/* v0.59 — 展示 SHAP efficiency 残差，
                让用户可以直观检查计算是否正确。 */}
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

// v0.56 — 网络代理 / Tor 配置。
// 允许用户将所有出站 HTTP（LLM 客户端、Polymarket
// CLOB、sidecar HTTP 等）通过代理路由。两种支持的
// scheme 是 http://（HTTP CONNECT）和 socks5://
// （例如 127.0.0.1:9050 上的 Tor SOCKS5）。更改需
// 重启生效（共享的 reqwest::Client 在启动时构建）。
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
  // IPC 返回时同步表单。
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
