import { useState, useEffect } from 'react'

export type Tab = 'effects' | 'pipeline' | 'presets'

interface HeaderProps {
  activeTab: Tab
  onTabChange: (tab: Tab) => void
  sourceUrl: string
  onSourceUrlChange: (url: string) => void
  onRandomize: () => void
}

const tabs: { id: Tab; label: string }[] = [
  { id: 'effects', label: 'Effects' },
  { id: 'pipeline', label: 'Pipeline' },
  { id: 'presets', label: 'Presets' },
]

export function Header({ activeTab, onTabChange, sourceUrl, onSourceUrlChange, onRandomize }: HeaderProps) {
  const [localUrl, setLocalUrl] = useState(sourceUrl)

  useEffect(() => {
    setLocalUrl(sourceUrl)
  }, [sourceUrl])

  const handleUrlSubmit = () => {
    if (localUrl.trim() && localUrl !== sourceUrl) {
      onSourceUrlChange(localUrl.trim())
    }
  }

  return (
    <div
      className="flex items-center px-5 py-3"
      style={{
        background: 'linear-gradient(180deg, rgba(245,158,11,0.03) 0%, transparent 100%)',
        borderBottom: '1px solid rgba(245,158,11,0.06)',
      }}
    >
      {/* Logo */}
      <div className="flex items-center gap-2">
        <div
          className="w-6 h-6 rounded-md flex items-center justify-center text-[10px] font-bold text-white"
          style={{ background: 'linear-gradient(135deg, #f59e0b, #f97316)' }}
        >
          M
        </div>
        <span className="text-[#fafaf9] font-semibold text-[13px] tracking-tight">
          Morph Studio
        </span>
      </div>

      {/* Centered pill tab switcher */}
      <div className="flex-1 flex justify-center">
        <div className="flex rounded-lg p-[3px] gap-[2px]" style={{ background: 'rgba(255,255,255,0.04)' }}>
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => onTabChange(tab.id)}
              className={`px-4 py-[5px] text-[11px] font-medium rounded-md transition-all duration-150 ${
                activeTab === tab.id
                  ? 'text-[#fafaf9] shadow-sm'
                  : 'text-[#78716c] hover:text-[#a8a29e]'
              }`}
              style={
                activeTab === tab.id
                  ? { background: 'rgba(255,255,255,0.08)', boxShadow: '0 1px 2px rgba(0,0,0,0.3)' }
                  : undefined
              }
            >
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      {/* Right: URL input + Randomize */}
      <div className="flex items-center gap-3">
        <input
          value={localUrl}
          onChange={(e) => setLocalUrl(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && handleUrlSubmit()}
          onBlur={handleUrlSubmit}
          placeholder="Stream URL..."
          className="px-3 py-[5px] text-[11px] rounded-md w-60 outline-none transition-colors"
          style={{
            background: '#0a0a09',
            border: '1px solid rgba(255,255,255,0.06)',
            color: '#a8a29e',
          }}
          onFocus={(e) => (e.target.style.borderColor = 'rgba(245,158,11,0.2)')}
          onBlurCapture={(e) => (e.target.style.borderColor = 'rgba(255,255,255,0.06)')}
        />
        <button
          onClick={onRandomize}
          className="px-3 py-[5px] text-[11px] font-medium rounded-md transition-all duration-150 text-[#a8a29e] hover:text-[#e7e5e4]"
          style={{
            background: 'rgba(255,255,255,0.04)',
            border: '1px solid rgba(255,255,255,0.06)',
          }}
        >
          Randomize
        </button>
      </div>
    </div>
  )
}
