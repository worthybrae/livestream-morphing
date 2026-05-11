export interface ParamDef {
  id: string
  name: string
  min: number
  max: number
  default: number
  step: number
}

export interface EffectDef {
  id: string
  name: string
  params: ParamDef[]
}

export interface PipelineSlot {
  slot_id: string
  effect_id: string
  params: Record<string, number>
  enabled: boolean
}

export interface PipelineEntry {
  effect_id: string
  params: Record<string, number>
  enabled: boolean
}

export interface PresetSummary {
  id: string
  name: string
}
