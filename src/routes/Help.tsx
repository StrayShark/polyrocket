import { Link } from 'react-router-dom';
import { Card } from '@/components/base/Card';
import { Book, Sparkles, GitBranch, Shield, Cpu, ExternalLink } from 'lucide-react';

export function Help() {
  return (
    <div className="space-y-4 max-w-3xl">
      <Card padding="sm">
        <div className="flex items-center gap-2">
          <Book className="w-4 h-4 text-muted" />
          <div>
            <h2 className="text-[13px] font-semibold text-fg">Help &amp; reference</h2>
            <p className="text-[11px] text-muted mt-0.5">
              Quick links, key concepts, and troubleshooting.
            </p>
          </div>
        </div>
      </Card>

      {/* Quick start */}
      <Card title="Quick start" description="First 5 minutes with polyrocket">
        <ol className="space-y-2 text-[12px] list-decimal pl-5 marker:text-muted">
          <li>
            Pick a theme from the topbar (Dark / Light / Matrix).
          </li>
          <li>
            Open <Link to="/wallets" className="text-accent hover:underline">/wallets</Link> and add a Polygon wallet
            (chain 137, or 80002 for Amoy testnet).
          </li>
          <li>
            Open <Link to="/llm-mgmt" className="text-accent hover:underline">/llm-mgmt</Link> and add an LLM provider
            + API key. Test connectivity before saving.
          </li>
          <li>
            Sync markets from <Link to="/markets" className="text-accent hover:underline">/markets</Link> →
            “Sync from Polymarket” button.
          </li>
          <li>
            Recompute signals from <Link to="/signals" className="text-accent hover:underline">/signals</Link>,
            then run a multi-LLM analysis from <Link to="/analysis" className="text-accent hover:underline">/analysis</Link>.
          </li>
        </ol>
      </Card>

      {/* Concepts */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        <Concept icon={Shield} title="Secret storage">
          Secrets (LLM API keys, PM CLOB creds, wallet PKs) live in the OS
          keychain. .env is dev-only and gated by{' '}
          <code className="font-mono">POLYROCKET_ENV=dev</code> +{' '}
          <code className="font-mono">POLYROCKET_KEYRING_ONLY=0</code>.
        </Concept>
        <Concept icon={Cpu} title="5-layer architecture">
          L1 Presentation (React) → L2 Application (Tauri commands) → L3 Domain
          (pure Rust) → L4 Infrastructure (DB/HTTP/scheduler) → L5 Platform
          (keyring/env/paths). Enforced by{' '}
          <code className="font-mono">scripts/check-layers.mjs</code>.
        </Concept>
        <Concept icon={GitBranch} title="Multi-LLM fan-out">
          A single <code className="font-mono">llm_analyze</code> call dispatches
          to N providers in parallel, builds a consensus, and returns per-LLM
          recommendations. 3 prompt templates ship by default.
        </Concept>
        <Concept icon={Sparkles} title="Brier score">
          Mean squared error between predicted probabilities and actual outcomes.
          Lower is better. &lt; 0.2 is good, &lt; 0.25 is acceptable, &gt; 0.25 needs work.
        </Concept>
      </div>

      {/* Shortcuts */}
      <Card title="Keyboard shortcuts" description="Work in progress — full list in v0.4.1">
        <div className="grid grid-cols-2 md:grid-cols-3 gap-2 text-[12px]">
          <Kbd k="⌘ K" label="Command palette" />
          <Kbd k="G D" label="Go to Dashboard" />
          <Kbd k="G M" label="Go to Markets" />
          <Kbd k="G S" label="Go to Signals" />
          <Kbd k="G B" label="Go to Brief" />
          <Kbd k="G L" label="Go to LLM Mgmt" />
        </div>
      </Card>

      {/* External */}
      <Card title="External resources">
        <div className="space-y-1.5 text-[12px]">
          <ExtLink href="https://docs.polyrocket.app" label="polyrocket docs" />
          <ExtLink href="https://docs.polymarket.com" label="Polymarket docs" />
          <ExtLink href="https://github.com/StrayShark/polyrocket" label="GitHub repo" />
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
