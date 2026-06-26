// v0.53b — ThemeStep (Step 3 of 6)。
//
// 选择主题。L1 已经有 3 种主题 (dark /
// light / matrix)。Welcome 步骤与 Settings 上的
// Theme 卡片相同,但采用 welcome
// 布局渲染。

import { useEffect } from 'react';
import { useThemeStore, type Theme } from '@/stores/theme-store';
import { useT } from '@/lib/i18n';
import { CheckCircle2 } from 'lucide-react';
import { cn } from '@/lib/cn';

const themes: Array<{ id: Theme; name: string; desc: string }> = [
  { id: 'dark', name: 'Dark', desc: 'Cursor / VS Code Dark+' },
  { id: 'light', name: 'Light', desc: 'Clean and bright' },
  { id: 'matrix', name: 'Matrix', desc: 'Green-on-black hacker' },
];

export function ThemeStep() {
  const { t } = useT();
  const theme = useThemeStore((s) => s.theme);
  const setTheme = useThemeStore((s) => s.setTheme);

  // v0.53b —— 用户进入 theme 步骤时标记 theme 为已配置。
  // (实际主题可能已从顶栏设置过;此处只是
  // "我们走过这一步" 的标记。)
  useEffect(() => {
    // no-op 标记;welcome-store 更新由
    // Welcome.tsx 中的 Next 按钮处理(或用户
    // 选主题时的直接调用)。当前每步的
    // configured flag 是独立考虑。
  }, []);

  return (
    <div className="space-y-4 py-2">
      <div>
        <h2 className="text-title-md font-semibold text-fg">
          {t('welcome.theme_title')}
        </h2>
        <p className="text-[12px] text-muted mt-1">
          {t('welcome.theme_desc')}
        </p>
      </div>
      <div className="grid grid-cols-3 gap-3">
        {themes.map((th) => (
          <button
            key={th.id}
            type="button"
            onClick={() => setTheme(th.id)}
            data-testid={`welcome-theme-${th.id}`}
            className={cn(
              'rounded-md border p-4 text-left transition-colors duration-base ease-out-cubic',
              theme === th.id
                ? 'bg-accent/10 border-accent/40'
                : 'bg-surface-2 border-border hover:bg-surface-hover',
            )}
          >
            <div className="flex items-center gap-2 mb-2">
              <div
                className="w-4 h-4 rounded-full"
                style={{
                  background:
                    th.id === 'dark'
                      ? '#1E1E1E'
                      : th.id === 'light'
                        ? '#FFFFFF'
                        : '#10A37F',
                  border: '1px solid var(--border)',
                }}
              />
              <span className="text-body-sm font-medium text-fg">
                {th.name}
              </span>
              {theme === th.id && (
                <CheckCircle2 className="w-3.5 h-3.5 text-accent ml-auto" />
              )}
            </div>
            <div className="text-[11px] text-muted">{th.desc}</div>
          </button>
        ))}
      </div>
    </div>
  );
}
