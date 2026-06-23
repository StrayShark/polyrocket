import { Link } from 'react-router-dom';
import { Card } from '@/components/base/Card';
import { Book, Sparkles, GitBranch, Shield, Cpu, ExternalLink } from 'lucide-react';
import { useT } from '@/lib/i18n';

/**
 * `/help` 路由 —— 文档 + 快捷入口。
 *
 * **5 个 section**（i18n 全部）：
 *   1. Quick start —— 4 步教程（welcome 流程简化版）
 *   2. Modules —— 跳到每个 main route 的卡片网格
 *   3. Architecture —— 5 层架构简图（from `docs/overview.md §1`）
 *   4. Privacy —— "OS keyring only" 保证（spec 强约束）
 *   5. External links —— GitHub repo / docs / Tauri 文档
 *
 * **无 IPC**：纯展示页，所有内容从 i18n 字符串 + `<Link>` 来。
 */
export function Help() {
  const { t } = useT();
  return (
    <div className="space-y-4 max-w-3xl">
      <Card padding="sm">
        <div className="flex items-center gap-2">
          <Book className="w-4 h-4 text-muted" />
          <div>
            <h2 className="text-title-sm font-semibold text-fg">{t('help.title')}</h2>
            <p className="text-[11px] text-muted mt-0.5">
              {t('help.subtitle')}
            </p>
          </div>
        </div>
      </Card>

      {/* Quick start */}
      <Card title={t('help.quick.title')} description={t('help.quick.desc')}>
        <ol className="space-y-2 text-[12px] list-decimal pl-5 marker:text-muted">
          <li>{t('help.quick.step1')}</li>
          <li>
            {t('help.quick.step2').split('/wallets')[0]}
            <Link to="/wallets" className="text-accent hover:underline">/wallets</Link>
            {t('help.quick.step2').split('/wallets')[1]}
          </li>
          <li>
            {t('help.quick.step3').split('/llm-mgmt')[0]}
            <Link to="/llm-mgmt" className="text-accent hover:underline">/llm-mgmt</Link>
            {t('help.quick.step3').split('/llm-mgmt')[1]}
          </li>
          <li>
            {t('help.quick.step4').split('/markets')[0]}
            <Link to="/markets" className="text-accent hover:underline">/markets</Link>
            {t('help.quick.step4').split('/markets')[1]}
          </li>
          <li>
            {t('help.quick.step5').split('/signals')[0]}
            <Link to="/signals" className="text-accent hover:underline">/signals</Link>
            {t('help.quick.step5').split('/signals')[1].split('/analysis')[0]}
            <Link to="/analysis" className="text-accent hover:underline">/analysis</Link>
            {t('help.quick.step5').split('/analysis')[1]}
          </li>
        </ol>
      </Card>

      {/* Concepts */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        <Concept icon={Shield} title={t('help.concept.storage')}>
          <span dangerouslySetInnerHTML={{ __html: t('help.concept.storage_body') }} />
        </Concept>
        <Concept icon={Cpu} title={t('help.concept.layers')}>
          <span dangerouslySetInnerHTML={{ __html: t('help.concept.layers_body') }} />
        </Concept>
        <Concept icon={GitBranch} title={t('help.concept.fanout')}>
          <span dangerouslySetInnerHTML={{ __html: t('help.concept.fanout_body') }} />
        </Concept>
        <Concept icon={Sparkles} title={t('help.concept.brier')}>
          <span dangerouslySetInnerHTML={{ __html: t('help.concept.brier_body') }} />
        </Concept>
      </div>

      {/* Shortcuts */}
      <Card title={t('help.shortcuts.title')} description={t('help.shortcuts.desc')}>
        <div className="grid grid-cols-2 md:grid-cols-3 gap-2 text-[12px]">
          <Kbd k="⌘ K" label={t('help.shortcuts.cmdk')} />
          <Kbd k="G D" label={t('help.shortcuts.gd')} />
          <Kbd k="G M" label={t('help.shortcuts.gm')} />
          <Kbd k="G S" label={t('help.shortcuts.gs')} />
          <Kbd k="G B" label={t('help.shortcuts.gb')} />
          <Kbd k="G L" label={t('help.shortcuts.gl')} />
        </div>
      </Card>

      {/* External */}
      <Card title={t('help.external.title')}>
        <div className="space-y-1.5 text-[12px]">
          <ExtLink href="https://docs.polyrocket.app" label={t('help.external.docs')} />
          <ExtLink href="https://docs.polymarket.com" label={t('help.external.polymarket')} />
          <ExtLink href="https://github.com/StrayShark/polyrocket" label={t('help.external.github')} />
        </div>
      </Card>
    </div>
  );
}

function Concept({ icon: Icon, title, children }: { icon: React.ComponentType<{ className?: string }>; title: string; children: React.ReactNode }) {
  return (
    <Card>
      <div className="flex items-start gap-2">
        <Icon className="w-4 h-4 text-accent mt-0.5 shrink-0" />
        <div>
          <div className="text-[12px] font-semibold text-fg">{title}</div>
          <div className="text-[11px] text-muted mt-1 leading-relaxed">{children}</div>
        </div>
      </div>
    </Card>
  );
}

function Kbd({ k, label }: { k: string; label: string }) {
  return (
    <div className="flex items-center gap-2">
      <kbd className="px-1.5 h-5 inline-flex items-center text-[10px] font-mono bg-surface-2 border border-border rounded text-fg">{k}</kbd>
      <span className="text-muted">{label}</span>
    </div>
  );
}

function ExtLink({ href, label }: { href: string; label: string }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="flex items-center gap-1.5 text-fg hover:text-accent"
    >
      <ExternalLink className="w-3 h-3" />
      {label}
    </a>
  );
}
