# rag-admin interface design system

Extracted from `src/ui/styles/global.css` and component usage in `src/ui/`.
The CSS holds the runtime truth; this file holds the *intent* behind it.

## Direction

This is a workbench. The user comes here to inspect datasets, runs, chunks, and retrievals — to read many values at once and decide what to do next. So the interface optimizes for **comparison over consumption**: monospace makes numbers stack, eyebrows label every datum, status colors carry meaning, density is the feature.

The aesthetic is dark, bordered, and quiet. Pink accent is a *pointer* that says "this is the thing on this screen" — never decoration. Shadows are absent except where elements escape document flow. A page should feel like a well-organized terminal session, not a brochure.

## The rules

1. **Density is the product.** Inline padding stays in the 4–16px range; 24px is a section break; 32/48px page-level gaps mark the *biggest* divisions only. If a card feels airy, the padding is wrong.
2. **One accent per screen.** The pink points to the page's primary subject — the primary action, the leader row, or the highlighted chart series. Hover, focus, the wordmark glyph, and link color are exempt. If two surfaces compete for it, one is wrong.
3. **Status is data.** Status colors belong only on pills, banners, log lines, and metric cells. Don't use them for decoration. Don't decorate with non-status colors.
4. **Borders draw structure.** Shadows are reserved for floating overlays (menus, the activity tray's top edge). No glow, no inner shadows — the one exception is the inset accent bar on the active activity-tray tab.
5. **Numbers align.** Use `font-variant-numeric: tabular-nums` anywhere digits stack vertically.
6. **Labels are uppercase with tracking.** Eyebrows, table headers, pills. Titles and body stay sentence case. No title case ever.
7. **The scale is the scale.** Spacing is one of 4 / 8 / 12 / 16 / 24 / 32 / 48 — nothing else, no `1.5`, no `2.5`. Type is one of five named roles — no sixth. Radius is 4px or fully round — no 6 or 8.
8. **No new colors.** Blends use `color-mix()` from existing tokens. Adding a hex literal in a component is a code smell — and breaks the light theme, since the override layer can only swap token *values*.

## Tokens

### Color

| Role | Token | Value |
|---|---|---|
| Page background | `--color-page-bg` | `#0b0d10` |
| Surface 1 (cards) | `--color-surface-1` | `#14171c` |
| Surface 2 (raised / hover) | `--color-surface-2` | `#1a1e25` |
| Surface 3 (active / pressed) | `--color-surface-3` | `#20242c` |
| Code background | `--color-code-bg` | `#0e1115` |
| Text | `--color-text` | `#e5e7eb` |
| Text muted | `--color-text-muted` | `#8b94a3` |
| Text faint | `--color-text-faint` | `#5b6470` |
| Border | `--color-border` | `#232831` |
| Border strong | `--color-border-strong` | `#2e3540` |
| Accent | `--color-accent` | `#f7768e` |
| Accent strong (hover) | `--color-accent-strong` | `#e25c72` |
| Accent soft (bg fill) | `--color-accent-soft` | `#2a1820` |
| Link | `--color-link` | `#a5b4fc` |
| Link hover | `--color-link-hover` | `#c7d2fe` |

**Surface ramp**: three steps, not five. Surface-1 = default card, surface-2 = raised/hover, surface-3 = pressed/active selection. The narrow gap between them is deliberate — you should feel the depth before you see it.

**Border vs border-strong**: `border` separates; `border-strong` invites interaction. Inputs, buttons, popovers use the strong variant so they read as touchable.

### Status

| Role | Token | When to use |
|---|---|---|
| OK | `--status-ok` `#34d399` | success states, healthy metrics, completed runs |
| Pending | `--status-pending` `#fbbf24` | in-progress, awaiting input, soft warnings |
| Fail | `--status-fail` `#f87171` | errors, destructive actions, failed runs |
| Stale | `--status-stale` `#94a3b8` | superseded, archived, no-longer-current |
| Info | `--status-info` `#60a5fa` | notes, neutral system messages |
| Winner | `--status-winner` `#FFD700` | first-place leader badge — used at most once per ranking |

Status colors are aliased as `--color-success`, `--color-danger` for semantic CSS. There is no `warn` token — soft warnings use `pending`.

### Themes

The app ships two themes. Default is dark; light mode opts in via `data-theme="light"` on `<html>`. Both speak the same token language — only the *values* swap — so components never branch on theme. If a component reaches for a literal hex, that's a token-extraction job, not a theme override.

| Role | Dark | Light |
|---|---|---|
| Page background | `#0b0d10` | `#eef1f5` |
| Surface 1 (cards) | `#14171c` | `#ffffff` |
| Surface 2 (raised / hover) | `#1a1e25` | `#e6eaf0` |
| Surface 3 (active / pressed) | `#20242c` | `#d6dce5` |
| Code background | `#0e1115` | `#f6f8fa` |
| Text | `#e5e7eb` | `#14181f` |
| Text muted | `#8b94a3` | `#4b5360` |
| Text faint | `#5b6470` | `#5f6775` |
| Border | `#232831` | `#d4dae3` |
| Border strong | `#2e3540` | `#b8bfca` |
| Accent | `#f7768e` | `#c43459` |
| Accent strong | `#e25c72` | `#9c1d40` |
| Accent soft | `#2a1820` | `#fde7ed` |
| Link | `#a5b4fc` | `#4338ca` |
| Link hover | `#c7d2fe` | `#312e81` |
| Status — OK | `#34d399` | `#15803d` |
| Status — Pending | `#fbbf24` | `#92400e` |
| Status — Fail | `#f87171` | `#b91c1c` |
| Status — Stale | `#94a3b8` | `#5e6573` |
| Status — Info | `#60a5fa` | `#1d4ed8` |
| Status — Winner | `#FFD700` | `#92560b` |
| On-accent (text on accent bg) | `#0b0d10` | `#ffffff` |
| Trend down (compare deltas) | `#d97757` | `#c2410c` |
| Badge text (on accent-soft) | `#fecdd3` | `#9c1d40` |

**Surface ramp inverts.** In dark mode the ramp goes darkest→lightest (surface-1 is the darkest of the three). In light mode it goes lightest→darkest. The *intent* is preserved: each step adds visual distinction to the previous, regardless of direction. Don't try to enforce "lighter = raised" globally; enforce "more interactive = more contrast against the resting state."

**Light-mode accents are darker on purpose.** The rose hue is the same brand pink, but values are picked so accent passes AA on white (≥4.5:1) and white text passes AA on the accent button (≥4.5:1). When you touch the accent in a component, never hardcode — read it from the token so both themes inherit the contrast guarantee.

**Overlay shadows soften in light mode.** Black drop-shadows punch holes in a white interface. The light theme rebinds the activity tray, popover, modal, and search-focus shadows to slate-tinted `rgba(15, 23, 42, 0.10–0.22)`. New floating overlays should reuse one of the existing classes rather than introduce a new shadow value.

**The toggle.** A pre-paint `<script>` in `<head>` reads `localStorage('rag-admin-theme')` (falling back to `prefers-color-scheme`) and sets `data-theme` before first paint, so there's no flash. The `<ThemeToggle />` in `nav.rs` mutates the same attribute and persists to storage. No global state, no context — the DOM attribute *is* the source of truth.

### Radius

One radius. Pills round fully.

- `--radius-1: 4px` — every surface, button, input, menu, chip
- `999px` — pills, dots, the running-jobs nav chip

Anything else (3px, 6px, 8px) is a leftover and should migrate on next touch.

### Spacing

Base unit 4px. The Tailwind utility number equals the step.

| Step | Value | Used for |
|---|---|---|
| 1 | 4px | dot/pill internals, tight inline gaps |
| 2 | 8px | icon-to-label, default inline gap |
| 3 | 12px | toolbar gap, card header gap |
| 4 | 16px | card padding, inter-block gap |
| 6 | 24px | section padding, inter-card gap |
| 8 | 32px | page-level — between unrelated sections |
| 12 | 48px | page-level — page-top hero spacing, max rhythm |

If a value isn't in this table, it isn't in the system.

### Typography

JetBrains Mono throughout. Base 13px / line-height 1.55.

Five levels, max:

| Role | Class | Size | Used for |
|---|---|---|---|
| Page title | `.page-title` | 24px / 1.5rem | page H1 only |
| Section title | `.section-title` | 18px / 1.125rem | card and section headings |
| Body | (inherited) | 13px / 0.8125rem | default body, table cells, buttons |
| Caption | `text-xs` | 12px / 0.75rem | secondary body, log lines, drilldowns |
| Eyebrow | `.eyebrow` | 11px / 0.6875rem | labels, table headers, IDs — UPPERCASE + `letter-spacing: 0.04em` |

Weights: 500 (medium) for emphasis, 600 (semibold) for titles. Never 700+ — at monospace 13px it stops feeling like the same font.

If you reach for a sixth level, the hierarchy is wrong — fix the layout, not the type ramp.

## Patterns

### Primitives

#### Button — `.btn`
Padding 0.4rem 0.75rem, font 0.8125rem, gap 0.5rem (icon→label), `--radius-1`, 1px `--color-border-strong`. Background ramps `surface-2` → `surface-3` (hover) → `surface-1` (active).

Variants: `btn-primary` (accent bg, dark text — *this is your one accent*), `btn-ghost` (transparent until hover), `btn-danger` (danger color, transparent bg), `btn-compact` (0.2rem 0.5rem padding, 12px font).

#### Input — `.input`
Padding 0.4rem 0.625rem, font 0.8125rem, `--radius-1`, 1px `--color-border-strong`. Background `surface-1`; on focus, border → accent and bg → `surface-2`. The bg shift is the focus indicator — borders alone wouldn't carry it.

#### Pill — `.pill`
Padding 0.125rem 0.5rem, `999px` radius, 1px border color-mixed from status × `--color-border`, bg `--color-surface-2`. Font 0.6875rem, weight 500, UPPERCASE, `letter-spacing: 0.02em`.

A 6×6 filled `::before` dot in `currentColor` is the **live-status signal**. Status variants (`pill-ok`, `pill-pending`, `pill-fail`, `pill-stale`, `pill-info`, `pill-winner`, `pill-accent`) all carry it; `pill-pending` animates with a pulse. The `pill-neutral` variant is a **label, not a status** — it has no dot. Use `pill-neutral` for things like "draft" or a tag; reach for a status pill when the value can change.

#### Eyebrow — `.eyebrow`
0.6875rem `--color-text-muted`, `letter-spacing: 0.04em`. The default label surface for table headers, metadata, log-stream level tags.

### Surfaces

#### Card — `.surface`, `.surface-raised`
1px `--color-border`, `--radius-1`. Background `surface-1` (default) or `surface-2` (raised — for nested or selected). Internal padding 1rem; surfaces with a top bar use `.surface-header` (margin-bottom 0.75rem) or `.surface-header-sticky` for scrolling content.

#### Log viewer — `.log-pre`, `.log-stream`
Background `--color-code-bg` (the only place this surface is used), 1px `--color-border`, `--radius-1`. `.log-pre` = 12px font with 0.75rem padding; `.log-stream` = 11px font with 0.125rem row padding for terminal-style density. Line color by level: info=text, warn=pending, error=fail, success=ok.

### Data displays

#### Data table — `.data-table`
Cell padding 0.625rem 0.875rem, body font 0.8125rem. Header is 0.6875rem UPPERCASE `--color-text-muted` on `--color-surface-1`. Rows separated by 1px `--color-border` bottom only (no top, no last-row). Row hover `--color-surface-2` + pointer cursor. No vertical lines — the visual rhythm comes from row stripes of light, not from cells.

#### Variants table — `.variants-table`
Same shape as data-table, plus: sticky header, tabular numerics, sortable headers (hover lifts to `--color-text`).

The **leader row** uses a 2px accent left border + 35% accent-soft row bg. This *is* the accent for the page — pages built around the variants table use `btn-ghost` for actions to preserve the rule of one.

Cells can be tagged `metric-good` / `metric-ok` / `metric-poor` for status-colored values. Drilldown rows nest on `--color-surface-2` with no padding (the drilldown sets its own).

#### Metric bar — `.metric-bar-row`
Three-column grid `9rem 1fr 6.5rem` — label / track / value. Track is 10px tall, 2px radius, `surface-2` bg with 1px border. The best value gets a 2px-wide accent vertical tick with a 1px page-bg outline ring so it survives across any fill color. Stddev shows as a faint 18%-opacity ribbon overlay.

The axis below uses a matching three-column grid so labels align under their tracks. 0.65rem text, tabular nums.

#### Metric chip — `.metric-chip`
0.125rem 0.375rem padding, `surface-2` bg, `--radius-1`, 11px monospace. Label muted, value carries `metric-good` / `metric-poor` color when present.

### Status surfaces

#### Banner — `.promote-status`, `.advisor-banner`
3px left border in the status color, 0.5rem 0.75rem padding, `--radius-1`, background = 10% color-mix of the status color into transparent (or `surface-2` for the advisor banner). The left border is the entire visual identity — no icon, no fill saturation beyond 10%.

### Floating overlays

#### Menu popover — `.actions-menu-popover`, `.app-nav-menu-popover`, `.nav-running-menu`
Anchored 6px below the trigger. `surface-1` bg, 1px `--color-border-strong`, `--radius-1`. Drop shadow `0 12px 28px -16px rgba(0,0,0,0.55)` (or `0 16px 32px -18px rgba(0,0,0,0.65)` for the actions menu). Internal padding 0.3–0.4rem; items use `--radius-1` with `surface-2/3` hover.

The active-item marker is a **2px-wide accent bar inside the item**, never a fill. This keeps the menu visually quiet when one item is selected — selection is a hint, not a flag.

#### Activity tray — `.activity-tray`
Fixed bottom of viewport. `--color-code-bg` background; top edge marked by 1px `--color-border-strong` + a negative-y drop shadow `0 -16px 32px -16px rgba(0,0,0,0.55)`. Tab strip uses right-bordered tabs; the active tab gets `box-shadow: inset 0 2px 0 0 var(--color-accent)` — the only inset shadow in the system, used as an indicator-not-a-shadow.

### Navigation

#### Nav link — `.app-nav-link`, `.app-nav-menu-trigger`
Underline-on-active. 1px transparent bottom border that becomes `--color-accent` when `aria-current="page"` or `.is-active`. No pill, no fill, no chip. The underline is the entire affordance.

### Brand

#### Wordmark — `.wordmark`
0.9375rem, weight 600, `letter-spacing: -0.01em`, prefixed with `▍` (U+258D) in accent color.

The `▍` glyph is **brand-locked**. It appears nowhere else — not as a bullet, not as a divider, not as decoration. Treating it as reusable dilutes the wordmark.

## Anti-patterns

- `shadow-*` Tailwind utility on anything that isn't a floating overlay
- `rounded-md` / `rounded-lg` (these are 6px and 8px — not in the system)
- Spacing values `1.5`, `2.5`, `5`, `7`, `9`, `10`, `11` (off-scale)
- Hex color literals in components — go through tokens or `color-mix` (also breaks the light theme)
- `rgba(0, 0, 0, …)` shadows that won't work on a white surface — soften via the `[data-theme="light"]` overrides or use `color-mix(in srgb, …)` with a token
- `text-base`, `text-lg`, `text-xl`, `text-2xl` — use the named role classes
- Title case anywhere
- Multiple accent surfaces on the same screen
- The `▍` glyph anywhere outside the wordmark
- Box-shadow as the *primary* boundary of a static element
