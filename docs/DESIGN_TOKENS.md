# DESIGN_TOKENS.md

This document defines the design tokens for this repository.

Design tokens are **hard constraints**, not suggestions.
All UI work must use values defined here.
If a needed value does not exist, propose a token change explicitly.
Do NOT invent ad-hoc styles.

The design system is inspired by Linear (linear.app):
clean, restrained, dense, and consistent.

---

## Spacing Scale

Only the following spacing values are allowed (in px):

| Token | Value |
|------|-------|
| space-1 | 4 |
| space-2 | 8 |
| space-3 | 12 |
| space-4 | 16 |
| space-5 | 24 |
| space-6 | 32 |
| space-7 | 48 |

Rules:
- Prefer smaller spacing by default.
- Use larger spacing only to separate major sections.
- Do not mix arbitrary spacing values.

---

## Typography Scale

### Font Sizes

| Token | Size |
|------|------|
| text-xs | 12px |
| text-sm | 13px |
| text-md | 14px |
| text-lg | 16px |
| text-xl | 18px |

Rules:
- Default body text is `text-md`.
- Use size changes sparingly.
- Hierarchy should be expressed via spacing and color before size.

### Font Weights

| Token | Weight |
|------|--------|
| weight-regular | 400 |
| weight-medium | 500 |
| weight-semibold | 600 |

Rules:
- Prefer `regular` or `medium`.
- Avoid heavy bold styles.

---

## Color Palette

### Neutral Colors

| Token | Usage |
|------|------|
| color-bg | Primary background |
| color-bg-subtle | Secondary background |
| color-border | Dividers, outlines |
| color-text | Primary text |
| color-text-muted | Secondary text |

Rules:
- Neutral colors are the default.
- Avoid strong contrast unless required for accessibility.

### Accent Colors

| Token | Usage |
|------|------|
| color-accent | Primary actions, focus states |
| color-accent-muted | Hover or subtle emphasis |

Rules:
- Accent color is used sparingly.
- Never use accent color for decorative purposes.

---

## Border Radius

| Token | Value |
|------|-------|
| radius-sm | 6px |
| radius-md | 8px |
| radius-lg | 12px |

Rules:
- Use `radius-md` by default.
- Avoid overly rounded elements.

---

## Shadows

| Token | Usage |
|------|------|
| shadow-sm | Subtle elevation |
| shadow-md | Floating elements (dropdowns, modals) |

Rules:
- Shadows should be barely noticeable.
- Avoid dramatic depth effects.

---

## Layout Width

| Token | Value |
|------|------|
| content-max-width | 1080px |

Rules:
- Center main content.
- Avoid full-width layouts unless explicitly required.

---

## Z-Index Layers

| Layer | Usage |
|------|------|
| z-base | Default content |
| z-overlay | Dropdowns, tooltips |
| z-modal | Modals |

Rules:
- Do not introduce arbitrary z-index values.
