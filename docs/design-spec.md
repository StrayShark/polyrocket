# polyrocket — Merged Design Spec (v0.119)

> **来源**: Cursor DESIGN.md (537 行, v0.119) + polyrocket 现有 design system (Tailwind config + themes.css + 24 组件库)
>
> **原则**: Cursor 是 marketing site 的 design system,**polyrocket 是 desktop app** — 因此 spec 选择性采用:
> - ✅ 全采用: typography hierarchy, hairline-only depth, spacing system, single-CTA-color scarcity rule, "no bold display" rule
> - ⚠️ 主题采用: polyrocket 3 themes (Dark/Light/Matrix) + pitch green accent (足球 pivot) — 替代 Cursor 的 cream canvas + Cursor Orange
> - ➕ 新增: TimelinePill 组件 (5 个 LLM stages 用 Cursor pastel 调色板,只在分析 UI 用)
> - ➕ 新增: BadgePill 组件 (uppercase 11px caption pattern)
>
> **配套**: [`polyrocket-ui-design.md`](./polyrocket-ui-design.md) (legacy 1484 行,部分章节被本文取代) · [`polyrocket-football-ui.md`](./polyrocket-football-ui.md) (football-first UI 设计) · [`polyrocket-football-prd.md`](./polyrocket-football-prd.md) (PRD)

---

## 1. 颜色 (Colors)

### 1.1 主题色 (polyrocket-specific, 替代 Cursor cream + orange)

polyrocket 保留 3 主题 (Dark / Light / Matrix),accent 是 pitch green variants (足球 pivot):

| Theme | bg | fg | accent | accent-hover | ratio | Pass |
|---|---|---|---|---|---|---|
| **dark** (default) | `#1E1E1E` | `#D4D4D4` | `#43A047` (pitch green) | `#4CAF50` | 5.05:1 | WCAG AA ✓ |
| **light** | `#FFFFFF` | `#1F2328` | `#1B5E20` (deep pitch green) | `#2E7D32` | 7.87:1 | WCAG AA ✓ |
| **matrix** | `#0A0A0A` | `#D4D4D4` | `#a4ff00` (lime) | `#b8ff3a` | 15.92:1 | WCAG AAA ✓ |

**为什么不用 Cursor Orange**: 足球 pivot 后,brand voltage 必须是 pitch green (球场色) 而不是 Cursor Orange (IDE 工具色)。polyrocket 跟 Cursor 的 brand identity 不一样 (一个是 desktop 客户端,一个是 AI 代码编辑器 marketing site)。

### 1.2 Semantic tokens (跨主题)

```ts
--bg:           // page background
--surface:      // card background
--surface-2:    // elevated card / nested surface
--surface-hover:// hover state on interactive surface
--border:       // 1px hairline
--border-strong:// stronger divider
--fg:           // primary text (ink)
--fg-secondary: // secondary text (body)
--muted:        // tertiary text (sub-titles, captions)
--accent:       // SINGLE CTA color (pitch green per theme)
--accent-hover: // CTA hover state
--bull:         // positive/profit/edge-up (#4EC9B0 dark / #1A7F37 light / #a4ff00 matrix)
--bear:         // negative/loss/edge-down
--warning:      // caution
--shadow-card:  // very subtle (hairline-only philosophy)
--shadow-overlay:// modals, popovers
```

**单一 CTA 颜色原则 (Cursor rule)**: 一个 app 一个 accent 颜色,只在 primary CTA / 选中状态 / 关键图标用。polyrocket 的 pitch green 满足这个。

### 1.3 Timeline pastels (Cursor signature, scoped)

Cursor 的 5 个 timeline pastel 只用在 **in-product agent timeline 可视化** (Thinking / Reading / Editing / Grepping / Done)。polyrocket 复用为 LLM 分析阶段:

| Stage | Cursor 名称 | Cursor 色 | polyrocket 用途 |
|---|---|---|---|
| Thinking | `--timeline-thinking` | `#dfa88f` peach | LLM 思考中 |
| Grep | `--timeline-grep` | `#9fc9a2` mint | LLM 检索市场数据 |
| Read | `--timeline-read` | `#9fbbe0` pastel blue | LLM 读取信号 |
| Edit | `--timeline-edit` | `#c0a8dd` lavender | LLM 生成推荐 |
| Done | `--timeline-done` | `#c08532` warm gold | LLM 完成 |

**作用域**: 只在 `TimelinePill` 组件 + LLM 分析进度 UI 用,**不用在 system action 颜色 (e.g. button primary)**。

---

## 2. 字体 (Typography)

### 2.1 Font family

- **Sans**: `Inter` (Cursor 用 CursorGothic,substitute 是 Inter — polyrocket 已用 Inter,免切换)
- **Mono**: `JetBrains Mono` (Cursor 同款,polyrocket 同款 ✓)

