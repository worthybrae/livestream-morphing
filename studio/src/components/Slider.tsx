import type { ParamDef } from '../types'

interface SliderProps {
  param: ParamDef
  value: number
  onChange: (value: number) => void
}

export function Slider({ param, value, onChange }: SliderProps) {
  return (
    <div className="mb-4">
      <div className="flex justify-between mb-1">
        <span className="text-sm text-gray-300">{param.name}</span>
        <span className="text-sm font-bold text-indigo-400">
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
        className="w-full h-1 bg-gray-700 rounded-lg appearance-none cursor-pointer accent-indigo-500"
      />
      <div className="flex justify-between mt-0.5">
        <span className="text-[10px] text-gray-500">{param.min}</span>
        <span className="text-[10px] text-gray-500">{param.max}</span>
      </div>
    </div>
  )
}
