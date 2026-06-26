// v0.53b — WelcomeStep (Step 1 of 6)。
//
// Hero + 语言选择器。挂载时无 IPC 副作用;
// 语言切换写入 locale-store + welcome-store。
//
// **Step 1 为什么无 IPC**: 用户只是首次
// 进入应用。我们还没有他们的语言偏好,
// 他们也可能中途退出。保持 step 1 轻量。
//
// **布局**: 垂直居中列,包含
//   1. Rocket 图标 (64×64) + 应用名
//   2. EN / 中文 语言选择器 (2 个按钮)
//   3. 三个 "价值主张" 标签 (本地优先 / 密钥 / LLM)

import { Rocket } from 'lucide-react';
import { useT } from '@/lib/i18n';

export function WelcomeStep({
  onLocale,
}: {
  onLocale: (l: string) => void;
}) {
  const { t, locale } = useT();
  return (
    <div className="space-y-6 py-4">
      <div className="text-center space-y-3">
        <div className="w-16 h-16 rounded-2xl bg-accent mx-auto grid place-items-center">
          <Rocket className="w-8 h-8 text-white" />
        </div>
        <div>
          <h1
            className="text-[24px] font-semibold text-fg"
            data-testid="welcome-step-welcome-title"
          >
            {t('welcome.hero_title')}
          </h1>
          <p className="text-body-sm text-muted mt-2 max-w-md mx-auto">
            {t('welcome.hero_tagline')}
          </p>
        </div>
      </div>

      {/* v0.53b — language picker. 用户可以
          随时从顶栏切换。默认取
          navigator.language 报告的语言;
          en 和 zh 都已打包。 */}
      <div className="max-w-md mx-auto">
        <div className="text-xs text-muted font-semibold mb-2 uppercase tracking-caption-uppercase">
          {t('welcome.locale_label')}
        </div>
        <div className="grid grid-cols-2 gap-2">
          <LocaleButton
            active={locale === 'en'}
            onClick={() => onLocale('en')}
            testid="welcome-locale-en"
            label="English"
            sub="EN"
          />
          <LocaleButton
            active={locale === 'zh'}
            onClick={() => onLocale('zh')}
            testid="welcome-locale-zh"
            label="中文"
            sub="ZH"
          />
        </div>
      </div>

      {/* v0.53b — value props. 3 个简短要点
          强化 "本地优先" / "自托管" /
          "OS keyring"。措辞与旧版
          M13 v2.0 引导一致。 */}
      <div className="grid grid-cols-3 gap-3 max-w-md mx-auto pt-2">
        <PropChip
          testid="welcome-step-welcome-vp-0"
          title={t('welcome.prop_local_title')}
          body={t('welcome.prop_local_body')}
        />
        <PropChip
          testid="welcome-step-welcome-vp-1"
          title={t('welcome.prop_keys_title')}
          body={t('welcome.prop_keys_body')}
        />
        <PropChip
          testid="welcome-step-welcome-vp-2"
          title={t('welcome.prop_llm_title')}
          body={t('welcome.prop_llm_body')}
        />
      </div>
    </div>
  );
}

/** Locale 按钮 — 激活的语言带有 accent 背景。 */
function LocaleButton({
  active,
  onClick,
  label,
  sub,
  testid,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
  sub: string;
  testid: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      data-testid={testid}
      className={
        'h-12 rounded-md text-body-sm font-medium border transition-colors duration-base ease-out-cubic ' +
        (active
          ? 'bg-accent/10 border-accent/40 text-fg'
          : 'bg-surface-2 border-border text-muted hover:text-fg')
      }
    >
      <div className="flex items-center justify-center gap-2">
        <span className="text-[10px] text-muted">{sub}</span>
        <span>{label}</span>
      </div>
    </button>
  );
}

/** 小的 "value prop" 标签 —— 标题 + 一行正文,横向排 3 个。 */
function PropChip({
  testid,
  title,
  body,
}: {
  testid: string;
  title: string;
  body: string;
}) {
  return (
    <div
      data-testid={testid}
      className="rounded-md border border-border bg-surface-2 p-3 text-center"
    >
      <div className="text-[11px] font-medium text-fg">{title}</div>
      <div className="text-[10px] text-muted mt-0.5">{body}</div>
    </div>
  );
}
