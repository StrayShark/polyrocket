// polyrocket —— ThemeSwitcher(v0.67c 密度)。
//
// 渲染为 segmented control 的三主题选择器
// (dark / light / matrix)。用于:
//   - 侧栏底部(AppShell)
//   - Settings 页面(用户级偏好)
//
// **为什么用 segmented 而非下拉**:项目只有 3 套主题。
// segmented 控件一眼就能看到全部选项 ——
// 1 次点击即可选中。下拉需要先点开再选,
// 多一次点击。在 3 个选项下,segmented 在速度上更优。
//
// **持久化**:当前主题存放在
// `useThemeStore`(zustand + persist)。重载应用时
// 恢复上次选择。DOM 的 `data-theme` 属性由
// `stores/theme-store.ts`(effect)设置。
//
// **CSS 变量**:主题颜色来自 `src/styles/*.css`
// 的 CSS 变量。每个主题有自己的变量定义。
// 切换器只翻 `data-theme` —— 实际颜色值
// 在 stylesheet 中。

import { Moon, Sun, Circle } from 'lucide-react';
import { useThemeStore, type Theme } from '@/stores/theme-store';
import { cn } from '@/lib/cn';

// 显示顺序。重要:Dark 在前(默认),Light 第二
// (最常用),Matrix 第三(小众/进阶用户)。
const ORDER: Theme[] = ['dark', 'light', 'matrix'];

// 每个主题对应的 Lucide icon。Moon = dark,Sun = light,Circle = matrix。
const ICONS: Record<Theme, typeof Moon> = {
  dark: Moon,
  light: Sun,
  matrix: Circle,
};

// 每个主题的人类可读 label。同时用于按钮文字
// 和 aria-label("Switch to {label} theme")。
const LABELS: Record<Theme, string> = {
  dark: 'Dark',
  light: 'Light',
  matrix: 'Matrix',
};

/** `<ThemeSwitcher>` 的 Props。 */
interface ThemeSwitcherProps {
  /**
   * 视觉样式。`segmented`(默认)是横向一排
   * 3 个按钮。`dropdown` 保留给未来 shadcn
   * DropdownMenu 使用 —— 当前返回 `null`。
   */
  variant?: 'segmented' | 'dropdown';
  /** 根容器的额外 Tailwind class。 */
  className?: string;
}

/**
 * 渲染 3 主题选择器。`segmented` 模式(默认)
 * 渲染 3 个按钮一排,当前激活的高亮。
 * `dropdown` 模式返回 `null`(留作未来
 * 紧凑变体的占位)。
 */
export function ThemeSwitcher({ variant = 'segmented', className }: ThemeSwitcherProps) {
  // v0.67c —— 通过两个 selector 从同一 store
  // 读取 theme 和 setter(zustand 模式)。
  // store 持久化到 localStorage;重新挂载时
  // 恢复上次选择。
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);

  if (variant === 'segmented') {
    return (
      <div
        className={cn(
          'inline-flex items-center gap-0.5 p-0.5 rounded-md border bg-surface-2',
          'border-border',
          className,
        )}
      >
        {ORDER.map((t) => {
          // 为每个主题解析 icon 和激活态
          const Icon = ICONS[t];
          const active = t === theme;
          return (
            <button
              key={t}
              type="button"
              onClick={() => setTheme(t)}
              aria-pressed={active}
              aria-label={`Switch to ${LABELS[t]} theme`}
              className={cn(
                // 布局:inline-flex icon + label,padding 2 / 1。
                // 激活:surface 背景 + fg 文字 + card 阴影。
                // 未激活:muted 文字,hover 过渡到 surface-hover。
                'inline-flex items-center gap-1 px-2 py-1 text-xs rounded transition-colors duration-base ease-out-cubic',
                active
                  ? 'bg-surface text-fg shadow-card'
                  : 'text-muted hover:text-fg hover:bg-surface-hover',
              )}
            >
              <Icon className="w-3 h-3" />
              <span>{LABELS[t]}</span>
            </button>
          );
        })}
      </div>
    );
  }

  // dropdown 占位 —— 后续用 shadcn DropdownMenu 填充
  return null;
}