import { useEffect, useRef, useState } from 'react'
import Hls from 'hls.js'
import { useEffects } from './hooks/useEffects'
import { usePipeline } from './hooks/usePipeline'
import './living-ink.css'

type Metrics = { width: number; height: number; frames: number; source_fps: number; frame_p50_ms: number; frame_p95_ms: number; total_ms: number; duration_seconds: number; bytes: number }
type Saved = { name: string; version: string; params: Record<string, number> }

export default function App() {
  const effects = useEffects()
  const { slots } = usePipeline()
  const preset = slots.find(slot => slot.effect_id === 'living_ink')
  const ink = effects.find(e => e.id === 'living_ink')
  const [params, setParams] = useState<Record<string, number>>({})
  const [url, setUrl] = useState('')
  const [metrics, setMetrics] = useState<Metrics | null>(null)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('Tune the strokes, then render your captured clip.')
  const [name, setName] = useState('Abbey Road')
  const [saved, setSaved] = useState<Saved[]>(() => {
    try { return JSON.parse(localStorage.getItem('living-ink-presets') || '[]') } catch { return [] }
  })
  const video = useRef<HTMLVideoElement>(null)
  const draft = Object.fromEntries((ink?.params ?? []).map(p => [p.id, params[p.id] ?? preset?.params[p.id] ?? p.default]))

  useEffect(() => {
    const el = video.current
    if (!el || !url) return
    if (el.canPlayType('application/vnd.apple.mpegurl')) { el.src = url; return }
    const hls = new Hls()
    hls.loadSource(url); hls.attachMedia(el)
    hls.on(Hls.Events.ERROR, (_, data) => { if (data.fatal) setMessage('Preview playback failed. Render the clip again.') })
    return () => hls.destroy()
  }, [url])

  async function render() {
    setBusy(true); setMessage('Rendering the captured clip…')
    try {
      const response = await fetch('/api/studio/preview', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(draft) })
      if (!response.ok) throw new Error(await response.text())
      const result = await response.json()
      setUrl(result.url); setMetrics(result.metrics)
      setMessage('Preview ready. Public stream settings have not changed.')
    } catch (error) { setMessage(error instanceof Error ? error.message : 'Render failed') }
    finally { setBusy(false) }
  }
  function save() {
    const entry = { name: name.trim() || 'Untitled', version: new Date().toISOString(), params: draft }
    const next = [entry, ...saved].slice(0, 30)
    try { localStorage.setItem('living-ink-presets', JSON.stringify(next)); setSaved(next); setMessage('Version saved in this browser.') }
    catch { setMessage('Browser storage is unavailable. Export the preset to keep it.') }
  }
  function exportPreset() {
    const data = { name: name.trim() || 'Untitled', version: new Date().toISOString(), renderer: 'living_ink', params: draft }
    const href = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' }))
    const link = document.createElement('a'); link.href = href; link.download = 'living-ink-preset.json'; link.click()
    setTimeout(() => URL.revokeObjectURL(href), 1000)
    setMessage('Preset exported. Include its params in the production preset and deploy after reviewing the cost benchmark.')
  }
  return <main className="ink-studio">
    <header className="ink-header"><span>W / LIVING INK</span><span>ABBEY ROAD · STUDY 01</span><span>LOCAL STUDIO</span></header>
    <div className="ink-workspace">
      <section className="ink-preview">
        <div className="ink-caption"><span>A STREET IN MOTION.</span><span>{metrics ? `${metrics.width} × ${metrics.height}` : 'CAPTURED SOURCE'}</span></div>
        <div className="ink-picture">
          <video ref={video} controls autoPlay loop muted playsInline aria-label="Rust-rendered Living Ink preview" hidden={!url} />
          {!url && <p>Light becomes a line.<br />Movement bends the line.<small>Render a captured clip to begin.</small></p>}
        </div>
        <p className="ink-message" role="status">{message}</p>
        <div className="ink-equation"><span>THE MODEL</span><code>contour = thin(∇ smooth(luminance))<br />memory = max(departed contour, memory × 2<sup>−Δt / half-life</sup>)<br />painting = charcoal + max(contour, memory × strength)</code></div>
        {metrics && <dl className="ink-metrics">
          <div><dt>FRAME P50 / P95</dt><dd>{metrics.frame_p50_ms.toFixed(1)} / {metrics.frame_p95_ms.toFixed(1)} ms</dd></div>
          <div><dt>SOURCE CADENCE</dt><dd>{metrics.source_fps.toFixed(2)} fps</dd></div>
          <div><dt>RENDER + ENCODE</dt><dd>{(metrics.total_ms / 1000).toFixed(2)}s / {metrics.duration_seconds.toFixed(2)}s clip</dd></div>
          <div><dt>ENCODED SIZE</dt><dd>{Math.round(metrics.bytes / 1024)} KiB</dd></div>
        </dl>}
        <p className="ink-note">One native renderer for the studio and the stream. Previewing uses this machine; it does not wake Railway. Timings measure this machine, not the production bill.</p>
      </section>
      <aside className="ink-controls">
        <h1>The marks</h1><p>Keep the street quiet. Let motion carry the light.</p>
        {(ink?.params ?? []).map(p => <label className="ink-slider" key={p.id}><span>{p.name}<output>{draft[p.id].toFixed(p.step < 1 ? 2 : 0)}</output></span><input type="range" min={p.min} max={p.max} step={p.step} value={draft[p.id]} onChange={e => setParams(previous => ({ ...previous, [p.id]: Number(e.target.value) }))} /></label>)}
        {!ink && <p>Start the local Rust engine to load the controls.</p>}
        <button className="ink-render" disabled={busy || !ink || !preset} onClick={() => void render()}>{busy ? 'Rendering…' : 'Render preview ↗'}</button>
        <div className="ink-presets"><label>PRESET NAME<input value={name} onChange={e => setName(e.target.value)} /></label><div className="ink-actions"><button onClick={save} disabled={!ink}>Save version</button><button onClick={exportPreset} disabled={!ink}>Export for production</button></div>
          {saved.length > 0 && <select aria-label="Load a saved preset" defaultValue="" onChange={e => { const p = saved[Number(e.target.value)]; if (p) { setParams(p.params); setName(p.name); setMessage('Saved version loaded into the draft. Render to preview.')} }}><option value="" disabled>Load a saved version</option>{saved.map((p, i) => <option key={p.version} value={i}>{p.name} · {new Date(p.version).toLocaleString()}</option>)}</select>}
        </div>
      </aside>
    </div>
  </main>
}
