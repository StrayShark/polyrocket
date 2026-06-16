import type { ReactNode } from 'react';

interface PlaceholderProps {
  title: string;
  icon: ReactNode;
  hint: string;
  filename?: string;
}

export function Placeholder({ title, icon, hint, filename }: PlaceholderProps) {
  return (
    <div className="px-6 py-10">
      <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
      <p className="text-xs mt-0.5" style={{ color: 'var(--muted)' }}>
        {hint}
      </p>
      <div
        className="mt-8 rounded-lg border border-dashed grid place-items-center text-center p-10"
        style={{ borderColor: 'var(--border)', color: 'var(--muted)' }}
      >
        <div style={{ color: 'var(--muted)' }}>{icon}</div>
        <div className="mt-3 text-sm">This route is a placeholder</div>
        {filename && (
          <div className="text-[11px] mt-1" style={{ color: 'var(--muted)' }}>
            Edit <code className="font-mono">{filename}</code>
          </div>
        )}
      </div>
    </div>
  );
}