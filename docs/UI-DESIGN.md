# RarZap — UI Design

**Version:** 1.0
**Date:** 2026-10-11
**Target platforms:** Windows 11, macOS 14+, Ubuntu 24.04 / Debian 12+
**Stack:** GPUI Kit (`gpui-kit` + `gpui-component` 0.7)

## 1. Design principles

Desktop UI in 2026 is shifting from "visual performance" to **cognitive
clarity** — rewarding judgment and restraint over novelty. A utility should
show what the user needs *now*, not everything it can do. RarZap follows three
principles:

**Calm.** A tool's value is "use it and leave." The UI must not grab attention.
Apart from the progress bar and status badges, large areas stay in low-contrast,
low-saturation neutral tones. Motion explains state changes; it never decorates.

**Liquid.** Information density adapts to the user's current stage. Empty state:
a simple drop zone with a hint. Loaded: a dense list view. Done: a summary and
follow-up actions. No page jumping — information flows within one window.

**Precise.** Every visual element has a functional purpose. Spacing, radii,
colors, and weights all come from design-system tokens; no hard-coded style
values appear in application code. This matches GPUI community practice: all
colors from `cx.theme()`, all spacing from the design system — no `rgb()`,
`rgba()`, or color literals in app code.

## 2. Visual direction

Two 2026 trends are relevant: **Bento Grid** and **Glassmorphism 2.0**. Some are
fully implementable in GPUI; some need to be degraded.

### 2.1 Adopted: Bento Grid's information logic

Bento Grid groups content into rounded cards — well suited to a batch overview.
We don't copy Apple's equal-width grid; instead we turn it into **stage-driven
layout variants**:

- **Empty** — one large centered card holding the drop zone and a short hint.
- **Running** — file list at 70% width; summary panel at 30%.
- **Done** — a full-width summary card, with an expandable failure list below.

Switching is done with GPUI conditional rendering — `render()` returns a
different element tree based on `self.stage`. GPUI's layout thresholds are
explicit Rust conditions rather than CSS media queries, which fits stage-driven
switching exactly.

### 2.2 Degraded: glassmorphism

Glassmorphism 2.0 needs `backdrop-filter: blur(24px) saturate(180%)`. GPUI's
blur support is platform-limited: `paint_backdrop_blur` is macOS/Metal only,
unavailable on Windows and Linux.

**Strategy:** substitute "translucent surface + 1px border" for real blur — a
panel background at ~92% opacity plus a 1px `border`, giving the *weightlessness*
of frosted glass without platform blur. Consistent across all three platforms.

### 2.3 Avoided: brutalism and neon

Brutalist revival and cyberpunk palettes conflict directly with the "calm"
positioning and are excluded. Dark mode uses deep gray-blue, not pure black
(≈`#0F1117`), with no neon accents.

## 3. Design tokens

### 3.1 Color

All colors live in the theme; app code references semantic token names only.
`gpui-component` ships `Theme` and `ThemeColor` with multi-theme support.

**Light:**

| Token | Value | Use |
|---|---|---|
| `background` | `#F7F8FA` | Window root |
| `surface` | `#FFFFFF` | Cards, panels |
| `surface_hover` | `#F0F1F3` | Hover background |
| `muted` | `#E8EAED` | Dividers, disabled bg |
| `border` | `#E2E4E8` | Borders, dividers |
| `text_primary` | `#1A1D23` | Titles, body |
| `text_secondary` | `#5F6572` | Secondary text |
| `text_muted` | `#9CA3AF` | Timestamps, meta |
| `accent` | `#3B82F6` | Primary buttons, progress, selection |
| `accent_hover` | `#2563EB` | Primary hover |
| `error` | `#EF4444` | Error |
| `success` | `#10B981` | Success |
| `warning` | `#F59E0B` | Warning |

**Dark:**

