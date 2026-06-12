import { useState, useCallback } from 'react'
import { Sidebar } from './components/Sidebar'
import { ResearchView } from './components/ResearchView'
import { EmptyState } from './components/EmptyState'
import { SettingsModal } from './components/SettingsModal'

export interface Session {
  id: string
  query: string
  status: string
  findings: any[]
  synthesis: any
  createdAt: Date
}

const DEFAULT_PROVIDER = 'openai'

function loadConfig() {
  try {
    const p = localStorage.getItem('scaffProvider')
    const k = localStorage.getItem('scaffApiKey')
    return { provider: p || DEFAULT_PROVIDER, apiKey: k || '' }
  } catch {
    return { provider: DEFAULT_PROVIDER, apiKey: '' }
  }
}

function saveConfig(provider: string, apiKey: string) {
  localStorage.setItem('scaffProvider', provider)
  localStorage.setItem('scaffApiKey', apiKey)
}

function App() {
  const [sessions, setSessions] = useState<Session[]>([])
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null)
  const [isResearching, setIsResearching] = useState(false)
  const [showSettings, setShowSettings] = useState(false)
  const [config, setConfig] = useState(loadConfig)

  const activeSession = sessions.find(s => s.id === activeSessionId)

  const startResearch = useCallback(async (question: string) => {
    setIsResearching(true)

    const newSession: Session = {
      id: `sess-${Date.now()}`,
      query: question,
      status: 'planning',
      findings: [],
      synthesis: null,
      createdAt: new Date(),
    }

    setSessions(prev => [newSession, ...prev])
    setActiveSessionId(newSession.id)

    try {
      const resp = await fetch('/api/v1/research', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ question }),
      })
      const data = await resp.json()

      setSessions(prev => prev.map(s =>
        s.id === newSession.id ? { ...s, id: data.session_id } : s
      ))
      setActiveSessionId(data.session_id)
    } catch (e) {
      console.error('Failed to start research:', e)
      setSessions(prev => prev.map(s =>
        s.id === newSession.id ? { ...s, status: 'error' } : s
      ))
    } finally {
      setIsResearching(false)
    }
  }, [])

  const updateSession = useCallback((id: string, updates: Partial<Session>) => {
    setSessions(prev => prev.map(s =>
      s.id === id ? { ...s, ...updates } : s
    ))
  }, [])

  const handleSaveConfig = (provider: string, apiKey: string) => {
    saveConfig(provider, apiKey)
    setConfig({ provider, apiKey })
  }

  return (
    <div className="flex h-screen overflow-hidden bg-dark-950">
      <Sidebar
        sessions={sessions}
        activeSessionId={activeSessionId}
        onSelectSession={setActiveSessionId}
        onNewResearch={() => setActiveSessionId(null)}
        isHome={!activeSession}
        provider={config.provider}
        onOpenSettings={() => setShowSettings(true)}
      />

      <main className={`flex-1 flex flex-col overflow-hidden ${activeSession ? '' : ''}`}>
        {activeSession ? (
          <ResearchView
            session={activeSession}
            onUpdateSession={(updates) => updateSession(activeSession.id, updates)}
          />
        ) : (
          <EmptyState onSubmit={startResearch} isResearching={isResearching} />
        )}
      </main>

      <SettingsModal
        open={showSettings}
        onClose={() => setShowSettings(false)}
        currentProvider={config.provider}
        currentKey={config.apiKey}
        onSave={handleSaveConfig}
      />
    </div>
  )
}

export default App
