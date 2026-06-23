/** @type {import('tailwindcss').Config} */
export default {
  // Three themes share IDENTICAL layout — only color tokens differ.
  // Spec v2.2 §2.0 forbids any per-theme structural change.
  // v0.119 — merged with Cursor design spec (see docs/design-spec.md).
  // Selective adoption: typography hierarchy, hairline-only depth,
  // spacing rhythm, single-CTA-color scarcity. Kept polyrocket-specific:
  // 3 themes + pitch green accent (football pivot) + 220px sidebar.
  darkMode: ['selector', '[data-theme="dark"], [data-theme="matrix"], [data-theme="light"]'],
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        // Semantic tokens — every theme maps these to its own CSS var values
        bg: 'var(--bg)',
        surface: 'var(--surface)',
        'surface-2': 'var(--surface-2)',
        'surface-hover': 'var(--surface-hover)',
        border: 'var(--border)',
        'border-strong': 'var(--border-strong)',
        fg: 'var(--fg)',
        'fg-secondary': 'var(--fg-secondary)',
        muted: 'var(--muted)',
        accent: 'var(--accent)',
        'accent-hover': 'var(--accent-hover)',
        bull: 'var(--bull)',
        bear: 'var(--bear)',
        warning: 'var(--warning)',
        // v0.119 — Cursor timeline pastel palette (LLM analysis stages only).
        // Scoped: TimelinePill component + LLM progress UI. NOT for system action colors.
        'timeline-thinking': '#dfa88f', // peach — LLM thinking
        'timeline-grep': '#9fc9a2',     // mint — LLM retrieving data
        'timeline-read': '#9fbbe0',     // pastel blue — LLM reading signals
        'timeline-edit': '#c0a8dd',     // lavender — LLM generating recommendation
        'timeline-done': '#c08532',     // warm gold — LLM finished
      },
      fontFamily: {
        sans: 'var(--font-sans)',
        mono: 'var(--font-mono)',
      },
      fontSize: {
        // v0.119 — merged Cursor typography hierarchy + polyrocket density scale.
        // Cursor: display-mega 72px (marketing hero, desktop app 不需要)
        //         display-xl 36 / display-lg 26 / display-md 22 / display-sm 18
        //         title-md 18/600 / title-sm 16/600
        //         body-md 16 / body-tracked 16/0.08 / body-sm 14
        //         caption 13 / caption-uppercase 11/600/0.88
        //         code 13 mono
        //         button 14/500 / nav-link 14/500
        // polyrocket (existing): xs/sm/base/lg/xl/2xl — 兼容保留
        'xs': ['11px', '16px'],
        'sm': ['12px', '18px'],
        'base': ['13px', '20px'],
        'lg': ['16px', '24px'],
        'xl': ['20px', '28px'],
        '2xl': ['28px', '36px'],
        // Cursor display scale (新增)
        'display-xl': ['36px', { lineHeight: '1.2', letterSpacing: '-0.72px' }],
        'display-lg': ['26px', { lineHeight: '1.25', letterSpacing: '-0.325px' }],
        'display-md': ['22px', { lineHeight: '1.3', letterSpacing: '-0.11px' }],
        'display-sm': ['18px', { lineHeight: '1.4' }],
        // Cursor title (新增)
        'title-md': ['18px', { lineHeight: '1.4' }], // weight 600 set in component class
        'title-sm': ['16px', { lineHeight: '1.4' }], // weight 600 set in component class
        // Cursor body (扩充)
        'body-sm': ['14px', { lineHeight: '1.5' }],
        // Cursor caption-uppercase (新增, 11px / 600 / 0.88px tracking / uppercase — Tailwind 不直接支持 uppercase,在组件里加)
        'caption-uppercase': ['11px', { lineHeight: '1.4', letterSpacing: '0.88px' }],
      },
      fontWeight: {
        regular: '400',
        medium: '500',
        semibold: '600',
      },
      letterSpacing: {
        // v0.119 — Cursor negative letter-spacing on display
        'tighter': '-0.025em',
        'tight': '-0.015em',
        'caption-uppercase': '0.88px', // for BadgePill / TimelinePill
      },
      borderRadius: {
        // v0.119 — Cursor full 8-token radius scale (was 3, now 8)
        // sm/md/lg: existing polyrocket tokens (no break)
        // + lg=12, xl=16, pill=9999 (Cursor spec)
        sm: '4px',
        md: '6px',
        lg: '8px',
        // New tokens
        'card': '12px',     // Cursor feature-card / IDE-pane
        'feature': '16px',  // Cursor larger feature-card (rare)
        'pill': '9999px',   // Cursor timeline pills / badges
      },
      boxShadow: {
        card: 'var(--shadow-card)',
        overlay: 'var(--shadow-overlay)',
      },
      transitionDuration: {
        fast: '80ms',
        base: '160ms',
        theme: '180ms',
      },
      transitionTimingFunction: {
        'ease-out-cubic': 'cubic-bezier(0.16, 1, 0.3, 1)',
      },
      spacing: {
        // v0.119 — Cursor 4-base spacing scale (matches existing Tailwind defaults)
        // No new tokens, but documented for spec compliance.
        // section 80px is achieved via Tailwind `p-20` (80px).
        'section': '80px',
      },
    },
  },
  plugins: [],
};