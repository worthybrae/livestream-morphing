import { useState, useEffect } from 'react'
import type { EffectDef } from '../types'

export function useEffects() {
  const [effects, setEffects] = useState<EffectDef[]>([])

  useEffect(() => {
    fetch('/api/effects')
      .then((r) => r.json())
      .then(setEffects)
      .catch(console.error)
  }, [])

  return effects
}
