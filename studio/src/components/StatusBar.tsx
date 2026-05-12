import { useState, useEffect } from 'react'

interface SegmentInfo {
  id: string
  size_kb: number
}

interface Stats {
  effects_ms: number
  total_ms: number
  frames: number
  segment_completed_at: number
  segments: SegmentInfo[]
  buffer_count: number
  buffer_max: number
}

export function StatusBar() {
  const [stats, setStats] = useState<Stats | null>(null)
  const [countdown, setCountdown] = useState<number | null>(null)

  useEffect(() => {
    const poll = () => {
      fetch('/api/status')
        .then((r) => r.json())
        .then(setStats)
        .catch(() => {})
    }
    poll()
    const id = setInterval(poll, 2000)
    return () => clearInterval(id)
  }, [])

  useEffect(() => {
    if (!stats || stats.segment_completed_at === 0) return
    const tick = () => {
      const elapsed = (Date.now() - stats.segment_completed_at) / 1000
      const remaining = Math.max(0, 6 - elapsed)
      setCountdown(Math.ceil(remaining))
    }
    tick()
    const id = setInterval(tick, 200)
    return () => clearInterval(id)
  }, [stats])

  if (!stats || stats.segment_completed_at === 0) {
    return (
      <div
        className="px-5 py-2 flex items-center gap-4 text-[10px] text-[#44403c]"
        style={{ borderTop: '1px solid rgba(245,158,11,0.06)' }}
      >
        Waiting for first segment...
      </div>
    )
  }

  const perfColor =
    stats.total_ms < 2000 ? '#22c55e' : stats.total_ms < 4000 ? '#eab308' : '#ef4444'

  const effectsPerFrame = stats.frames > 0 ? (stats.effects_ms / stats.frames).toFixed(1) : '—'

  const dotSep = (
    <span style={{ color: 'rgba(245,158,11,0.1)' }}>·</span>
  )

  // The newest segment (last in array) is the one most recently processed
  const segments = stats.segments ?? []
  const bufferMax = stats.buffer_max ?? 10
  const bufferCount = stats.buffer_count ?? 0
  const newestIdx = segments.length - 1

  return (
    <div
      className="px-5 py-2 flex items-center gap-4 text-[10px] text-[#44403c]"
      style={{ borderTop: '1px solid rgba(245,158,11,0.06)' }}
    >
      {/* Segment countdown */}
      <div className="flex items-center gap-1.5">
        <div
          className={`w-[5px] h-[5px] rounded-full ${countdown !== null && countdown <= 1 ? 'animate-glow-pulse' : ''}`}
          style={{
            background: '#22c55e',
            boxShadow: '0 0 6px rgba(34,197,94,0.4)',
          }}
        />
        <span>
          Next segment{' '}
          <span className="text-[#78716c]" style={{ fontVariantNumeric: 'tabular-nums' }}>
            {countdown ?? '—'}s
          </span>
        </span>
      </div>

      {dotSep}

      <span>
        Effects{' '}
        <span style={{ color: perfColor, fontVariantNumeric: 'tabular-nums' }}>
          {stats.effects_ms}ms
        </span>
      </span>

      {dotSep}

      <span>
        Total{' '}
        <span style={{ color: perfColor, fontVariantNumeric: 'tabular-nums' }}>
          {stats.total_ms}ms
        </span>
      </span>

      {dotSep}

      <span>
        Per frame{' '}
        <span className="text-[#78716c]" style={{ fontVariantNumeric: 'tabular-nums' }}>
          {effectsPerFrame}ms
        </span>
      </span>

      {dotSep}

      <span>
        <span className="text-[#78716c]" style={{ fontVariantNumeric: 'tabular-nums' }}>
          {stats.frames}
        </span>{' '}
        frames
      </span>

      {dotSep}

      {/* Segment buffer diagram */}
      <div className="flex items-center gap-1">
        <span className="mr-1">Buffer</span>
        <div className="flex items-center gap-[2px]">
          {Array.from({ length: bufferMax }, (_, i) => {
            const isFilled = i < segments.length
            const isNewest = i === newestIdx
            const seg = isFilled ? segments[i] : null

            return (
              <div
                key={i}
                className="relative group"
                style={{
                  width: 14,
                  height: 10,
                  borderRadius: 2,
                  background: isFilled
                    ? isNewest
                      ? '#f59e0b'
                      : 'rgba(245,158,11,0.3)'
                    : 'rgba(245,158,11,0.06)',
                  border: `1px solid ${
                    isFilled
                      ? isNewest
                        ? 'rgba(245,158,11,0.8)'
                        : 'rgba(245,158,11,0.15)'
                      : 'rgba(245,158,11,0.04)'
                  }`,
                  transition: 'background 0.3s',
                }}
              >
                {seg && (
                  <div
                    className="absolute bottom-full left-1/2 -translate-x-1/2 mb-1 px-1.5 py-0.5 rounded text-[9px] whitespace-nowrap pointer-events-none opacity-0 group-hover:opacity-100 transition-opacity"
                    style={{
                      background: 'rgba(12,10,9,0.95)',
                      border: '1px solid rgba(245,158,11,0.15)',
                      color: '#a8a29e',
                    }}
                  >
                    #{seg.id} · {seg.size_kb}KB
                  </div>
                )}
              </div>
            )
          })}
        </div>
        <span className="ml-1 text-[#78716c]" style={{ fontVariantNumeric: 'tabular-nums' }}>
          {bufferCount}/{bufferMax}
        </span>
      </div>
    </div>
  )
}
