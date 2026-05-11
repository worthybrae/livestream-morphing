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
      style={style}
      className={`flex items-center gap-2 mb-1 px-2 py-1.5 rounded text-sm cursor-pointer transition-colors ${
        isSelected ? 'bg-indigo-900/50 border-l-[3px] border-indigo-500' : 'bg-gray-800 border-l-[3px] border-green-400'
      } ${!slot.enabled ? 'opacity-50' : ''}`}
      onClick={onSelect}
    >
      <span
        {...attributes}
        {...listeners}
        className="text-gray-500 cursor-grab text-[10px] select-none"
      >
        ⠿
      </span>
      <span className={`flex-1 ${!slot.enabled ? 'line-through text-gray-500' : 'text-gray-200'}`}>
        {index + 1}. {effectName}
      </span>
      <button
        onClick={(e) => { e.stopPropagation(); onToggle() }}
        className={`text-[10px] ${slot.enabled ? 'text-green-400' : 'text-red-400'}`}
      >
        ●
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

  return (
    <div className="p-3">
      <div className="text-xs uppercase tracking-wider text-gray-400 mb-3">
        Pipeline Order
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
        <div className="text-sm text-gray-500 italic">No effects — add from library</div>
      )}
      <div className="text-[10px] text-gray-500 mt-2">
        drag to reorder · click dot to toggle
      </div>
    </div>
  )
}
