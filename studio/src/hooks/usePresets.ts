import { useState, useEffect, useCallback } from 'react'
import type { PresetSummary } from '../types'

export function usePresets() {
  const [presets, setPresets] = useState<PresetSummary[]>([])

  const refresh = useCallback(() => {
    fetch('/api/presets')
      .then((r) => r.json())
      .then(setPresets)
      .catch(console.error)
  }, [])

  useEffect(() => { refresh() }, [refresh])

  const savePreset = useCallback(async (name: string) => {
    await fetch('/api/presets', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name }),
    })
    refresh()
  }, [refresh])

  const applyPreset = useCallback(async (id: string) => {
    const res = await fetch(`/api/presets/${id}/apply`, { method: 'PUT' })
    return res.json()
  }, [])

  const deletePreset = useCallback(async (id: string) => {
    await fetch(`/api/presets/${id}`, { method: 'DELETE' })
    refresh()
  }, [refresh])

  return { presets, savePreset, applyPreset, deletePreset, refresh }
}
