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
