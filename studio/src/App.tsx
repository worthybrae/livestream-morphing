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
