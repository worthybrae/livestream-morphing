import type { ParamDef } from '../types'

interface SliderProps {
  param: ParamDef
  value: number
  onChange: (value: number) => void
}

export function Slider({ param, value, onChange }: SliderProps) {
  const pct = ((value - param.min) / (param.max - param.min)) * 100

  return (
    <div className="mb-4">
      <div className="flex justify-between mb-1.5">
        <span className="text-[11px] text-[#a8a29e]">{param.name}</span>
        <span
          className="text-[11px] font-semibold text-[#f59e0b]"
          style={{ fontVariantNumeric: 'tabular-nums' }}
        >
          {param.step >= 1 ? Math.round(value) : value.toFixed(2)}
        </span>
      </div>
      <input
        type="range"
        min={param.min}
        max={param.max}
        step={param.step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className="w-full ember-slider"
        style={{
          background: `linear-gradient(90deg, #f59e0b 0%, #f97316 ${pct}%, rgba(255,255,255,0.06) ${pct}%)`,
          height: '3px',
          borderRadius: '2px',
        }}
      />
      <div className="flex justify-between mt-1">
        <span className="text-[9px] text-[#44403c]" style={{ fontVariantNumeric: 'tabular-nums' }}>
          {param.min}
        </span>
        <span className="text-[9px] text-[#44403c]" style={{ fontVariantNumeric: 'tabular-nums' }}>
          {param.max}
        </span>
      </div>
    </div>
  )
}
