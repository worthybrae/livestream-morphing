# Morph Studio UI Redesign — Ember Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the Morph Studio frontend from a 3-column layout with flat toolbar into a focused 2-column layout with tabbed right panel, applying the warm "Ember" visual style.

**Architecture:** Replace the current PresetBar + 3-column layout with a Header (logo + tabs + URL + randomize) and a single right panel that swaps content between Effects, Pipeline, and Presets tabs. All hooks and API contracts are unchanged — this is purely a component restructure and restyle.

**Tech Stack:** React 19, TypeScript, Tailwind CSS v4 (with `@tailwindcss/vite`), dnd-kit for drag-and-drop, hls.js for video.

**Spec:** `docs/superpowers/specs/2026-05-11-ui-redesign-design.md`

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `studio/src/index.css` | Custom slider styles, glow animations, Ember CSS variables |
| Create | `studio/src/components/Header.tsx` | Logo, pill tab switcher, stream URL input, randomize button |
| Create | `studio/src/components/PresetList.tsx` | Presets tab — save flow, preset list, apply/delete |
| Modify | `studio/src/App.tsx` | 2-column layout, activeTab state, wire up Header + TabPanel logic |
| Modify | `studio/src/components/EffectLibrary.tsx` | Restyle to Ember palette for right-panel context |
| Modify | `studio/src/components/Slider.tsx` | Gradient track fill, accent value display |
| Modify | `studio/src/components/PipelineEditor.tsx` | Gradient number badges, Ember colors, effect count |
| Modify | `studio/src/components/ParamPanel.tsx` | Ember restyle, integrated below pipeline |
| Modify | `studio/src/components/StatusBar.tsx` | Ember palette, dot separators, glow effects |
| Modify | `studio/src/components/VideoPlayer.tsx` | LIVE badge with glow dot |
| Delete | `studio/src/components/PresetBar.tsx` | Replaced by Header.tsx + PresetList.tsx |

---

### Task 1: CSS Foundation — Ember Theme & Custom Styles

**Files:**
- Modify: `studio/src/index.css`

This task lays the CSS groundwork that all subsequent components depend on: custom range input styling (the browser default is unusable for our design), glow animations, and transition utilities.

- [ ] **Step 1: Write the custom CSS**

Replace the contents of `studio/src/index.css` with:

```css
@import "tailwindcss";

/* === Ember Glow Animations === */
@keyframes glow-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

@keyframes live-pulse {
  0%, 100% { box-shadow: 0 0 4px rgba(239, 68, 68, 0.6); }
  50% { box-shadow: 0 0 10px rgba(239, 68, 68, 0.9); }
}

.animate-glow-pulse {
  animation: glow-pulse 2s ease-in-out infinite;
}

.animate-live-pulse {
  animation: live-pulse 2s ease-in-out infinite;
}

/* === Custom Range Slider === */
input[type="range"].ember-slider {
  -webkit-appearance: none;
  appearance: none;
  background: transparent;
  cursor: pointer;
  height: 20px;
}

input[type="range"].ember-slider::-webkit-slider-runnable-track {
  height: 3px;
  border-radius: 2px;
  background: rgba(255, 255, 255, 0.06);
}

input[type="range"].ember-slider::-moz-range-track {
  height: 3px;
  border-radius: 2px;
  background: rgba(255, 255, 255, 0.06);
}

input[type="range"].ember-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: linear-gradient(135deg, #f59e0b, #f97316);
  margin-top: -4.5px;
  box-shadow: 0 0 6px rgba(245, 158, 11, 0.3);
  transition: box-shadow 0.15s ease;
}

input[type="range"].ember-slider::-moz-range-thumb {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: linear-gradient(135deg, #f59e0b, #f97316);
  border: none;
  box-shadow: 0 0 6px rgba(245, 158, 11, 0.3);
  transition: box-shadow 0.15s ease;
}

input[type="range"].ember-slider:hover::-webkit-slider-thumb {
  box-shadow: 0 0 10px rgba(245, 158, 11, 0.5);
}

input[type="range"].ember-slider:hover::-moz-range-thumb {
  box-shadow: 0 0 10px rgba(245, 158, 11, 0.5);
}

/* Gradient fill trick: use a linear-gradient on the track and clip via range progress */
input[type="range"].ember-slider::-moz-range-progress {
  height: 3px;
  border-radius: 2px;
  background: linear-gradient(90deg, #f59e0b, #f97316);
}
```

- [ ] **Step 2: Verify build compiles**

Run: `cd studio && npx vite build 2>&1 | tail -5`
Expected: Build succeeds with no errors.