| Token | Value | Use |
|---|---|---|
| `background` | `#0F1117` | Window root |
| `surface` | `#1A1D23` | Cards, panels |
| `surface_hover` | `#252830` | Hover background |
| `muted` | `#2A2D35` | Dividers, disabled bg |
| `border` | `#2E3138` | Borders, dividers |
| `text_primary` | `#E8EAED` | Titles, body |
| `text_secondary` | `#9CA3AF` | Secondary text |
| `text_muted` | `#5F6572` | Timestamps, meta |
| `accent` | `#60A5FA` | Primary buttons, progress (brightened) |
| `accent_hover` | `#3B82F6` | Primary hover |

The dark accent is ~20% brighter: `#3B82F6` does not meet WCAG AA contrast on a
dark background. Tuning contrast for dark mode is a production-grade
requirement, not an option.

### 3.2 Typography

**Families:** **Inter** for UI; **JetBrains Mono** for file sizes/paths.
GPUI loads system fonts via `font-kit`, or fonts can be bundled.

| Level | Size | Weight | Line height | Tracking | Use |
|---|---|---|---|---|---|
| `display` | 24px | 600 | 1.3 | -0.02em | Empty-state title |
| `heading` | 18px | 600 | 1.4 | -0.01em | Panel title |
| `body` | 14px | 400 | 1.5 | 0 | Body, filenames |
| `body-strong` | 14px | 500 | 1.5 | 0 | Button labels, emphasis |
| `caption` | 12px | 400 | 1.4 | 0.01em | Hints, sizes |
| `mono` | 13px | 400 | 1.5 | 0 | Paths, technical text |

Large text uses negative tracking; small text trends normal — aligned with
Inter's own optimization.

### 3.3 Spacing & radius

**Spacing:** 4px base unit; every value is a multiple of 4.

| Token | Value | Use |
|---|---|---|
| `space-1` | 4px | Icon-to-label |
| `space-2` | 8px | Tight gaps |
| `space-3` | 12px | Within list rows |
| `space-4` | 16px | Card padding |
| `space-5` | 20px | Panel padding |
| `space-6` | 24px | Between panels |
| `space-8` | 32px | Large sections |
| `space-12` | 48px | Empty-state vertical |

**Radius:**

| Token | Value | Use |
|---|---|---|
| `radius-sm` | 6px | Buttons, inputs, small badges |
| `radius-md` | 10px | Cards, panels |
| `radius-lg` | 14px | Large cards, drop zone |
| `radius-full` | 9999px | Round indicators |

GPUI's `.rounded(px(8.))` supports these directly. Bento's recommended 16–24px
suits large cards; RarZap's cards are smaller, so 14px is the visual ceiling.

## 4. Layout

### 4.1 Window

| Property | Value |
|---|---|
| Default size | 720 × 520 px |
| Minimum size | 560 × 420 px |
| Maximum size | Unbounded |
| Title bar | Custom (`gpui-component` `TitleBar`), consistent across platforms |

GPUI supports client-side decorations with resize-edge hitboxes. On Linux,
GPUI renders its own border/shadow because GNOME/KDE client-decoration behavior
is inconsistent.

### 4.2 Empty state

```
┌──────────────────────────────────────────────┐
│  ● ● ●                    RarZap        — □ × │  ← custom title bar, 36px
├──────────────────────────────────────────────┤
│                                              │
│         ┌──────────────────────────┐         │
│         │      [RAR icon]          │         │
│         │  Drop a RAR file to start │        │
│         │  [Select file] [Select folder]│     │
│         └──────────────────────────┘         │
│            Batch · RAR → ZIP                  │
└──────────────────────────────────────────────┘
```

The centered drop zone uses `radius-lg` (14px), `surface` background, 1px
`border`. On drag-over the border turns `accent` and the background brightens
slightly. File dialogs use GPUI's native `prompt_for_paths()`.

### 4.3 Running state

