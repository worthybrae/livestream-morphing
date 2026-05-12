import { useState, useEffect, useCallback, useRef } from 'react'
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
  const [toast, setToast] = useState<{ message: string; phase: 'enter' | 'exit' } | null>(null)
  const toastTimer = useRef<ReturnType<typeof setTimeout>>(null)

  const showToast = useCallback((message: string) => {
    if (toastTimer.current) clearTimeout(toastTimer.current)
    setToast({ message, phase: 'enter' })
  }, [])

  const dismissToast = useCallback(() => {
    setToast((prev) => prev ? { ...prev, phase: 'exit' } : null)
    toastTimer.current = setTimeout(() => setToast(null), 300)
  }, [])

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
    showToast('Randomizing pipeline...')

    const count = 3 + Math.floor(Math.random() * 3) // 3-5 effects
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
    setActiveTab('pipeline')
    dismissToast()
  }, [effects, refresh, showToast, dismissToast])

  const handleApplyPreset = async (id: string) => {
    const preset = presets.find((p) => p.id === id)
    showToast(`Applying "${preset?.name ?? 'preset'}"...`)
    await applyPreset(id)
    refresh()
    setSelectedSlotId(null)
    setActiveTab('pipeline')
    dismissToast()
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
          onClick={() => setSelectedSlotId(null)}
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
              {selectedSlot ? (
                <ParamPanel
                  slot={selectedSlot}
                  effects={effects}
                  onUpdateParam={updateParam}
                  onRemove={(id) => {
                    removeSlot(id)
                    if (selectedSlotId === id) setSelectedSlotId(null)
                  }}
                />
              ) : (
                <EffectLibrary effects={effects} onAdd={handleAddEffect} />
              )}
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

      {/* Toast notification */}
      {toast && (
        <div className="fixed inset-0 pointer-events-none flex items-start justify-center z-50" style={{ paddingTop: '72px' }}>
          <div
            className={`pointer-events-auto px-4 py-2.5 rounded-lg text-[12px] font-medium text-[#e7e5e4] ${
              toast.phase === 'enter' ? 'toast-enter' : 'toast-exit'
            }`}
            style={{
              background: 'rgba(12,10,9,0.9)',
              border: '1px solid rgba(245,158,11,0.15)',
              boxShadow: '0 4px 20px rgba(0,0,0,0.5), 0 0 15px rgba(245,158,11,0.05)',
              backdropFilter: 'blur(12px)',
            }}
          >
            <div className="flex items-center gap-2.5">
              <div
                className="w-1.5 h-1.5 rounded-full animate-glow-pulse"
                style={{ background: '#f59e0b', boxShadow: '0 0 6px rgba(245,158,11,0.5)' }}
              />
              {toast.message}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

export default App
