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
