import { useState, useEffect, useCallback, useRef } from 'react'
import type { PipelineSlot, PipelineEntry } from '../types'

export function usePipeline() {
  const [slots, setSlots] = useState<PipelineSlot[]>([])
  const debounceTimers = useRef<Record<string, ReturnType<typeof setTimeout>>>({})

  const refresh = useCallback(() => {
    fetch('/api/pipeline')
      .then((r) => r.json())
      .then(setSlots)
      .catch(console.error)
  }, [])

  useEffect(() => { refresh() }, [refresh])

  const addEffect = useCallback(async (effectId: string) => {
    const res = await fetch(`/api/pipeline/add/${effectId}`, { method: 'POST' })
    const data = await res.json()
    setSlots(data)
  }, [])

  const removeSlot = useCallback(async (slotId: string) => {
    const res = await fetch(`/api/pipeline/${slotId}`, { method: 'DELETE' })
    const data = await res.json()
    setSlots(data)
  }, [])

  const updateParam = useCallback((slotId: string, paramId: string, value: number) => {
    setSlots((prev) =>
      prev.map((s) =>
        s.slot_id === slotId
          ? { ...s, params: { ...s.params, [paramId]: value } }
          : s
      )
    )

    const key = `${slotId}:${paramId}`
    if (debounceTimers.current[key]) {
      clearTimeout(debounceTimers.current[key])
    }
    debounceTimers.current[key] = setTimeout(async () => {
      await fetch(`/api/pipeline/${slotId}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ params: { [paramId]: value } }),
      })
      delete debounceTimers.current[key]
    }, 100)
  }, [])

  const setEnabled = useCallback(async (slotId: string, enabled: boolean) => {
    setSlots((prev) =>
      prev.map((s) => (s.slot_id === slotId ? { ...s, enabled } : s))
    )
    const res = await fetch(`/api/pipeline/${slotId}`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled }),
    })
    const data = await res.json()
    setSlots(data)
  }, [])

  const reorder = useCallback(async (newSlots: PipelineSlot[]) => {
    setSlots(newSlots)
    const entries: PipelineEntry[] = newSlots.map((s) => ({
      effect_id: s.effect_id,
      params: s.params,
      enabled: s.enabled,
    }))
    const res = await fetch('/api/pipeline', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(entries),
    })
    const data = await res.json()
    setSlots(data)
  }, [])

  return { slots, addEffect, removeSlot, updateParam, setEnabled, reorder, refresh }
}
