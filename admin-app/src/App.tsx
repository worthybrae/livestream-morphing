import { useEffect } from 'react'
import { Header } from './components/Header'
import { VideoPreview } from './components/VideoPreview'
import { CodeEditor } from './components/CodeEditor'
import { useProcessorStatus } from './hooks/useProcessorStatus'
import { useCodeEditor } from './hooks/useCodeEditor'
import { useLivePreview } from './hooks/useLivePreview'
import './App.css'

function App() {
  // Custom hooks for state management
  const status = useProcessorStatus()
  const frameUrl = useLivePreview(status)
  const {
    code,
    language,
    fileName,
    activeTab,
    setActiveTab,
    loading,
    saving,
    hasChanges,
    saveMessage,
    fetchCode,
    saveCode,
    resetCode,
    handleCodeChange,
  } = useCodeEditor()

  // Fetch code on mount
  useEffect(() => {
    fetchCode()
  }, [fetchCode])

  // Add keyboard shortcut for Cmd+S / Ctrl+S to save
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 's') {
        e.preventDefault()
        if (hasChanges && !saving) {
          saveCode()
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [hasChanges, saving, saveCode])

  return (
    <div className="h-screen flex flex-col bg-[#0a0a0a] text-white">
      <Header
        status={status}
        hasChanges={hasChanges}
        saving={saving}
        saveMessage={saveMessage}
        onSave={saveCode}
        onReset={resetCode}
      />

      <div className="flex-1 flex overflow-hidden">
        <VideoPreview frameUrl={frameUrl} />
        <CodeEditor
          code={code}
          language={language}
          fileName={fileName}
          activeTab={activeTab}
          onTabChange={setActiveTab}
          loading={loading}
          hasChanges={hasChanges}
          onChange={handleCodeChange}
        />
      </div>
    </div>
  )
}

export default App
