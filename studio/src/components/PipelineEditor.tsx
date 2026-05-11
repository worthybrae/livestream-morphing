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