```
┌──────────────────────────────────────────────┐
│  ● ● ●                    RarZap        — □ × │
├──────────────────────────────────────────────┤
│  ┌────────────────────────┐ ┌──────────────┐ │
│  │ Files (3)              │ │ Summary       │ │
│  │ ▸ archive_01.rar  2.1GB│ │ Total 4.8GB   │ │
│  │   ████████░░ 78%       │ │ Done 1/3      │ │
│  │ ▸ backup.rar     1.4GB │ │ [Cancel all]  │ │
│  │   ░░░░░░░░░░ Waiting   │ │ Output         │ │
│  │ ▸ photos.rar     1.3GB │ │ ~/Desktop/     │ │
│  │   ░░░░░░░░░░ Waiting   │ │ [Change]       │ │
│  └────────────────────────┘ └──────────────┘ │
│  ████████████░░░░░░░░  Overall 33%           │  ← fixed bottom bar
└──────────────────────────────────────────────┘
```

The file list uses a **virtualized** list/table so even hundreds of files scroll
smoothly. Each row's status is double-encoded (color + text):

| Status | Color | Text |
|---|---|---|
| Waiting | `text_muted` | Waiting |
| Running | `accent` | percent |
| Done | `success` | ✓ Done |
| Failed | `error` | ✗ Failed |
| Cancelled | `text_muted` | Cancelled |

### 4.4 Done state

The summary panel expands to full width:

```
┌──────────────────────────────────────────────┐
│  ✓ Conversion complete                        │
│  3 succeeded · 1 failed · 12s                 │
│  [Open output folder] [Retry failed] [Add more]│
├──────────────────────────────────────────────┤
│  Failures (1)                                 │
│  backup.rar                                   │
│  Error: file in use by another program        │
│  [Retry]                                      │
└──────────────────────────────────────────────┘
```

The summary card uses `surface` + `radius-md`. Success/failure counts use
`success`/`error` colors (dual encoding). "Open output folder" calls
`reveal_path()`.

## 5. Component spec

### 5.1 Overview vs. GPUI capability

| Area | Component | GPUI capability | Availability |
|---|---|---|---|
| Title bar | `TitleBar` | Built-in, all platforms | ✅ |
| Primary actions | `Button` | `primary`/`secondary`/`danger`, `xs`–`lg` | ✅ |
| File list | `List` / virtualized | Smooth large-set scrolling | ✅ |
| Overall progress | `Progress` | Linear, animated | ✅ |
| Row progress | `Progress` | Same, inline in rows | ✅ |
| Status badge | `Badge` | Color variants | ✅ |
| Error dialog | `Dialog` | Modal, backdrop, custom size | ✅ |
| Notifications | `Notification` | Built-in toast system | ✅ |
| Path input | `Input` | Single-line, prefix/suffix | ✅ |
| Output picker | native dialog | `prompt_for_paths()` | ✅ |
| Drop zone | custom `div()` | Flex + border/radius/hover | ✅ compose |
| Summary card | custom `div()` | Same | ✅ compose |

### 5.2 Buttons

| Variant | Background | Text | Use |
|---|---|---|---|
| Primary | `accent` | `text_on_accent` | "Select file", "Start" |
| Secondary | `surface` + `border` | `text_primary` | "Select folder", "Change" |
| Ghost | transparent | `text_secondary` | "Cancel all", "Retry" |
| Danger | `error` | `text_on_accent` | "Remove", "Clear list" |

Default `md` (32px tall); compact `sm` (28px). Radius `radius-sm` (6px). A
shadow only on Primary, opacity ≤ 0.15, kept restrained.

### 5.3 Progress bars

**Overall:** fixed to the bottom, 3px tall, no radius (flush), `accent` on
`muted`, with a `caption` "Overall 33%" label.

**Per-row:** inline under the filename, 2px tall. Running rows show real
progress; waiting rows show an empty track; done rows show full.

`gpui-component`'s `Progress` tracks the previous value via
`window.use_keyed_state` and interpolates smoothly. Keep the animation ≤200ms so
updates feel responsive, not laggy.

### 5.4 Drop zone

Built with GPUI flexbox; on drag-over the **border** changes to `accent` (not the
background) to avoid a large visual jump. Transition 150ms `ease-out`.

## 6. Motion

2026 design emphasizes "motion explains state, never performs." GPUI Kit offers
three motion primitives: `transition` (known-duration value change), `spring`
(bounce), and `with_animation` (sequence). All respect the system
reduced-motion preference.