### 2.2 完整 hierarchy (合并 Cursor + polyrocket)

| Token | Size | Weight | Line Height | Letter Spacing | Use |
|---|---|---|---|---|---|
| `display-mega` | 72px | 400 | 1.1 | -2.16px | (Cursor 独有 — polyrocket 桌面端不用) |
| `display-xl` | 36px | 400 | 1.2 | -0.72px | Welcome hero h1 (Cursor 大字) |
| `display-lg` | 26px | 400 | 1.25 | -0.325px | Page h1 (Page header,例如 /football Hub) |
| `display-md` | 22px | 400 | 1.3 | -0.11px | Section h2 |
| `display-sm` | 18px | 400 | 1.4 | 0 | Card group h3 |
| `title-md` | 18px | 600 | 1.4 | 0 | Card title / KPI value |
| `title-sm` | 16px | 600 | 1.4 | 0 | List item title / button text |
| `body-md` | 16px | 400 | 1.5 | 0 | Default body (Cursor 同款) |
| `body-tracked` | 16px | 400 | 1.5 | +0.08px | Editorial body (Cursor 同款) |
| `body-sm` | 14px | 400 | 1.5 | 0 | Footer / dense text |
| `caption` | 13px | 400 | 1.4 | 0 | Photo captions |
| `caption-uppercase` | 11px | 600 | 1.4 | +0.88px UPPER | **Section labels** + timeline pill labels + badge-pill text |
| `code` | 13px | 400 | 1.5 | 0 | Code blocks (JetBrains Mono) |
| `button` | 14px | 500 | 1.0 | 0 | CTA pill labels |
| `nav-link` | 14px | 500 | 1.4 | 0 | Top-nav / sidebar menu |

### 2.3 原则 (Cursor rules)

- **Display weight stays at 400.** 不要用 700+. polyrocket 已遵守 (0 个 `font-bold`)。
- **Negative letter-spacing on display only.** `-0.11px` 到 `-0.72px` (Cursor); polyrocket 大标题加 `tracking-tight` 类似效果。
- **JetBrains Mono on every code surface.** polyrocket 已遵守 (`code`, `kbd`, `.font-mono` 都走 mono)。

---

## 3. 间距 (Spacing)

### 3.1 Spacing scale (合并 Cursor 4-base + polyrocket Tailwind defaults)

| Token | px | Cursor 用途 | polyrocket 用途 |
|---|---|---|---|
| `xxs` | 4px | tight inline | border / icon padding |
| `xs` | 8px | tight gaps | component internal gaps |
| `sm` | 12px | inline padding | form field padding |
| `base` | 16px | default | card padding |
| `md` | 20px | — | KPI card padding |
| `lg` | 24px | card padding (Cursor `feature-card.padding`) | feature-card padding |
| `xl` | 32px | pricing-card padding (Cursor `pricing-tier-card.padding`) | large card padding |
| `xxl` | 48px | hero padding | modal padding |
| `section` | 80px | section rhythm (Cursor `hero-band.padding`, `cta-band.padding`) | **页面之间垂直间距** |

### 3.2 Section rhythm 原则 (Cursor)

- **80px 垂直 section rhythm**: 页面大 section 之间用 80px 间距,营造 editorial pacing。
- polyrocket 当前没强制 — 多数页面内部用 `p-6` (24px) 紧凑布局。新加的设计需遵循 80px 大 section。
- 桌面端 main content 容器: `max-width: 1200px` (Cursor `Grid & Container`)。

---

## 4. 圆角 (Border Radius)

### 4.1 Radius scale (完整 Cursor 8 个)

| Token | px | Cursor 用途 | polyrocket 现状 |
|---|---|---|---|
| `none` | 0px | reserved | ✅ (默认) |
| `xs` | 4px | inline tags | ✅ (Tailwind `rounded-sm`) |
| `sm` | 6px | compact rows | ✅ (Tailwind `rounded-md`) |
| `md` | 8px | CTA buttons, form inputs | ✅ (Tailwind `rounded-lg`) — **注**: polyrocket 现有 `lg`=8px,跟 Cursor `md` 对齐;**不重命名**避免破坏 5 处现有用法 |
| `card` | 12px | cards, IDE panes | ❌ **缺失**,需添加 (Cursor `lg` 等价) |
| `feature` | 16px | larger feature cards | ❌ **缺失**,需添加 (Cursor `xl`) |
| `pill` | 9999px | timeline pills, badges | ❌ **缺失**,需添加 |

**v0.119 action**: 在 tailwind config 添加 `card: '12px'` / `feature: '16px'` / `pill: '9999px'`(用 `card` 而不是 `lg` 是为了**不破坏** polyrocket 现有 `rounded-lg` = 8px 的 5+ 处使用)。

