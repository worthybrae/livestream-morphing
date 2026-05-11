import { useState } from 'react'
import type { PresetSummary } from '../types'

interface PresetBarProps {
  presets: PresetSummary[]
  onSave: (name: string) => void
  onApply: (id: string) => void
  onDelete: (id: string) => void
}

export function PresetBar({ presets, onSave, onApply, onDelete }: PresetBarProps) {
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
    <div className="bg-gray-900/80 border-b border-gray-800 px-4 py-2 flex items-center gap-3">
      <span className="text-amber-500 font-bold text-sm">Morph Studio</span>

      <div className="flex-1" />

      <div className="flex items-center gap-2">
        {presets.map((p) => (
          <div key={p.id} className="flex items-center gap-1">
            <button
              onClick={() => onApply(p.id)}
              className="px-3 py-1 text-xs bg-gray-800 text-gray-300 rounded hover:bg-gray-700 transition-colors"
            >
              {p.name}
            </button>
            <button
              onClick={() => onDelete(p.id)}
              className="text-gray-600 hover:text-red-400 text-xs transition-colors"
            >
              ×
            </button>
          </div>
        ))}
      </div>

      {showSave ? (
        <div className="flex items-center gap-2">
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSave()}
            placeholder="Preset name..."
            className="px-2 py-1 text-xs bg-gray-800 text-white rounded border border-gray-700 focus:border-indigo-500 outline-none w-36"
            autoFocus
          />
          <button
            onClick={handleSave}
            className="px-3 py-1 text-xs bg-indigo-600 text-white rounded hover:bg-indigo-500"
          >
            Save
          </button>
          <button
            onClick={() => setShowSave(false)}
            className="text-xs text-gray-500 hover:text-gray-300"
          >
            Cancel
          </button>
        </div>
      ) : (
        <button
          onClick={() => setShowSave(true)}
          className="px-3 py-1 text-xs bg-indigo-600 text-white rounded hover:bg-indigo-500 transition-colors"
        >
          Save Preset
        </button>
      )}
    </div>
  )
}
