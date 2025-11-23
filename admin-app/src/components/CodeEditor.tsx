import Editor from '@monaco-editor/react'

type FileTab = 'cpp' | 'python'

interface CodeEditorProps {
  code: string
  language: string
  fileName: string
  activeTab: FileTab
  onTabChange: (tab: FileTab) => void
  loading: boolean
  hasChanges: boolean
  onChange: (value: string | undefined) => void
}

export function CodeEditor({ code, language, fileName, activeTab, onTabChange, loading, hasChanges, onChange }: CodeEditorProps) {
  return (
    <div className="w-1/2 flex flex-col bg-[#1e1e1e]">
      {/* Editor Tabs */}
      <div className="h-10 bg-[#2d2d2d] border-b border-zinc-800 flex items-center gap-1" style={{ paddingLeft: '0.5rem', paddingRight: '1rem' }}>
        <button
          onClick={() => onTabChange('cpp')}
          className={`px-3 py-1.5 text-xs rounded-t transition-colors ${
            activeTab === 'cpp'
              ? 'bg-[#1e1e1e] text-zinc-200'
              : 'text-zinc-500 hover:text-zinc-300'
          }`}
        >
          <div className="flex items-center gap-2">
            fast_processor.cpp
            {activeTab === 'cpp' && hasChanges && (
              <span className="w-1.5 h-1.5 bg-zinc-400 rounded-full"></span>
            )}
          </div>
        </button>
        <button
          onClick={() => onTabChange('python')}
          className={`px-3 py-1.5 text-xs rounded-t transition-colors ${
            activeTab === 'python'
              ? 'bg-[#1e1e1e] text-zinc-200'
              : 'text-zinc-500 hover:text-zinc-300'
          }`}
        >
          <div className="flex items-center gap-2">
            image_processing.py
            {activeTab === 'python' && hasChanges && (
              <span className="w-1.5 h-1.5 bg-zinc-400 rounded-full"></span>
            )}
          </div>
        </button>
      </div>

      {/* Editor */}
      <div className="flex-1">
        {loading ? (
          <div className="h-full flex items-center justify-center bg-[#1e1e1e]">
            <p className="text-xs text-zinc-600">Loading...</p>
          </div>
        ) : (
          <Editor
            height="100%"
            defaultLanguage={language}
            value={code}
            onChange={onChange}
            theme="vs-dark"
            options={{
              minimap: { enabled: false },
              fontSize: 13,
              lineNumbers: 'on',
              scrollBeyondLastLine: false,
              automaticLayout: true,
              tabSize: language === 'cpp' ? 4 : 4,
              wordWrap: 'off',
              fontFamily: 'SF Mono, Menlo, Monaco, Courier New, monospace',
              padding: { top: 12, bottom: 12 },
              lineHeight: 20,
            }}
          />
        )}
      </div>
    </div>
  )
}