---

## 5. 深度 (Elevation & Depth)

### 5.1 Hairline-only 原则 (Cursor)

**不要 drop shadow**。深度靠:
- 1px `border` (`--border`)
- 1px `border` 更强 (`--border-strong`)
- 卡片轻微 vs canvas 颜色对比 (e.g. `#FFFFFF` surface vs `#F3F3F3` light bg)

**polyrocket 当前 shadow tokens**:
```ts
--shadow-card:    0 1px 0 (dark) / 0 1px 2px (light) / none (matrix) — 极轻
--shadow-overlay: 0 8px 24-32px — modal/dropdown
```

→ **保留** `--shadow-card` 和 `--shadow-overlay`,因为 polyrocket 是 desktop app,modal/dropdown 仍需要 shadow 跟 canvas 分离。Cursor 是 marketing site 静态页面,不需要 overlay。

**Hairline-first** 仍然是主原则,只是允许 modal 级别用 shadow。

---

## 6. 组件 (Components)

### 6.1 现有 polyrocket 组件 (24 个,保留 + 增强)

| 组件 | 状态 | Cursor 对应 |
|---|---|---|
| `Button` | ✅ 保留 | `button-primary` / `button-secondary` (高度 40px / 44px 待统一) |
| `Card` | ✅ 保留 | `feature-card` (rounded-lg 12px, padding 24px) |
| `Input` | ✅ 保留 | `text-input` (rounded-md 8px, height 44px) |
| `Pill` | ⚠️ 增强 | `badge-pill` (加 uppercase variant + pill shape) |
| `Modal` | ✅ 保留 | (Cursor 没 modal — desktop app 特有) |
| `Toast` | ✅ 保留 | (同上) |
| `EmptyState` / `ErrorState` / `Skeleton` | ✅ 保留 | (同上) |
| `ModelVersionPill` | ✅ 保留 | (desktop 特有) |
| `SidecarHealthBadge` | ✅ 保留 | (desktop 特有) |
| `AnalyzeProgress` | ✅ 保留 | (LLM 特有) |
| `TrainProgress` | ✅ 保留 | (同上) |
| `ThemeSwitcher` | ✅ 保留 | (3 主题切换) |
| `CommandPalette` / `KbdHelpDialog` | ✅ 保留 | (desktop 特有) |

### 6.2 新增组件 (Cursor-inspired)

#### TimelinePill (v0.119 NEW)

用于 LLM 分析的阶段标记。映射 Cursor 的 5 个 timeline pastel:

```tsx
import { TimelinePill } from '@/components/feedback/TimelinePill';

<TimelinePill stage="thinking" />  // peach bg
<TimelinePill stage="grep" />      // mint bg
<TimelinePill stage="read" />      // pastel blue bg
<TimelinePill stage="edit" />      // lavender bg
<TimelinePill stage="done" />      // gold bg (white text)
```

- Padding: `4px 10px`
- Font: caption-uppercase (11px / 600 / +0.88px tracking / uppercase)
- Rounded: pill (9999px)
- Bg: timeline-* token
- Text: ink (or on-primary for done stage)

#### BadgePill (v0.119 NEW)

通用 uppercase badge,遵循 Cursor 的 `badge-pill` spec:

```tsx
import { BadgePill } from '@/components/base/BadgePill';

<BadgePill variant="neutral">NEW</BadgePill>
<BadgePill variant="accent">v0.119</BadgePill>
<BadgePill variant="bull">+7.6%</BadgePill>
<BadgePill variant="bear">-3.2%</BadgePill>
```

- Padding: `4px 10px`
- Font: caption-uppercase (11px / 600 / +0.88px tracking / uppercase)
- Rounded: pill (9999px)
- Bg: surface-strong / accent/15 / bull/15 / bear/15
- Text: ink / accent / bull / bear

### 6.3 改造 Pill (v0.119 ENHANCE)

现有 Pill 加 2 个 variant:
- `variant="square"` (默认, 现有 4px rounded — 保持向后兼容)
- `variant="pill"` (新, pill shape 9999px)
- `uppercase={true}` (新, 11px / 600 / 0.88px tracking / uppercase)

**向后兼容**: 现有 5 处使用方 (MirrorPanel, AnalyzeProgress, TrainProgress, AllocationTable, MarketDetail) 不动,只是 API 增强。

---

## 7. 触控目标 (Touch Targets — Cursor rule)

- **Primary CTA**: 40px height (Cursor `button-primary`)
- **Download CTA**: 44px height (Cursor `button-download`)
- **Form input**: 44px height (Cursor `text-input`)
- **Nav link**: 14px font, padding 至少 8px (Cursor `nav-link`)

