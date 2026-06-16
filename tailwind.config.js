/** @type {import('tailwindcss').Config} */
export default {
  // Three themes share IDENTICAL layout — only color tokens differ.
  // Spec v2.2 §2.0 forbids any per-theme structural change.
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
      },
      fontFamily: {
        sans: 'var(--font-sans)',
        mono: 'var(--font-mono)',
      },
      fontSize: {
        // Cursor-like density scale (per spec v2.2 §3.2)
        xs: ['11px', '16px'],
        sm: ['12px', '18px'],
        base: ['13px', '20px'],
        lg: ['16px', '24px'],
        xl: ['20px', '28px'],
        '2xl': ['28px', '36px'],
      },
      fontWeight: {
        regular: '400',
        medium: '500',
        semibold: '600',
      },
      borderRadius: {
        // All themes share these — spec v2.2 §3.4 forbids per-theme overrides
        sm: '4px',
        md: '6px',
        lg: '8px',
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
    },
  },
  plugins: [],
};