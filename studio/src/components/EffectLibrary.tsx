import type { EffectDef } from '../types'

interface EffectLibraryProps {
  effects: EffectDef[]
  onAdd: (effectId: string) => void
}

export function EffectLibrary({ effects, onAdd }: EffectLibraryProps) {
  return (
    <div className="p-3">
      <div className="text-xs uppercase tracking-wider text-gray-400 mb-3">
        Effect Library
      </div>
      {effects.map((effect) => (
        <button
          key={effect.id}
          onClick={() => onAdd(effect.id)}
          className="w-full text-left px-3 py-2 mb-1 text-sm text-gray-200 bg-gray-800 rounded border-l-2 border-indigo-500 hover:bg-gray-700 transition-colors"
        >
          {effect.name}
        </button>
      ))}
      <div className="mt-2 text-[11px] text-gray-500 text-center">
        click to add to pipeline
      </div>
    </div>
  )
}
