# Morph Studio UI Redesign — Ember

## Summary

Redesign the Morph Studio React frontend from a 3-column layout with a flat toolbar into a focused 2-column layout with a tabbed right panel. Apply a distinctive warm dark visual style ("Ember") with Stripe-level craft: subtle amber-tinted borders, gradient accents, generous spacing, and refined typography.

## Layout — Focused Studio

### Header
- **Left**: Logo icon (gradient amber square with "M") + "Morph Studio" wordmark
- **Center**: Pill tab switcher with 3 tabs — Effects, Pipeline, Presets. Active tab has an inner background highlight with subtle shadow. Inactive tabs are muted.
- **Right**: Stream URL input (compact, ~250px) + Randomize button (ghost style with muted border)

### Body (flex row, fills remaining height)
- **Left ~70%**: Video player (black background, HLS stream, LIVE badge with red glow dot)
- **Right ~280px**: Single panel whose content swaps based on active header tab. Separated from video by a 1px amber-tinted border.

### Footer — Status Bar
- Slim bar with: green glow dot + segment countdown, effects timing, total timing, frame count
- Dot separators (not pipe characters)
- `font-variant-numeric: tabular-nums` on all numbers
- Performance color coding: green < 2s, yellow < 4s, red >= 4s

## Visual Style — Ember

### Color Tokens
| Token | Value | Usage |
|-------|-------|-------|
| `bg-base` | `#0c0a09` | Page background |
| `bg-surface` | `rgba(245,158,11,0.01)` | Panel backgrounds |
| `bg-elevated` | `rgba(245,158,11,0.06)` | Active/selected items |
| `border-subtle` | `rgba(245,158,11,0.06)` | Panel dividers, status bar |
| `border-active` | `rgba(245,158,11,0.12)` | Selected item borders |
| `border-muted` | `rgba(255,255,255,0.04)` | Inactive item borders |
| `accent-gradient` | `linear-gradient(135deg, #f59e0b, #f97316)` | Number badges, slider fills, primary buttons |
| `text-primary` | `#e7e5e4` | Primary text |
| `text-secondary` | `#a8a29e` | Secondary text, inactive items |
| `text-tertiary` | `#78716c` | Metadata, timestamps |
| `text-label` | `#57534e` | Section labels (uppercase) |
| `text-muted` | `#44403c` | Hints, separators |
| `green-glow` | `#22c55e` with `box-shadow: 0 0 6px rgba(34,197,94,0.4)` | Status dots, healthy metrics |

### Typography
- Font stack: `-apple-system, BlinkMacSystemFont, 'Inter', sans-serif` (via Tailwind defaults)
- Section labels: 10px, uppercase, `letter-spacing: 1.5px`, `text-label` color, font-weight 600
- Body text: 11-12px
- Status bar: 10px
- `font-variant-numeric: tabular-nums` on all numeric displays

### Borders & Surfaces
- All borders use rgba with 4-12% opacity — never hard lines
- Header bottom border: `border-subtle`
- Status bar top border: `border-subtle`
- Panel left border: `border-subtle`
- Active items: `border-active` + `bg-elevated`
- Inactive items: `border-muted` + `rgba(255,255,255,0.02)`

## Tab Content

### Effects Tab
- Section label: "EFFECTS"
- Scrollable list of available effects
- Each effect: row with name, click-to-add behavior
- Style: `bg-muted` background, `border-muted` border, rounded-lg
- Hover: slight background lift
- Bottom hint text: "click to add to pipeline"

### Pipeline Tab
- Section label: "PIPELINE" with effect count on the right (e.g. "3 active")
- Ordered list of pipeline slots with drag-to-reorder (dnd-kit, unchanged)
- Each slot: gradient number badge (active) or muted badge (inactive), effect name, green dot for enabled, red for disabled
- Selected slot: `bg-elevated` + `border-active`
- Below the pipeline list (scrollable, same panel): **Parameters** section
  - Section label: "PARAMETERS"
  - Slider for each param: label left, value right in accent color with tabular-nums
  - Slider track: 3px height, `rgba(white,0.06)` background, gradient fill
  - Remove button at bottom: subtle red-tinted destructive style
- If no slot selected: italic hint "Select an effect to edit parameters"
- If pipeline empty: italic hint "No effects — add from the Effects tab"

### Presets Tab
- Section label: "PRESETS"
- Top actions row: **Save Current** (gradient amber button) + **Randomize** (ghost button with muted border)
- Clean list of presets, each row:
  - Left: 28px rounded icon square (gradient for first/active, muted for others)
  - Center: preset name (12px, primary text) + subtitle line (effect count, 9px tertiary)
  - Right: `×` delete button (muted, hover to red)
- Click anywhere on the row to apply
- Bottom hint: "click to apply · × to delete"
- Save flow: clicking Save Current shows an inline input field replacing the button (same as current behavior, restyled)

## Component Changes

### New Components
- **Header.tsx** — replaces PresetBar. Contains logo, tab switcher, stream URL input, save button. Manages `activeTab` state.
- **TabPanel.tsx** — wrapper that renders the correct tab content based on `activeTab`. Contains Effects, Pipeline, and Presets views.

### Modified Components
- **App.tsx** — remove 3-column layout, replace with Header + 2-column body (video + TabPanel) + StatusBar. Lift `activeTab` state here.
- **EffectLibrary.tsx** — restyle to Ember palette, adjust for full-width right panel context instead of narrow sidebar.
- **PipelineEditor.tsx** — restyle with gradient badges, ember colors. Add effect count display.
- **ParamPanel.tsx** — restyle sliders, remove button. Integrated below pipeline list (not a separate section).
- **Slider.tsx** — custom styled slider with gradient track fill and accent value display.
- **StatusBar.tsx** — restyle to Ember palette, dot separators, glow effects.
- **VideoPlayer.tsx** — restyle LIVE badge with glow dot.

### Removed Components
- **PresetBar.tsx** — replaced by Header.tsx

### Unchanged
- All hooks: `useEffects`, `usePipeline`, `usePresets` — no API changes
- All types in `types/index.ts`
- `vite.config.ts`
- `hls.js` video logic

## CSS Approach
All styling via Tailwind utility classes (already set up with `@tailwindcss/vite`). Use Tailwind's arbitrary value syntax for the Ember-specific colors (e.g., `bg-[#0c0a09]`, `border-[rgba(245,158,11,0.06)]`). For gradients on badges and sliders, use inline styles or small CSS additions in `index.css`.

Add a small set of custom CSS in `index.css` for:
- Custom range input styling (slider thumb and track gradients)
- Glow animations for the LIVE dot and status indicators
- Smooth tab transition animations
