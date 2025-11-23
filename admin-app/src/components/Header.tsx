import { useState, useEffect } from 'react'
import type { ProcessorStatus } from '../types'
import { Button } from './ui/button'

interface HeaderProps {
  status: ProcessorStatus | null
  hasChanges: boolean
  saving: boolean
  saveMessage: string | null
  onSave: () => void
  onReset: () => void
}

const DEFAULT_STREAM_URL = 'https://videos-3.earthcam.com/fecnetwork/AbbeyRoadHD1.flv/chunklist_w'

export function Header({ status, hasChanges, saving, saveMessage, onSave, onReset }: HeaderProps) {
  const [streamUrl, setStreamUrl] = useState(() => {
    // Load from localStorage or use default
    return localStorage.getItem('streamUrl') || DEFAULT_STREAM_URL
  })
  const [isEditingUrl, setIsEditingUrl] = useState(false)

  const handleUrlChange = async (newUrl: string) => {
    setStreamUrl(newUrl)
    localStorage.setItem('streamUrl', newUrl)

    // Send to backend
    try {
      await fetch('http://localhost:8000/api/admin/stream-url', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ url: newUrl })
      })
      console.log('✅ Stream URL updated:', newUrl)
    } catch (err) {
      console.error('Failed to update stream URL:', err)
    }
  }

  useEffect(() => {
    // Set initial URL on backend on mount
    handleUrlChange(streamUrl)
  }, [])

  return (
    <header className="h-12 border-b border-zinc-800/60 bg-black" style={{ paddingLeft: '2rem', paddingRight: '2rem' }}>
      <div className="h-full max-w-7xl mx-auto flex items-center justify-between">
        <div className="flex items-center gap-6">
          <h1 className="text-sm font-medium text-zinc-200">Livestream Morphing</h1>

          {/* Stream URL Input */}
          <div className="flex items-center gap-2">
            <label className="text-xs text-zinc-500">Stream:</label>
            {isEditingUrl ? (
              <input
                type="text"
                value={streamUrl}
                onChange={(e) => setStreamUrl(e.target.value)}
                onBlur={() => {
                  setIsEditingUrl(false)
                  handleUrlChange(streamUrl)
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    setIsEditingUrl(false)
                    handleUrlChange(streamUrl)
                  }
                }}
                className="text-xs font-mono bg-zinc-900 text-zinc-300 border border-zinc-700 rounded px-2 py-1"
                style={{ width: '300px' }}
                autoFocus
              />
            ) : (
              <button
                onClick={() => setIsEditingUrl(true)}
                className="text-xs font-mono text-zinc-400 hover:text-zinc-200 transition-colors"
              >
                {streamUrl.length > 40 ? `${streamUrl.slice(0, 40)}...` : streamUrl}
              </button>
            )}
          </div>

          <div className="flex items-center gap-4 text-xs text-zinc-500">
            <div className="flex items-center gap-1.5">
              <div className={`w-1 h-1 rounded-full ${status?.running ? 'bg-emerald-500' : 'bg-zinc-600'}`} />
              <span>{status?.running ? 'Live' : 'Offline'}</span>
            </div>
            <div className="w-px h-3 bg-zinc-800" />
            <span className="font-mono text-zinc-400">{status?.recent_segments?.[0] || '—'}</span>
            <div className="w-px h-3 bg-zinc-800" />
            <span><span className="font-mono text-zinc-400">{status?.total_processed || 0}</span> processed</span>
          </div>
        </div>

      <div className="flex items-center gap-2">
        {saveMessage && (
          <span className="text-xs text-zinc-500 mr-2">
            {saveMessage}
          </span>
        )}
        {hasChanges && (
          <Button
            onClick={onReset}
            disabled={saving}
            variant="ghost"
            size="sm"
            className="text-zinc-400 hover:text-zinc-200"
            style={{ paddingLeft: '1rem', paddingRight: '1rem' }}
          >
            Reset
          </Button>
        )}
        <Button
          onClick={onSave}
          disabled={!hasChanges || saving}
          variant="outline"
          size="sm"
          className="border-white text-white hover:bg-white hover:text-black"
          style={{ paddingLeft: '1.5rem', paddingRight: '1.5rem' }}
        >
          {saving ? 'Applying...' : 'Apply'}
        </Button>
      </div>
    </div>
    </header>
  )
}
