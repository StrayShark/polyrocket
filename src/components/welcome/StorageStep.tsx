// v0.53b — StorageStep (Step 2 of 6).
//
// Pick default or custom path. v0.53a wires the
// 3 IPCs (getStorageInfo, setStoragePath,
// resetStoragePath). The custom path uses a text
// input (v0.53 doesn't introduce tauri-plugin-dialog;
// v0.54+ adds the Browse... button).

import { useEffect, useState, useCallback } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getStorageInfo,
  setStoragePath,
  resetStoragePath,
  type StorageInfo,
} from '@/ipc';
import { useWelcomeStore } from '@/stores/welcome-store';
import { useT } from '@/lib/i18n';
import { HardDrive, RotateCcw, CheckCircle2, AlertTriangle } from 'lucide-react';
import { cn } from '@/lib/cn';
import { toast } from '@/stores/toast-store';

type Mode = 'default' | 'custom';

export function StorageStep({
  welcome,
}: {
  welcome: ReturnType<typeof useWelcomeStore.getState>;
}) {
  const { t } = useT();
  const qc = useQueryClient();
  const info = useQuery({
    queryKey: ['storage-info'],
    queryFn: () => getStorageInfo(),
    staleTime: 0,
  });
  const [mode, setMode] = useState<Mode>('default');
  const [customPath, setCustomPath] = useState('');
  const [submitting, setSubmitting] = useState(false);

  // When the IPC returns the actual state, sync
  // the form (default vs custom) so the user sees
  // what's currently in effect.
  useEffect(() => {
    if (info.data?.isCustom) {
      setMode('custom');
      setCustomPath(info.data.currentPath);
    } else {
      setMode('default');
    }
  }, [info.data?.isCustom, info.data?.currentPath]);

  const onApply = useCallback(async () => {
    setSubmitting(true);
    try {
      if (mode === 'default') {
        await resetStoragePath();
        welcome.setConfigured('storagePath', true);
        toast.success(t('welcome.storage_reset_ok'));
      } else {
        if (!customPath.trim()) {
          toast.error(t('welcome.storage_path_required'));
          return;
        }
        await setStoragePath(customPath.trim());
        welcome.setConfigured('storagePath', true);
        toast.success(
          t('welcome.storage_set_ok'),
          t('welcome.storage_restart_hint'),
        );
      }
      qc.invalidateQueries({ queryKey: ['storage-info'] });
    } catch (e) {
      toast.error(String(e));
    } finally {
      setSubmitting(false);
    }
  }, [mode, customPath, qc, welcome, t]);

  return (
    <div className="space-y-4 py-2">
      <div>
        <h2 className="text-[18px] font-semibold text-fg">
          {t('welcome.storage_title')}
        </h2>
        <p className="text-[12px] text-muted mt-1">
          {t('welcome.storage_desc')}
        </p>
      </div>

      <div className="grid grid-cols-1 gap-3">
        <ModeCard
          active={mode === 'default'}
          onClick={() => setMode('default')}
          testid="welcome-storage-mode-default"
          title={t('welcome.storage_default_title')}
          body={info.data?.defaultPath ?? '...'}
        />
        <ModeCard
          active={mode === 'custom'}
          onClick={() => setMode('custom')}
          testid="welcome-storage-mode-custom"
          title={t('welcome.storage_custom_title')}
        >
          {mode === 'custom' && (
            <div className="space-y-2">
              <input
                type="text"
                value={customPath}
                onChange={(e) => setCustomPath(e.target.value)}
                placeholder={t('welcome.storage_path_placeholder')}
                data-testid="welcome-storage-path-input"
                className="w-full h-8 px-2.5 rounded-md text-[12px] bg-surface text-fg border border-border focus:outline-none focus:ring-2 focus:ring-accent font-mono"
              />
              {info.data && (
                <StorageStatus info={info.data} />
              )}
            </div>
          )}
        </ModeCard>
      </div>

      {/* "What lives here" callout. Mirrors the
          spec: db + logs/ + logs/telemetry/. NO
          secrets — those live in OS keyring only. */}
      <div className="rounded-md border border-border bg-surface-2 p-3 text-[11px] text-muted space-y-1">
        <div className="text-fg text-[12px] mb-1">
          {t('welcome.storage_contents_title')}
        </div>
        <div>• polyrocket.db ({t('welcome.storage_contents_db')})</div>
        <div>• logs/ ({t('welcome.storage_contents_logs')})</div>
        <div>• logs/telemetry/ ({t('welcome.storage_contents_telemetry')})</div>
        <div className="text-bull mt-2">
          {t('welcome.storage_contents_no_secrets')}
        </div>
      </div>

      <div className="flex items-center justify-end gap-2 pt-2">
        {info.data?.isCustom && (
          <button
            type="button"
            onClick={onApply}
            disabled={submitting}
            data-testid="welcome-storage-reset"
            className="text-[12px] text-muted hover:text-fg disabled:opacity-50"
          >
            <RotateCcw className="w-3 h-3 inline mr-1" />
            {t('welcome.storage_use_default')}
          </button>
        )}
        <button
          type="button"
          onClick={onApply}
          disabled={submitting}
          data-testid="welcome-storage-apply"
          className="h-8 px-4 rounded-md text-[12px] font-medium bg-accent text-bg hover:bg-accent/90 disabled:opacity-50"
        >
          {t('welcome.storage_apply')}
        </button>
      </div>
    </div>
  );
}

function ModeCard({
  active,
  onClick,
  title,
  body,
  testid,
  children,
}: {
  active: boolean;
  onClick: () => void;
  title: string;
  body?: string;
  testid: string;
  children?: React.ReactNode;
}) {
  return (
    <div
      onClick={onClick}
      data-testid={testid}
      className={cn(
        'rounded-md border p-3 cursor-pointer transition-colors',
        active
          ? 'bg-accent/10 border-accent/40'
          : 'bg-surface-2 border-border hover:bg-surface-hover',
      )}
    >
      <div className="flex items-center gap-2">
        <HardDrive
          className={cn(
            'w-4 h-4',
            active ? 'text-accent' : 'text-muted',
          )}
        />
        <span className="text-[13px] font-medium text-fg">{title}</span>
        {active && (
          <CheckCircle2 className="w-3.5 h-3.5 text-accent ml-auto" />
        )}
      </div>
      {body && (
        <div className="text-[10px] text-muted mt-1 font-mono break-all">
          {body}
        </div>
      )}
      {children && <div className="mt-2">{children}</div>}
    </div>
  );
}

function StorageStatus({ info }: { info: StorageInfo }) {
  const { t } = useT();
  if (!info.writable) {
    return (
      <div
        data-testid="welcome-storage-status"
        className="text-[10px] text-bear flex items-center gap-1"
      >
        <AlertTriangle className="w-3 h-3" />
        {t('welcome.storage_not_writable')}
      </div>
    );
  }
  return (
    <div
      data-testid="welcome-storage-status"
      className="text-[10px] text-bull flex items-center gap-1"
    >
      <CheckCircle2 className="w-3 h-3" />
      {t('welcome.storage_writable')}
    </div>
  );
}
