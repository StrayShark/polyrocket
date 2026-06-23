/**
 * Command palette UI (v0.9c).
 *
 * Modal with a search input + a scrollable list of filtered commands.
 * Keyboard navigation:
 *   - `↑` / `↓` to move the highlight
 *   - `Enter` to execute
 *   - `Esc` to close
 *
 * Selecting a command runs its `action()` then closes the palette.
 */

import { useEffect, useMemo, useRef, useState } from 'react';
import { Modal } from './Modal';
import { Search } from 'lucide-react';
import { filterCommands, type PaletteCommand } from '@/lib/command-palette';
import { useT } from '@/lib/i18n';

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  commands: PaletteCommand[];
}

export function CommandPalette({ open, onClose, commands }: CommandPaletteProps) {
  const { t } = useT();
  const [query, setQuery] = useState('');
  const [highlight, setHighlight] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Reset state when opened
  useEffect(() => {
    if (open) {
      setQuery('');
      setHighlight(0);
      // Focus the input on next frame
      setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open]);

  const filtered = useMemo(() => filterCommands(commands, query, 10), [commands, query]);

  // Keep highlight in range
  useEffect(() => {
    if (highlight >= filtered.length) {
      setHighlight(Math.max(0, filtered.length - 1));
    }
  }, [filtered.length, highlight]);

  const run = (cmd: PaletteCommand) => {
    cmd.action();
    onClose();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setHighlight((h) => Math.min(filtered.length - 1, h + 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setHighlight((h) => Math.max(0, h - 1));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const cmd = filtered[highlight];
      if (cmd) run(cmd);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  };

  return (
    <Modal open={open} onClose={onClose} size="md">
      <div className="-m-4">  {/* Edge-to-edge inside modal */}
        <div className="flex items-center gap-2 px-3 py-2 border-b border-border">
          <Search className="w-4 h-4 text-muted shrink-0" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder={t('palette.placeholder')}
            className="flex-1 bg-transparent outline-none text-body-sm text-fg placeholder:text-muted"
            autoComplete="off"
            spellCheck={false}
            data-testid="palette-input"
          />
          <span className="text-[10px] font-mono text-muted">Esc</span>
        </div>
        <div
          ref={listRef}
          className="max-h-80 overflow-y-auto py-1"
          data-testid="palette-list"
        >
          {filtered.length === 0 ? (
            <div className="px-4 py-8 text-center text-[12px] text-muted">
              {t('palette.empty')}
            </div>
          ) : (
            filtered.map((cmd, i) => (
              <button
                key={cmd.id}
                onClick={() => run(cmd)}
                onMouseEnter={() => setHighlight(i)}
                className={`w-full flex items-center justify-between gap-2 px-3 py-1.5 text-left text-[12px] ${
                  i === highlight ? 'bg-surface-hover' : ''
                }`}
                data-testid={`palette-item-${cmd.id}`}
                data-highlighted={i === highlight}
              >
                <span className="flex items-center gap-2 min-w-0">
                  <span className="text-fg truncate">{cmd.label}</span>
                  <span className="text-xs text-muted font-semibold uppercase tracking-caption-uppercase shrink-0">
                    {cmd.category}
                  </span>
                </span>
                {cmd.shortcut && (
                  <span className="flex items-center gap-1 shrink-0">
                    {cmd.shortcut.map((k, j) => (
                      <kbd
                        key={j}
                        className="inline-flex items-center justify-center min-w-[18px] h-4 px-1 text-[9px] font-mono bg-bg border border-border rounded text-muted"
                      >
                        {k === 'escape' ? 'Esc' : k.toUpperCase()}
                      </kbd>
                    ))}
                  </span>
                )}
              </button>
            ))
          )}
        </div>
      </div>
    </Modal>
  );
}

/** Hook so AppShell can manage its own dialog state. */
export function useCommandPalette() {
  const [open, setOpen] = useState(false);
  return {
    open,
    openPalette: () => setOpen(true),
    closePalette: () => setOpen(false),
  };
}
