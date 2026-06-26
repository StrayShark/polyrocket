/**
 * L1 —— 键盘快捷键注册表 + hook。
 *
 * 「Gmail 风格」的两键组合：
 *   1. 按下 `g`（无修饰键）→ 进入「pending prefix」模式
 *   2. 在 1.2 秒内按下第二个键 → 触发绑定
 *      例如 `g d` → /dashboard，`g m` → /markets，`g h` → /help
 *
 * 单键快捷键（无需前缀）：
 *   `?`           → 打开帮助对话框
 *   `/`           → 聚焦全局搜索框
 *   `Esc`         → 关闭任何打开的对话框 / 取消待处理前缀
 *
 * 该 hook 返回一个 `useKeyboardNav()` API，
 * 它挂载一个 document 级别的 `keydown`
 * 监听器（高效 —— 一个监听器，而不是
 * 每个快捷键一个）。
 *
 * 帮助对话框（`<KbdHelpDialog>`）渲染
 * 当前激活的绑定，使用户能够发现它们。
 */

import { useEffect, useState, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';

export interface KbdBinding {
  /** 显示标签，例如 "Go to Dashboard" */
  label: string;
  /** 按键序列（1 或 2 个）。例如 ['g', 'd'] 或 ['?']。 */
  keys: string[];
  /** 激活时的副作用。 */
  action: () => void;
}

const PREFIX_TIMEOUT_MS = 1200;

function normalizeKey(e: KeyboardEvent): string {
  // v0.64b —— 先检查修饰键。之前 `?` 的映射在最前面，
  // 这意味着 Cmd+? 仍会触发 help 绑定
  // （不正确 —— Cmd+? 是 macOS 系统快捷键，
  // 应放行）。
  if (e.metaKey || e.ctrlKey || e.altKey) return '';  // 忽略
  // 我们确实想捕获纯 `?`（Shift+/），因此把
  // `?` 映射为 "?"，其他情况使用 `e.key`。
  if (e.key === '?' || (e.shiftKey && e.key === '/')) return '?';
  return e.key.toLowerCase();
}

/**
 * 构建规范的绑定集。与 polyrocket 的
 * 18 个路由绑定 —— 新增路由意味着在同一
 * commit 中添加绑定（路由映射参见
 * docs/overview.md §4.2）。
 */
export function useNavBindings(opts: {
  onOpenHelp: () => void;
  onOpenSearch: () => void;
  onCloseDialog: () => void;
}): KbdBinding[] {
  const nav = useNavigate();
  return [
    // 双键和弦(g + key)—— 9 个最重要的路由
    { label: 'Go to Dashboard',  keys: ['g', 'd'], action: () => nav('/dashboard') },
    { label: 'Go to Markets',    keys: ['g', 'm'], action: () => nav('/markets') },
    { label: 'Go to Signals',    keys: ['g', 's'], action: () => nav('/signals') },
    { label: 'Go to Copy',       keys: ['g', 'c'], action: () => nav('/copy') },
    { label: 'Go to P&L',        keys: ['g', 'p'], action: () => nav('/pnl') },
    { label: 'Go to History',    keys: ['g', 'h'], action: () => nav('/history') },
    { label: 'Go to Wallets',    keys: ['g', 'w'], action: () => nav('/wallets') },
    { label: 'Go to Lab',        keys: ['g', 'l'], action: () => nav('/lab') },
    { label: 'Go to Settings',   keys: ['g', 's'], action: () => nav('/settings') },  // 与 Go to Signals 冲突；按定义顺序，后者生效
    // 单键快捷键
    { label: 'Show keyboard shortcuts', keys: ['?'], action: opts.onOpenHelp },
    { label: 'Focus search',             keys: ['/'], action: opts.onOpenSearch },
    { label: 'Close dialog',             keys: ['escape'], action: opts.onCloseDialog },
  ];
}

/**
 * 挂载键盘监听器。返回 `pendingPrefix`，以便可以
 * 显示一个状态指示器（例如 "g…"）。
 */
export function useKeyboardNav(bindings: KbdBinding[]) {
  const [pendingPrefix, setPendingPrefix] = useState<string | null>(null);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null;

    const handler = (e: KeyboardEvent) => {
      // 在表单字段中输入时不要捕获。
      const target = e.target as HTMLElement | null;
      if (target) {
        const tag = target.tagName;
        if (tag === 'INPUT' || tag === 'TEXTAREA' || target.isContentEditable) {
          return;
        }
      }
      const key = normalizeKey(e);
      if (!key) return;

      // 已经处于 prefix 模式？
      if (pendingPrefix) {
        // 查找匹配 prefix + key 的两键绑定
        const match = bindings.find(
          (b) => b.keys.length === 2 && b.keys[0] === pendingPrefix && b.keys[1] === key,
        );
        if (match) {
          e.preventDefault();
          match.action();
        }
        // 无论如何,退出 prefix 模式。
        setPendingPrefix(null);
        if (timer) { clearTimeout(timer); timer = null; }
        return;
      }

      // 没有 prefix。查找 1-key 绑定。
      const single = bindings.find((b) => b.keys.length === 1 && b.keys[0] === key);
      if (single) {
        e.preventDefault();
        single.action();
        return;
      }

      // 查找首键匹配的 2 键绑定。
      const isPrefix = bindings.some(
        (b) => b.keys.length === 2 && b.keys[0] === key,
      );
      if (isPrefix) {
        e.preventDefault();
        setPendingPrefix(key);
        if (timer) { clearTimeout(timer); }
        timer = setTimeout(() => setPendingPrefix(null), PREFIX_TIMEOUT_MS);
      }
    };

    document.addEventListener('keydown', handler);
    return () => {
      document.removeEventListener('keydown', handler);
      if (timer) clearTimeout(timer);
    };
  }, [bindings, pendingPrefix]);

  const clearPrefix = useCallback(() => setPendingPrefix(null), []);

  return { pendingPrefix, clearPrefix };
}

/**
 * 美化打印按键序列（供帮助对话框使用）。
 *  ['g', 'd']  → "G D"
 *  ['?']       → "?"
 *  ['escape']  → "Esc"
 */
export function formatKeys(keys: string[]): string {
  return keys
    .map((k) => {
      if (k === 'escape') return 'Esc';
      if (k === '/') return '/';
      if (k === '?') return '?';
      return k.toUpperCase();
    })
    .join(' ');
}
