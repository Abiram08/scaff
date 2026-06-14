import { useState, useCallback, useEffect } from 'react'
import { Sidebar } from './Sidebar'
import { ResearchChat } from './ResearchChat'
import { LandingPage } from './LandingPage'
import { SettingsModal } from './SettingsModal'
import type { Session, TokenStats } from './types'

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
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false)
  const [tokenStats, setTokenStats] = useState<TokenStats | null>(null)

  const fetchTokenStats = useCallback(async () => {
    try {
      const resp = await fetch('/api/v1/tokens')
      if (resp.ok) {
        setTokenStats(await resp.json())
      }
    } catch {}
  }, [])

  useEffect(() => {
    fetchTokenStats()
    const interval = setInterval(fetchTokenStats, 10000)
    return () => clearInterval(interval)
  }, [fetchTokenStats])

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
        body: JSON.stringify({
          question,
          provider: config.provider,
          api_key: config.apiKey || undefined,
        }),
      })
      if (!resp.ok) {
        const errData = await resp.json().catch(() => ({}))
        throw new Error(errData.detail || errData.error || `Request failed (${resp.status})`)
      }
      const data = await resp.json()

      setSessions(prev => prev.map(s =>
        s.id === newSession.id ? { ...s, id: data.session_id } : s
      ))
      setActiveSessionId(data.session_id)
    } catch (e) {
      const msg = e instanceof Error ? e.message : 'Unknown error'
      console.error('Failed to start research:', msg)
      setSessions(prev => prev.map(s =>
        s.id === newSession.id ? { ...s, status: 'error', synthesis: msg } : s
      ))
    }
    fetchTokenStats()
    setIsResearching(false)
  }, [fetchTokenStats])

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
        onSelectSession={(id) => { setActiveSessionId(id); setMobileSidebarOpen(false) }}
        onNewResearch={() => { setActiveSessionId(null); setMobileSidebarOpen(false) }}
        isHome={!activeSession}
        provider={config.provider}
        onOpenSettings={() => setShowSettings(true)}
        mobileOpen={mobileSidebarOpen}
        onToggleMobile={() => setMobileSidebarOpen(v => !v)}
        tokenStats={tokenStats}
      />

      <main className="flex-1 flex flex-col overflow-hidden">
        {activeSession ? (
          <ResearchChat
            session={activeSession}
            onUpdateSession={(updates) => updateSession(activeSession.id, updates)}
          />
        ) : (
          <LandingPage
            onSubmit={startResearch}
            isResearching={isResearching}
            hasSessions={sessions.length > 0}
            onViewSessions={() => {
              if (sessions.length > 0) {
                setActiveSessionId(sessions[0].id)
                setMobileSidebarOpen(false)
              }
            }}
          />
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