polyrocket 当前 Button 高度未明示 — 需在 `Button.tsx` 加 `size="md"` (40px) / `size="lg"` (44px) 默认值。

---

## 8. 响应式 (Responsive — Cursor 3 breakpoints)

| Name | Width | polyrocket 应用 |
|---|---|---|
| Mobile | < 640px | Sidebar 折叠成 hamburger, KPI stack 1-up |
| Tablet | 640-1024px | Sidebar 220px fixed, content 自适应 |
| Desktop | 1024-1280px | Full layout (默认) |
| Wide | > 1280px | Content caps at 1200px |

---

## 9. Do's and Don'ts (合并版)

### ✅ Do

- **单一 accent color** (polyrocket pitch green per theme; Cursor 用 orange — 同样稀缺原则)
- **Display weight stays at 400** (no `font-bold` on h1/h2/h3 — Cursor rule)
- **JetBrains Mono on every code surface** (Cursor + polyrocket 一致)
- **Timeline pastels 只在 in-product agent timeline 用** (Cursor scope rule — polyrocket TimelinePill 继承)
- **Hairline-first depth** (1px borders, 极轻 shadow — Cursor rule,polyrocket 微调)
- **80px section rhythm** between major sections (Cursor editorial pacing)
- **Compact CTA radius 8px (md)**, cards 12px (lg), timeline pills 9999px (pill)

### ❌ Don't

- 不要 bold display headings (no `font-bold` / 700+)
- 不要 drop shadows on cards (hairline-only)
- 不要用 timeline pastels 做 system action colors (e.g. button primary)
- 不要 inline hex — 永远用 `--accent` / `--bg` 等 CSS vars
- 不要单一 theme (polyrocket 有 3 themes, 跟 Cursor 单 cream 不一样 — 这是 desktop app 的合理差异)
- 不要 pure white (#FFFFFF) for dark theme background (用 #1E1E1E)
- 不要 pure black (#000000) for matrix theme bg (用 #0A0A0A)

---

## 10. 差异说明 (polyrocket vs Cursor)

| 维度 | Cursor | polyrocket | 原因 |
|---|---|---|---|
| Brand color | Orange #f54e00 | Pitch green variants | 足球 pivot,不是 IDE 工具 |
| Canvas | Cream only | 3 themes (Dark/Light/Matrix) | Desktop app 需要 dark mode |
| Display size | 72px hero | 不需要 (桌面端不渲染 marketing hero) | App 不是 marketing site |
| Shadows | None | 极轻 (overlay 用) | Modal/dropdown 需要 |
| Nav structure | 横向 top-nav | 220px sidebar | Desktop app layout |

polyrocket 选择性采用 Cursor 的 **typography / spacing / depth / scarcity / timeline palette** 原则,保留 **3 themes + pitch green + sidebar layout**。

---

## 11. 实施检查表 (v0.119 zh-fallback + Cursor-merge)

### P0 (本轮)

- [ ] **Tailwind config**: 添加 radius `lg=12px` / `xl=16px` / `pill=9999px`
- [ ] **Tailwind config**: 添加 fontSize `display-xl/36` / `display-lg/26` / `display-md/22` / `display-sm/18` / `title-md/18/600` / `body-sm/14` / `caption-uppercase/11/600/0.88`
- [ ] **Tailwind config**: 添加 letterSpacing `tighter/-0.025em` / `tight/-0.015em`
- [ ] **globals.css**: 添加 `--section: 80px` 间距 token
- [ ] **新建** `src/components/feedback/TimelinePill.tsx` (5 stage + Cursor pastel)
- [ ] **新建** `src/components/base/BadgePill.tsx` (uppercase variant)
- [ ] **改造** `src/components/base/Pill.tsx` 加 `variant="pill"` + `uppercase` props (向后兼容)
- [ ] **新建** `src/components/feedback/TimelinePill.test.tsx` + `BadgePill.test.tsx`
- [ ] **改造** `src/components/base/Pill.test.tsx` 加新 variant 测试

### P1 (后续 rounds)

- [ ] Button 高度 40/44px (P1,Cursor `button-primary` / `button-download`)
- [ ] 80px section rhythm 应用到 Football Hub / Brief / Edge Board
- [ ] TimelinePill 应用到 Analysis / Brief / Edge Board UI

---

## 12. 一句话

> polyrocket 选择性采用 Cursor design system:**typography hierarchy + hairline-only depth + 80px section rhythm + timeline pastel palette + single CTA color scarcity**;保留 **3 themes (Dark/Light/Matrix) + pitch green accent (football pivot) + 220px sidebar layout**。新加 **TimelinePill + BadgePill** 2 个组件,改造 **Pill** + **Tailwind config**。
