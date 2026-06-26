/**
 * 展示可用键盘快捷键的帮助对话框。
 * 由 AppShell 全局挂载,按 `?` 键打开。
 */

import { useState } from 'react';
import { Modal } from './Modal';
import { formatKeys, type KbdBinding } from '@/lib/keyboard-nav';
import { useT } from '@/lib/i18n';

interface KbdHelpDialogProps {
  bindings: KbdBinding[];
  open: boolean;
  onClose: () => void;
}

export function KbdHelpDialog({ bindings, open, onClose }: KbdHelpDialogProps) {
  const { t } = useT();
  // 按按键数量(单键 vs 双键)分组
  const singles = bindings.filter((b) => b.keys.length === 1);
  const chords = bindings.filter((b) => b.keys.length === 2);

  return (
    <Modal open={open} onClose={onClose} title={t('kbd.title')}>
      <div className="space-y-4">
        <Section title={t('palette.category.navigate')}>
          {chords.map((b, i) => (
            <Row key={i} label={b.label} keys={formatKeys(b.keys)} />
          ))}
        </Section>
        <Section title={t('palette.category.actions')}>
          {singles.map((b, i) => (
            <Row key={i} label={b.label} keys={formatKeys(b.keys)} />
          ))}
        </Section>
        <p className="text-[11px] text-muted pt-2 border-t border-border">
          {t('kbd.tip')}
        </p>
      </div>
    </Modal>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <h3 className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase mb-2">
        {title}
      </h3>
      <div className="space-y-1.5">{children}</div>
    </div>
  );
}

function Row({ label, keys }: { label: string; keys: string }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <span className="text-[12px] text-fg">{label}</span>
      <span className="flex items-center gap-1">
        {keys.split(' ').map((k, i) => (
          <Kbd key={i}>{k}</Kbd>
        ))}
      </span>
    </div>
  );
}

function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <kbd className="inline-flex items-center justify-center min-w-[20px] h-5 px-1.5 text-[10px] font-mono bg-bg border border-border rounded text-muted">
      {children}
    </kbd>
  );
}

/** AppShell 用于管理自身对话框状态的 hook。 */
export function useKbdHelpDialog() {
  const [open, setOpen] = useState(false);
  return {
    open,
    openDialog: () => setOpen(true),
    closeDialog: () => setOpen(false),
  };
}
