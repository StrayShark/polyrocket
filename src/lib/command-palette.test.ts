import { describe, expect, it } from 'vitest';
import { buildPaletteCommands, filterCommands, scoreCommand, isPaletteTrigger, type PaletteCommand } from './command-palette';

const noop = () => {};
const sample = (): PaletteCommand[] => buildPaletteCommands({
  onNavigate: noop,
  onOpenHelp: noop,
  onSyncMarkets: noop,
  onOpenSettings: noop,
  onResetDemoData: noop,
});

describe('buildPaletteCommands', () => {
  it('returns the canonical set', () => {
    const cmds = sample();
    expect(cmds.length).toBeGreaterThanOrEqual(10);
    // 每个命令都有非空 label
    for (const c of cmds) {
      expect(c.label.length).toBeGreaterThan(0);
      expect(c.id.length).toBeGreaterThan(0);
      expect(c.category.length).toBeGreaterThan(0);
    }
  });

  it('has both Navigate and Actions categories', () => {
    const cmds = sample();
    const cats = new Set(cmds.map((c) => c.category));
    expect(cats.has('Navigate')).toBe(true);
    expect(cats.has('Actions')).toBe(true);
  });
});

describe('scoreCommand', () => {
  it('exact label match scores highest', () => {
    const cmd: PaletteCommand = { id: 'a', label: 'Sync markets', category: 'Actions', action: noop };
    expect(scoreCommand(cmd, 'sync markets')).toBe(100);
  });

  it('prefix match scores second', () => {
    const cmd: PaletteCommand = { id: 'a', label: 'Sync markets', category: 'Actions', action: noop };
    expect(scoreCommand(cmd, 'sync')).toBe(80);
  });

  it('substring match scores third', () => {
    const cmd: PaletteCommand = { id: 'a', label: 'Sync markets', category: 'Actions', action: noop };
    expect(scoreCommand(cmd, 'rket')).toBe(60);
  });

  it('alias exact scores above label substring', () => {
    // 注意：label 本身不以 query 开头，所以
      // alias 路径才是实际生效的路径。
    const cmd: PaletteCommand = {
      id: 'a', label: 'Refresh signals', aliases: ['recompute'], category: 'Actions', action: noop,
    };
    expect(scoreCommand(cmd, 'recompute')).toBe(70);
  });

  it('returns 0 for no match', () => {
    const cmd: PaletteCommand = { id: 'a', label: 'Sync markets', category: 'Actions', action: noop };
    expect(scoreCommand(cmd, 'xyz123')).toBe(0);
  });

  it('returns stable score for empty query', () => {
    const cmd: PaletteCommand = { id: 'a', label: 'X', category: 'C', action: noop };
    expect(scoreCommand(cmd, '')).toBe(50);
  });
});

describe('filterCommands', () => {
  const cmds = sample();

  it('returns stable order for empty query (limited to 10)', () => {
    const out = filterCommands(cmds, '');
    expect(out.length).toBeLessThanOrEqual(10);
    // 前 3 个应按定义顺序为导航项
    expect(out[0].id).toBe('nav.dashboard');
  });

  it('ranks "sync" → Sync markets first', () => {
    const out = filterCommands(cmds, 'sync');
    expect(out[0].id).toBe('act.sync');
  });

  it('ranks alias "arbitrage" → Arb Board first', () => {
    // v0.126 —— recompute / purge 动作已移除 (v0.123 football-only),
    // 改用「arbitrage」alias 测试 ranking 逻辑。
    const out = filterCommands(cmds, 'arbitrage');
    expect(out[0].id).toBe('nav.arb-board');
  });

  it('respects the limit', () => {
    const out = filterCommands(cmds, 'a', 3);
    expect(out.length).toBeLessThanOrEqual(3);
  });

  it('returns empty array for unmatched query', () => {
    const out = filterCommands(cmds, 'zzzzzzzz');
    expect(out).toEqual([]);
  });
});

describe('isPaletteTrigger', () => {
  it('matches Cmd+K on mac', () => {
    const e = { metaKey: true, ctrlKey: false, key: 'k' } as unknown as KeyboardEvent;
    expect(isPaletteTrigger(e)).toBe(true);
  });

  it('matches Ctrl+K elsewhere', () => {
    const e = { metaKey: false, ctrlKey: true, key: 'K' } as unknown as KeyboardEvent;
    expect(isPaletteTrigger(e)).toBe(true);
  });

  it('rejects plain K', () => {
    const e = { metaKey: false, ctrlKey: false, key: 'k' } as unknown as KeyboardEvent;
    expect(isPaletteTrigger(e)).toBe(false);
  });

  it('rejects Cmd+other', () => {
    const e = { metaKey: true, ctrlKey: false, key: 'd' } as unknown as KeyboardEvent;
    expect(isPaletteTrigger(e)).toBe(false);
  });
});