- [ ] **Step 3: Commit**

```bash
git add studio/src/index.css
git commit -m "style: add Ember theme CSS — slider styles, glow animations"
```

---

### Task 2: Header Component

**Files:**
- Create: `studio/src/components/Header.tsx`

The Header replaces the old PresetBar. It contains the logo, centered pill tab switcher, stream URL input, and randomize button. It receives `activeTab` and `onTabChange` as props — App.tsx owns the state.

- [ ] **Step 1: Create Header.tsx**

```tsx
import { useState, useEffect } from 'react'

export type Tab = 'effects' | 'pipeline' | 'presets'

interface HeaderProps {
  activeTab: Tab
  onTabChange: (tab: Tab) => void
  sourceUrl: string
  onSourceUrlChange: (url: string) => void
  onRandomize: () => void
}

const tabs: { id: Tab; label: string }[] = [
  { id: 'effects', label: 'Effects' },
  { id: 'pipeline', label: 'Pipeline' },
  { id: 'presets', label: 'Presets' },
]

export function Header({ activeTab, onTabChange, sourceUrl, onSourceUrlChange, onRandomize }: HeaderProps) {
  const [localUrl, setLocalUrl] = useState(sourceUrl)

  useEffect(() => {
    setLocalUrl(sourceUrl)
  }, [sourceUrl])

  const handleUrlSubmit = () => {
    if (localUrl.trim() && localUrl !== sourceUrl) {
      onSourceUrlChange(localUrl.trim())
    }
  }

  return (
    <div
      className="flex items-center px-5 py-3"
      style={{
        background: 'linear-gradient(180deg, rgba(245,158,11,0.03) 0%, transparent 100%)',
        borderBottom: '1px solid rgba(245,158,11,0.06)',
      }}
    >
      {/* Logo */}
      <div className="flex items-center gap-2">
        <div
          className="w-6 h-6 rounded-md flex items-center justify-center text-[10px] font-bold text-white"
          style={{ background: 'linear-gradient(135deg, #f59e0b, #f97316)' }}
        >
          M
        </div>
        <span className="text-[#fafaf9] font-semibold text-[13px] tracking-tight">
          Morph Studio
        </span>
      </div>

      {/* Centered pill tab switcher */}
      <div className="flex-1 flex justify-center">
        <div className="flex rounded-lg p-[3px] gap-[2px]" style={{ background: 'rgba(255,255,255,0.04)' }}>
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => onTabChange(tab.id)}
              className={`px-4 py-[5px] text-[11px] font-medium rounded-md transition-all duration-150 ${
                activeTab === tab.id
                  ? 'text-[#fafaf9] shadow-sm'
                  : 'text-[#78716c] hover:text-[#a8a29e]'
              }`}
              style={
                activeTab === tab.id
                  ? { background: 'rgba(255,255,255,0.08)', boxShadow: '0 1px 2px rgba(0,0,0,0.3)' }
                  : undefined
              }
            >
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      {/* Right: URL input + Randomize */}
      <div className="flex items-center gap-3">
        <input
          value={localUrl}
          onChange={(e) => setLocalUrl(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && handleUrlSubmit()}
          onBlur={handleUrlSubmit}
          placeholder="Stream URL..."
          className="px-3 py-[5px] text-[11px] rounded-md w-60 outline-none transition-colors"
          style={{
            background: '#0a0a09',
            border: '1px solid rgba(255,255,255,0.06)',
            color: '#a8a29e',
          }}
          onFocus={(e) => (e.target.style.borderColor = 'rgba(245,158,11,0.2)')}
          onBlurCapture={(e) => (e.target.style.borderColor = 'rgba(255,255,255,0.06)')}
        />
        <button
          onClick={onRandomize}
          className="px-3 py-[5px] text-[11px] font-medium rounded-md transition-all duration-150 text-[#a8a29e] hover:text-[#e7e5e4]"
          style={{
            background: 'rgba(255,255,255,0.04)',
            border: '1px solid rgba(255,255,255,0.06)',
          }}
        >
          Randomize
        </button>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Verify build compiles**

Run: `cd studio && npx vite build 2>&1 | tail -5`
Expected: Build succeeds (Header isn't imported yet, but should have no syntax errors).

- [ ] **Step 3: Commit**

```bash
git add studio/src/components/Header.tsx
git commit -m "feat: add Header component with pill tab switcher"
```

---

### Task 3: PresetList Component

**Files:**
- Create: `studio/src/components/PresetList.tsx`

The Presets tab content — replaces the preset buttons that were in PresetBar. Clean list with save flow, apply, delete.

- [ ] **Step 1: Create PresetList.tsx**

```tsx
import { useState } from 'react'
import type { PresetSummary } from '../types'

