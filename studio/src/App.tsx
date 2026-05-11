import { useState, useEffect, useCallback } from 'react'
import { useEffects } from './hooks/useEffects'
import { usePipeline } from './hooks/usePipeline'
import { usePresets } from './hooks/usePresets'
import { EffectLibrary } from './components/EffectLibrary'
import { VideoPlayer } from './components/VideoPlayer'
import { PipelineEditor } from './components/PipelineEditor'
import { ParamPanel } from './components/ParamPanel'
import { PresetBar } from './components/PresetBar'

function App() {
  const effects = useEffects()
  const { slots, addEffect, removeSlot, updateParam, setEnabled, reorder, refresh } = usePipeline()
  const { presets, savePreset, applyPreset, deletePreset } = usePresets()
  const [selectedSlotId, setSelectedSlotId] = useState<string | null>(null)
  const [sourceUrl, setSourceUrl] = useState('')

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

  const handleApplyPreset = async (id: string) => {
    await applyPreset(id)
    refresh()
    setSelectedSlotId(null)
  }

  return (
    <div className="h-screen flex flex-col bg-gray-950 text-white overflow-hidden">
      <PresetBar
        presets={presets}
        onSave={savePreset}
        onApply={handleApplyPreset}
        onDelete={deletePreset}
        sourceUrl={sourceUrl}
        onSourceUrlChange={handleSourceUrlChange}
      />

      <div className="flex flex-1 min-h-0">
        {/* Left: Effect Library */}
        <div className="w-[200px] bg-gray-900/50 border-r border-gray-800 overflow-y-auto">
          <EffectLibrary effects={effects} onAdd={addEffect} />
        </div>

        {/* Center: Video Player */}
        <VideoPlayer />

        {/* Right: Pipeline + Params */}
        <div className="w-[280px] bg-gray-900/50 border-l border-gray-800 flex flex-col">
          <div className="border-b border-gray-800 overflow-y-auto max-h-[50%]">
            <PipelineEditor
              slots={slots}
              effects={effects}
              selectedSlotId={selectedSlotId}
              onSelect={setSelectedSlotId}
              onToggle={setEnabled}
              onReorder={reorder}
            />
          </div>
          <div className="flex-1 overflow-y-auto">
            <ParamPanel
              slot={selectedSlot}
              effects={effects}
              onUpdateParam={updateParam}
              onRemove={(id) => {
                removeSlot(id)
                if (selectedSlotId === id) setSelectedSlotId(null)
              }}
            />
          </div>
        </div>
      </div>
    </div>
  )
}

export default App
