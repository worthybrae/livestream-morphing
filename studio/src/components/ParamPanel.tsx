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
    <div className="p-4" style={{ borderTop: '1px solid rgba(245,158,11,0.06)' }} onClick={(e) => e.stopPropagation()}>
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