interface PresetListProps {
  presets: PresetSummary[]
  onSave: (name: string) => void
  onApply: (id: string) => void
  onDelete: (id: string) => void
}

export function PresetList({ presets, onSave, onApply, onDelete }: PresetListProps) {
  const [showSave, setShowSave] = useState(false)
  const [name, setName] = useState('')

  const handleSave = () => {
    if (name.trim()) {
      onSave(name.trim())
      setName('')
      setShowSave(false)
    }
  }

  return (
    <div className="p-4 flex flex-col gap-3">
      {/* Section label */}
      <div className="text-[10px] uppercase tracking-[1.5px] text-[#57534e] font-semibold">
        Presets
      </div>

      {/* Actions row */}
      <div className="flex gap-2">
        {showSave ? (
          <div className="flex gap-2 flex-1">
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSave()}
              placeholder="Preset name..."
              className="flex-1 px-3 py-[6px] text-[11px] rounded-md outline-none"
              style={{
                background: '#0a0a09',
                border: '1px solid rgba(245,158,11,0.15)',
                color: '#e7e5e4',
              }}
              autoFocus
            />
            <button
              onClick={handleSave}
              className="px-3 py-[6px] text-[11px] font-medium text-white rounded-md transition-opacity hover:opacity-90"
              style={{ background: 'linear-gradient(135deg, #f59e0b, #f97316)' }}
            >
              Save
            </button>
            <button
              onClick={() => { setShowSave(false); setName('') }}
              className="text-[11px] text-[#57534e] hover:text-[#a8a29e] transition-colors px-1"
            >
              Cancel
            </button>
          </div>
        ) : (
          <button
            onClick={() => setShowSave(true)}
            className="px-3 py-[6px] text-[11px] font-medium text-white rounded-md transition-opacity hover:opacity-90"
            style={{ background: 'linear-gradient(135deg, #f59e0b, #f97316)' }}
          >
            + Save Current
          </button>
        )}
      </div>

      {/* Preset list */}
      <div className="flex flex-col gap-1">
        {presets.map((preset, i) => (
          <div
            key={preset.id}
            onClick={() => onApply(preset.id)}
            className="flex items-center gap-3 px-3 py-[10px] rounded-lg cursor-pointer transition-all duration-150 group"
            style={{
              background: i === 0 ? 'rgba(245,158,11,0.06)' : 'rgba(255,255,255,0.02)',
              border: `1px solid ${i === 0 ? 'rgba(245,158,11,0.12)' : 'rgba(255,255,255,0.04)'}`,
            }}
            onMouseEnter={(e) => {
              if (i !== 0) e.currentTarget.style.background = 'rgba(245,158,11,0.04)'
            }}
            onMouseLeave={(e) => {
              if (i !== 0) e.currentTarget.style.background = 'rgba(255,255,255,0.02)'
            }}
          >
            {/* Icon */}
            <div
              className="w-7 h-7 rounded-[7px] flex items-center justify-center text-[12px] shrink-0"
              style={{
                background: i === 0
                  ? 'linear-gradient(135deg, #f59e0b, #f97316)'
                  : 'rgba(255,255,255,0.06)',
              }}
            >
              {i === 0 ? '🎨' : '✦'}
            </div>
            {/* Name + meta */}
            <div className="flex-1 min-w-0">
              <div className={`text-[12px] font-medium truncate ${i === 0 ? 'text-[#e7e5e4]' : 'text-[#a8a29e]'}`}>
                {preset.name}
              </div>
            </div>
            {/* Delete */}
            <button
              onClick={(e) => { e.stopPropagation(); onDelete(preset.id) }}
              className="text-[14px] text-[#44403c] hover:text-red-400 transition-colors opacity-0 group-hover:opacity-100"
            >
              ×
            </button>
          </div>
        ))}
      </div>

      {presets.length === 0 && (
        <div className="text-[11px] text-[#57534e] italic text-center py-4">
          No presets saved yet
        </div>
      )}

      {presets.length > 0 && (
        <div className="text-[9px] text-[#44403c] text-center mt-1">
          click to apply · × to delete
        </div>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Verify build compiles**

Run: `cd studio && npx vite build 2>&1 | tail -5`
Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add studio/src/components/PresetList.tsx
git commit -m "feat: add PresetList component for Presets tab"
```

---

### Task 4: Restyle EffectLibrary

**Files:**
- Modify: `studio/src/components/EffectLibrary.tsx`

Restyle to Ember palette. The layout is the same (scrollable list, click to add) but visuals shift to warm theme.

- [ ] **Step 1: Rewrite EffectLibrary.tsx**

Replace the entire file with:

```tsx
import type { EffectDef } from '../types'

interface EffectLibraryProps {
  effects: EffectDef[]
  onAdd: (effectId: string) => void
}

export function EffectLibrary({ effects, onAdd }: EffectLibraryProps) {
  return (
    <div className="p-4 flex flex-col gap-3">
      <div className="text-[10px] uppercase tracking-[1.5px] text-[#57534e] font-semibold">
        Effects
      </div>
      <div className="flex flex-col gap-1">
        {effects.map((effect) => (
          <button
            key={effect.id}
            onClick={() => onAdd(effect.id)}
            className="w-full text-left px-3 py-[9px] text-[11px] text-[#a8a29e] rounded-lg transition-all duration-150"
            style={{
              background: 'rgba(255,255,255,0.02)',
              border: '1px solid rgba(255,255,255,0.04)',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'rgba(245,158,11,0.04)'
              e.currentTarget.style.borderColor = 'rgba(245,158,11,0.1)'
              e.currentTarget.style.color = '#e7e5e4'
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'rgba(255,255,255,0.02)'
              e.currentTarget.style.borderColor = 'rgba(255,255,255,0.04)'
              e.currentTarget.style.color = '#a8a29e'
            }}
          >
            {effect.name}
          </button>
        ))}
      </div>
      <div className="text-[9px] text-[#44403c] text-center mt-1">
        click to add to pipeline
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Verify build compiles**

Run: `cd studio && npx vite build 2>&1 | tail -5`
Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add studio/src/components/EffectLibrary.tsx
git commit -m "style: restyle EffectLibrary to Ember theme"
```

---

### Task 5: Restyle Slider

**Files:**
- Modify: `studio/src/components/Slider.tsx`

Custom gradient-fill slider with accent value display. Uses the `.ember-slider` CSS class from Task 1 for the range input. Adds an inline style for the WebKit gradient fill trick (since `::-webkit-slider-runnable-track` can't use `var()` for progress).

- [ ] **Step 1: Rewrite Slider.tsx**

Replace the entire file with:

```tsx
import type { ParamDef } from '../types'

interface SliderProps {
  param: ParamDef
  value: number
  onChange: (value: number) => void
}

export function Slider({ param, value, onChange }: SliderProps) {
  const pct = ((value - param.min) / (param.max - param.min)) * 100

  return (
    <div className="mb-4">
      <div className="flex justify-between mb-1.5">
        <span className="text-[11px] text-[#a8a29e]">{param.name}</span>
        <span
          className="text-[11px] font-semibold text-[#f59e0b]"
          style={{ fontVariantNumeric: 'tabular-nums' }}
        >
          {param.step >= 1 ? Math.round(value) : value.toFixed(2)}
        </span>
      </div>
      <input
        type="range"
        min={param.min}
        max={param.max}
        step={param.step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className="w-full ember-slider"
        style={{
          background: `linear-gradient(90deg, #f59e0b 0%, #f97316 ${pct}%, rgba(255,255,255,0.06) ${pct}%)`,
          height: '3px',
          borderRadius: '2px',
        }}
      />
      <div className="flex justify-between mt-1">
        <span className="text-[9px] text-[#44403c]" style={{ fontVariantNumeric: 'tabular-nums' }}>
          {param.min}
        </span>
        <span className="text-[9px] text-[#44403c]" style={{ fontVariantNumeric: 'tabular-nums' }}>
          {param.max}
        </span>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Verify build compiles**

Run: `cd studio && npx vite build 2>&1 | tail -5`
Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add studio/src/components/Slider.tsx
git commit -m "style: restyle Slider with Ember gradient track"
```

---

### Task 6: Restyle PipelineEditor

**Files:**
- Modify: `studio/src/components/PipelineEditor.tsx`

Gradient number badges, Ember colors, effect count in header. The dnd-kit drag logic is unchanged — only the visual rendering changes.

- [ ] **Step 1: Rewrite PipelineEditor.tsx**

Replace the entire file with:

```tsx
import { DndContext, closestCenter, type DragEndEvent } from '@dnd-kit/core'
import { SortableContext, verticalListSortingStrategy, useSortable, arrayMove } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import type { PipelineSlot, EffectDef } from '../types'

interface PipelineEditorProps {
  slots: PipelineSlot[]
  effects: EffectDef[]
  selectedSlotId: string | null
  onSelect: (slotId: string) => void
  onToggle: (slotId: string, enabled: boolean) => void
  onReorder: (newSlots: PipelineSlot[]) => void
}

function SortableSlot({
  slot,
  effects,
  index,
  isSelected,
  onSelect,
  onToggle,
}: {
  slot: PipelineSlot
  effects: EffectDef[]
  index: number
  isSelected: boolean
  onSelect: () => void
  onToggle: () => void
}) {
  const { attributes, listeners, setNodeRef, transform, transition } = useSortable({
    id: slot.slot_id,
  })

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  }

  const effectName = effects.find((e) => e.id === slot.effect_id)?.name ?? slot.effect_id

  return (
    <div
      ref={setNodeRef}
      style={{
        ...style,
        background: isSelected ? 'rgba(245,158,11,0.06)' : 'rgba(255,255,255,0.02)',
        border: `1px solid ${isSelected ? 'rgba(245,158,11,0.12)' : 'rgba(255,255,255,0.04)'}`,
      }}
      className={`flex items-center gap-2 mb-1 px-2 py-[7px] rounded-lg text-[11px] cursor-pointer transition-all duration-150 ${
        !slot.enabled ? 'opacity-50' : ''
      }`}
      onClick={onSelect}
    >
      <span
        {...attributes}
        {...listeners}
        className="text-[#44403c] cursor-grab text-[10px] select-none"
      >
        ⠿
      </span>
      {/* Number badge */}
      <span
        className="w-[18px] h-[18px] rounded-[5px] flex items-center justify-center text-[9px] font-semibold text-white shrink-0"
        style={{
          background: slot.enabled
            ? 'linear-gradient(135deg, #f59e0b, #f97316)'
            : 'rgba(255,255,255,0.06)',
          color: slot.enabled ? 'white' : '#78716c',
        }}
      >
        {index + 1}
      </span>
      <span className={`flex-1 ${!slot.enabled ? 'line-through text-[#57534e]' : 'text-[#a8a29e]'} ${isSelected ? 'text-[#e7e5e4]' : ''}`}>
        {effectName}
      </span>
      {/* Toggle dot */}
      <button
        onClick={(e) => { e.stopPropagation(); onToggle() }}
        className="transition-all duration-150"
      >
        <div
          className="w-[5px] h-[5px] rounded-full"
          style={{
            background: slot.enabled ? '#22c55e' : '#ef4444',
            boxShadow: slot.enabled
              ? '0 0 6px rgba(34,197,94,0.4)'
              : '0 0 6px rgba(239,68,68,0.4)',
          }}
        />
      </button>
    </div>
  )
}

export function PipelineEditor({
  slots,
  effects,
  selectedSlotId,
  onSelect,
  onToggle,
  onReorder,
}: PipelineEditorProps) {
  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event
    if (!over || active.id === over.id) return

    const oldIndex = slots.findIndex((s) => s.slot_id === active.id)
    const newIndex = slots.findIndex((s) => s.slot_id === over.id)
    const newSlots = arrayMove(slots, oldIndex, newIndex)
    onReorder(newSlots)
  }

  const activeCount = slots.filter((s) => s.enabled).length

  return (
    <div className="p-4">
      <div className="flex justify-between items-center mb-3">
        <span className="text-[10px] uppercase tracking-[1.5px] text-[#57534e] font-semibold">
          Pipeline
        </span>
        <span className="text-[10px] text-[#44403c]">
          {activeCount} active
        </span>
      </div>
      <DndContext collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={slots.map((s) => s.slot_id)} strategy={verticalListSortingStrategy}>
          {slots.map((slot, i) => (
            <SortableSlot
              key={slot.slot_id}
              slot={slot}
              effects={effects}
              index={i}
              isSelected={slot.slot_id === selectedSlotId}
              onSelect={() => onSelect(slot.slot_id)}
              onToggle={() => onToggle(slot.slot_id, !slot.enabled)}
            />
          ))}
        </SortableContext>
      </DndContext>
      {slots.length === 0 && (
        <div className="text-[11px] text-[#57534e] italic">
          No effects — add from the Effects tab
        </div>
      )}
    </div>
  )
}
```

- [ ] **Step 2: Verify build compiles**

Run: `cd studio && npx vite build 2>&1 | tail -5`
Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add studio/src/components/PipelineEditor.tsx
git commit -m "style: restyle PipelineEditor with Ember gradient badges"
```

---

### Task 7: Restyle ParamPanel

**Files:**
- Modify: `studio/src/components/ParamPanel.tsx`

Ember palette, uses the restyled Slider from Task 5. Subtle red-tinted remove button.

- [ ] **Step 1: Rewrite ParamPanel.tsx**

Replace the entire file with:

```tsx
import { Slider } from './Slider'
import type { PipelineSlot, EffectDef } from '../types'

interface ParamPanelProps {
  slot: PipelineSlot | null
  effects: EffectDef[]
  onUpdateParam: (slotId: string, paramId: string, value: number) => void
  onRemove: (slotId: string) => void
}

export function ParamPanel({ slot, effects, onUpdateParam, onRemove }: ParamPanelProps) {
  if (!slot) {
    return (
      <div className="p-4 text-[11px] text-[#57534e] italic">
        Select an effect to edit parameters
      </div>
    )
  }

  const effectDef = effects.find((e) => e.id === slot.effect_id)
  if (!effectDef) return null

  return (
    <div className="p-4" style={{ borderTop: '1px solid rgba(245,158,11,0.06)' }}>
      <div className="text-[10px] uppercase tracking-[1.5px] text-[#57534e] font-semibold mb-3">
        {effectDef.name} — Parameters
      </div>
      {effectDef.params.map((param) => (
        <Slider
          key={param.id}
          param={param}
          value={slot.params[param.id] ?? param.default}
          onChange={(v) => onUpdateParam(slot.slot_id, param.id, v)}
        />
      ))}
      <div className="mt-4 pt-3" style={{ borderTop: '1px solid rgba(255,255,255,0.04)' }}>
        <button
          onClick={() => onRemove(slot.slot_id)}
          className="w-full py-2 text-[11px] text-red-300/70 rounded-lg transition-all duration-150 hover:text-red-300"
          style={{
            background: 'rgba(239,68,68,0.06)',
            border: '1px solid rgba(239,68,68,0.1)',
          }}
        >
          Remove from Pipeline
        </button>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Verify build compiles**

Run: `cd studio && npx vite build 2>&1 | tail -5`
Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add studio/src/components/ParamPanel.tsx
git commit -m "style: restyle ParamPanel to Ember theme"
```

---

### Task 8: Restyle StatusBar

**Files:**
- Modify: `studio/src/components/StatusBar.tsx`

Ember palette, dot separators (not pipes), glow effects on status dot, tabular-nums on all numbers.

- [ ] **Step 1: Rewrite StatusBar.tsx**

Replace the entire file with:

```tsx
import { useState, useEffect } from 'react'

interface Stats {
  effects_ms: number
  total_ms: number
  frames: number
  segment_completed_at: number
}

export function StatusBar() {
  const [stats, setStats] = useState<Stats | null>(null)
  const [countdown, setCountdown] = useState<number | null>(null)

  useEffect(() => {
    const poll = () => {
      fetch('/api/status')
        .then((r) => r.json())
        .then(setStats)
        .catch(() => {})
    }
    poll()
    const id = setInterval(poll, 2000)
    return () => clearInterval(id)
  }, [])

  useEffect(() => {
    if (!stats || stats.segment_completed_at === 0) return
    const tick = () => {
      const elapsed = (Date.now() - stats.segment_completed_at) / 1000
      const remaining = Math.max(0, 6 - elapsed)
      setCountdown(Math.ceil(remaining))
    }
    tick()
    const id = setInterval(tick, 200)
    return () => clearInterval(id)
  }, [stats])

  if (!stats || stats.segment_completed_at === 0) {
    return (
      <div
        className="px-5 py-2 flex items-center gap-4 text-[10px] text-[#44403c]"
        style={{ borderTop: '1px solid rgba(245,158,11,0.06)' }}
      >
        Waiting for first segment...
      </div>
    )
  }

  const perfColor =
    stats.total_ms < 2000 ? '#22c55e' : stats.total_ms < 4000 ? '#eab308' : '#ef4444'

  const effectsPerFrame = stats.frames > 0 ? (stats.effects_ms / stats.frames).toFixed(1) : '—'

  const dotSep = (
    <span style={{ color: 'rgba(245,158,11,0.1)' }}>·</span>
  )

  return (
    <div
      className="px-5 py-2 flex items-center gap-4 text-[10px] text-[#44403c]"
      style={{ borderTop: '1px solid rgba(245,158,11,0.06)' }}
    >
      {/* Segment countdown */}
      <div className="flex items-center gap-1.5">
        <div
          className={`w-[5px] h-[5px] rounded-full ${countdown !== null && countdown <= 1 ? 'animate-glow-pulse' : ''}`}
          style={{
            background: '#22c55e',
            boxShadow: '0 0 6px rgba(34,197,94,0.4)',
          }}
        />
        <span>
          Next segment{' '}
          <span className="text-[#78716c]" style={{ fontVariantNumeric: 'tabular-nums' }}>
            {countdown ?? '—'}s
          </span>
        </span>
      </div>

      {dotSep}

      <span>
        Effects{' '}
        <span style={{ color: perfColor, fontVariantNumeric: 'tabular-nums' }}>
          {stats.effects_ms}ms
        </span>
      </span>

      {dotSep}

      <span>
        Total{' '}
        <span style={{ color: perfColor, fontVariantNumeric: 'tabular-nums' }}>
          {stats.total_ms}ms
        </span>
      </span>

      {dotSep}

      <span>
        Per frame{' '}
        <span className="text-[#78716c]" style={{ fontVariantNumeric: 'tabular-nums' }}>
          {effectsPerFrame}ms
        </span>
      </span>

      {dotSep}

      <span>
        <span className="text-[#78716c]" style={{ fontVariantNumeric: 'tabular-nums' }}>
          {stats.frames}
        </span>{' '}
        frames
      </span>
    </div>
  )
}
```

- [ ] **Step 2: Verify build compiles**

Run: `cd studio && npx vite build 2>&1 | tail -5`
Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add studio/src/components/StatusBar.tsx
git commit -m "style: restyle StatusBar with Ember theme and glow"
```

---

### Task 9: Restyle VideoPlayer

**Files:**
- Modify: `studio/src/components/VideoPlayer.tsx`

Only the LIVE badge changes — glow dot instead of solid rectangle. HLS logic untouched.

- [ ] **Step 1: Rewrite VideoPlayer.tsx**

Replace the entire file with:

```tsx
import { useEffect, useRef } from 'react'
import Hls from 'hls.js'

export function VideoPlayer() {
  const videoRef = useRef<HTMLVideoElement>(null)

  useEffect(() => {
    const video = videoRef.current
    if (!video) return

    const src = '/api/stream'

    if (Hls.isSupported()) {
      const hls = new Hls({
        liveSyncDurationCount: 2,
        liveMaxLatencyDurationCount: 4,
        enableWorker: true,
      })
      hls.loadSource(src)
      hls.attachMedia(video)
      hls.on(Hls.Events.MANIFEST_PARSED, () => {
        video.play().catch(() => {})
      })
      return () => hls.destroy()
    } else if (video.canPlayType('application/vnd.apple.mpegurl')) {
      video.src = src
      video.addEventListener('loadedmetadata', () => {
        video.play().catch(() => {})
      })
    }
  }, [])

  return (
    <div className="flex-1 bg-black flex items-center justify-center relative">
      <video
        ref={videoRef}
        className="w-full h-full object-contain"
        muted
        playsInline
      />
      <div className="absolute top-3 left-3.5 flex items-center gap-1.5">
        <div
          className="w-1.5 h-1.5 rounded-full animate-live-pulse"
          style={{ background: '#ef4444' }}
        />
        <span className="text-[10px] text-white/50 font-semibold tracking-wide">
          LIVE
        </span>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Verify build compiles**

Run: `cd studio && npx vite build 2>&1 | tail -5`
Expected: Build succeeds.

- [ ] **Step 3: Commit**

```bash
git add studio/src/components/VideoPlayer.tsx
git commit -m "style: restyle LIVE badge with glow dot"
```

---

### Task 10: Restructure App.tsx — Wire Everything Together

**Files:**
- Modify: `studio/src/App.tsx`

This is the core layout change: remove 3-column layout, add `activeTab` state, render Header + 2-column body (video + tab panel) + StatusBar. Import new components, remove PresetBar import.

- [ ] **Step 1: Rewrite App.tsx**

Replace the entire file with:

```tsx
import { useState, useEffect, useCallback } from 'react'
import { useEffects } from './hooks/useEffects'
import { usePipeline } from './hooks/usePipeline'
import { usePresets } from './hooks/usePresets'
import { Header, type Tab } from './components/Header'
import { EffectLibrary } from './components/EffectLibrary'
import { VideoPlayer } from './components/VideoPlayer'
import { PipelineEditor } from './components/PipelineEditor'
import { ParamPanel } from './components/ParamPanel'
import { PresetList } from './components/PresetList'
import { StatusBar } from './components/StatusBar'
import type { PipelineEntry } from './types'

function App() {
  const effects = useEffects()
  const { slots, addEffect, removeSlot, updateParam, setEnabled, reorder, refresh } = usePipeline()
  const { presets, savePreset, applyPreset, deletePreset } = usePresets()
  const [selectedSlotId, setSelectedSlotId] = useState<string | null>(null)
  const [sourceUrl, setSourceUrl] = useState('')
  const [activeTab, setActiveTab] = useState<Tab>('pipeline')

  useEffect(() => {
    fetch('/api/source')
      .then((r) => r.json())
      .then((data) => setSourceUrl(data.url))
      .catch(() => {})
  }, [])

  const handleSourceUrlChange = useCallback((url: string) => {
    setSourceUrl(url)
    fetch('/api/source', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ url }),
    }).catch(() => {})
  }, [])

  const selectedSlot = slots.find((s) => s.slot_id === selectedSlotId) ?? null

  const handleRandomize = useCallback(async () => {
    if (effects.length === 0) return

    const count = 4 + Math.floor(Math.random() * 5)
    const entries: PipelineEntry[] = []

    for (let i = 0; i < count; i++) {
      const effect = effects[Math.floor(Math.random() * effects.length)]
      const params: Record<string, number> = {}
      for (const p of effect.params) {
        const range = p.max - p.min
        const steps = Math.round(range / p.step)
        const randomSteps = Math.floor(Math.random() * (steps + 1))
        params[p.id] = p.min + randomSteps * p.step
      }
      entries.push({ effect_id: effect.id, params, enabled: true })
    }

    await fetch('/api/pipeline', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(entries),
    })
    refresh()
    setSelectedSlotId(null)
  }, [effects, refresh])

  const handleApplyPreset = async (id: string) => {
    await applyPreset(id)
    refresh()
    setSelectedSlotId(null)
  }

  const handleAddEffect = useCallback((effectId: string) => {
    addEffect(effectId)
    setActiveTab('pipeline')
  }, [addEffect])

  return (
    <div className="h-screen flex flex-col overflow-hidden" style={{ background: '#0c0a09', color: '#e7e5e4' }}>
      <Header
        activeTab={activeTab}
        onTabChange={setActiveTab}
        sourceUrl={sourceUrl}
        onSourceUrlChange={handleSourceUrlChange}
        onRandomize={handleRandomize}
      />

      <div className="flex flex-1 min-h-0">
        {/* Video Player */}
        <VideoPlayer />

        {/* Right Panel — tab content */}
        <div
          className="w-[280px] shrink-0 overflow-y-auto"
          style={{
            background: 'rgba(245,158,11,0.01)',
            borderLeft: '1px solid rgba(245,158,11,0.06)',
          }}
        >
          {activeTab === 'effects' && (
            <EffectLibrary effects={effects} onAdd={handleAddEffect} />
          )}

          {activeTab === 'pipeline' && (
            <>
              <PipelineEditor
                slots={slots}
                effects={effects}
                selectedSlotId={selectedSlotId}
                onSelect={setSelectedSlotId}
                onToggle={setEnabled}
                onReorder={reorder}
              />
              <ParamPanel
                slot={selectedSlot}
                effects={effects}
                onUpdateParam={updateParam}
                onRemove={(id) => {
                  removeSlot(id)
                  if (selectedSlotId === id) setSelectedSlotId(null)
                }}
              />
            </>
          )}

          {activeTab === 'presets' && (
            <PresetList
              presets={presets}
              onSave={savePreset}
              onApply={handleApplyPreset}
              onDelete={deletePreset}
            />
          )}
        </div>
      </div>

      <StatusBar />
    </div>
  )
}

export default App
```

- [ ] **Step 2: Verify build compiles**

Run: `cd studio && npx vite build 2>&1 | tail -5`
Expected: Build succeeds with no errors.

- [ ] **Step 3: Commit**

```bash
git add studio/src/App.tsx
git commit -m "feat: restructure to Focused Studio layout with tabbed right panel"
```

---

### Task 11: Delete PresetBar & Final Verification

**Files:**
- Delete: `studio/src/components/PresetBar.tsx`

- [ ] **Step 1: Delete PresetBar.tsx**

```bash
rm studio/src/components/PresetBar.tsx
```

- [ ] **Step 2: Verify build compiles cleanly**

Run: `cd studio && npx vite build 2>&1 | tail -10`
Expected: Build succeeds with no errors or warnings about missing imports.

- [ ] **Step 3: Verify dev server starts**

Run: `cd studio && npx vite --host 2>&1 | head -10`
Expected: Vite dev server starts on port 5173 with no errors.

- [ ] **Step 4: Commit**

```bash
git add -u studio/src/components/PresetBar.tsx
git commit -m "chore: remove PresetBar — replaced by Header + PresetList"
```
