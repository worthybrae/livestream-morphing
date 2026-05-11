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
      <div className="p-3 text-sm text-gray-500 italic">
        Select an effect in the pipeline to edit its parameters
      </div>
    )
  }

  const effectDef = effects.find((e) => e.id === slot.effect_id)
  if (!effectDef) return null

  return (
    <div className="p-3">
      <div className="text-xs uppercase tracking-wider text-gray-400 mb-3">
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
      <div className="mt-5 pt-3 border-t border-gray-800">
        <button
          onClick={() => onRemove(slot.slot_id)}
          className="w-full py-2 text-sm text-red-300 bg-red-950 rounded hover:bg-red-900 transition-colors"
        >
          Remove from Pipeline
        </button>
      </div>
    </div>
  )
}