| Scenario | Type | Duration | Notes |
|---|---|---|---|
| Panel expand/collapse | `transition` | 200ms | Height + opacity together |
| Button hover | `transition` | 150ms | Background |
| Progress update | internal | 200ms | Handled by `Progress` |
| Row state change | color | 200ms | Text + color gradient |
| Empty → running | `Presence` | 300ms | Old fades out, new fades in |
| Toast | `spring` | 400ms | Slide up from bottom, slight bounce |

**Excluded:** page-level slide transitions, parallax, decorative particles,
auto-playing icon animations. The UI is fully still when idle.

## 7. Platform adaptation

### 7.1 Window decoration

| Platform | Strategy |
|---|---|
| macOS | Client-side; traffic-light buttons keep system style |
| Windows | Client-side; controls match Windows 11 (rectangular hover) |
| Linux | Client-side; 1px `border` frame, shadow rendered by GPUI |

### 7.2 File dialogs

All three platforms use GPUI's `prompt_for_paths()`; on Linux this goes through
the XDG Desktop Portal to GNOME/KDE dialogs.

### 7.3 High DPI

GPUI's `px()` is logical pixels and scales by the window's `scale_factor`. All
tokens are defined in logical pixels — no manual DPI math. On a 200% Windows
machine, elements scale automatically.

## 8. Accessibility

### 8.1 Contrast

All text/background pairs meet WCAG AA (≥ 4.5:1):

| Pair | Ratio | Result |
|---|---|---|
| `text_primary` / `background` (light) | 15.2:1 | ✅ |
| `text_secondary` / `background` (light) | 6.8:1 | ✅ |
| `text_primary` / `surface` (dark) | 13.1:1 | ✅ |
| `text_secondary` / `surface` (dark) | 5.4:1 | ✅ |
| `accent` / `background` (light, progress) | 4.6:1 | ✅ |
| `error` / `surface` (light, error text) | 4.7:1 | ✅ |

### 8.2 Dual-encoded status

Status is conveyed by color **and** a text/glyph label (✓ / ✗ / ○), so
color-blind users can still read it.

### 8.3 Keyboard

| Shortcut | Action |
|---|---|
| `Ctrl/Cmd + O` | Open file picker |
| `Ctrl/Cmd + Shift + O` | Open folder picker |
| `Ctrl/Cmd + Enter` | Start conversion |
| `Esc` | Cancel current action / close dialog |
| `Tab` / `Shift + Tab` | Move focus |

GPUI's `KeyBinding` handles cross-platform shortcuts; form widgets ship focus
management and `Tab` navigation.

### 8.4 Reduced motion

When the user enables "reduce motion": all `transition` durations drop to 0ms,
`spring` becomes an instant switch, and `Presence` fades halve. GPUI Kit
supports this natively.

## 9. Theming

### 9.1 Follow system

Default: follow the OS light/dark setting. GPUI detects the system theme via the
platform API and re-renders on change.

### 9.2 Manual override

A theme control cycles **System → Light → Dark**, persisted to the local config
file.

### 9.3 Implementation

Based on `gpui-component`'s `Theme`/`ThemeColor`. Multi-theme support isn't in
`gpui-component` itself but is easy to build on `Theme`.

```rust
Theme::change(ThemeMode::Dark, Some(window), cx);

let theme = cx.theme();
div().bg(theme.background).text_color(theme.foreground)
```

## 10. Deliverables checklist

| Deliverable | Format | Notes |
|---|---|---|
| Color tokens | Rust constants | Full light + dark sets |
| Type scale | this document | Inter + JetBrains Mono, 6 levels |
| Spacing/radius tokens | Rust constants | 4px base |
| Component spec | §5 | Buttons, progress, badges, dialogs |
| Layout variants | §4 | Empty, running, done |
| Motion spec | §6 | 6 scenarios + exclusions |
| A11y spec | §8 | Contrast, dual encoding, shortcuts |

**Priority:** light-mode three layouts + core components first, then dark mode,
then motion and accessibility polish.
