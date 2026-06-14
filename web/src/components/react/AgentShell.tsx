import { useState, useCallback, useEffect } from 'react'
import { Sidebar } from './Sidebar'
import { ResearchChat } from './ResearchChat'
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

export function AgentShell() {
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
    if (isResearching) return
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
      {/* Desktop sidebar always visible; mobile via toggle */}
      <div className="hidden lg:flex">
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
      </div>

      {/* Mobile sidebar */}
      <div className="lg:hidden">
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
      </div>

      <main className="flex-1 flex flex-col overflow-hidden">
        {activeSession ? (
          <ResearchChat
            session={activeSession}
            onUpdateSession={(updates) => updateSession(activeSession.id, updates)}
          />
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center px-6">
            <div className="max-w-xl w-full text-center space-y-8">
              <div className="space-y-4">
                <div className="w-16 h-16 rounded-2xl bg-gradient-to-br from-relay to-accent flex items-center justify-center mx-auto shadow-lg shadow-relay/30">
                  <svg className="w-8 h-8 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
                  </svg>
                </div>
                <h2 className="text-2xl font-hand font-bold tracking-[0.15em] text-white/90">
                  Start a Research Session
                </h2>
                <p className="text-sm font-hand font-bold tracking-[0.08em] text-white/40 max-w-md mx-auto leading-relaxed">
                  Type a question and press Enter. Relay will decompose, search, verify, and stream findings in real-time.
                </p>
              </div>

              <form
                onSubmit={(e) => {
                  e.preventDefault()
                  const input = (e.target as HTMLFormElement).querySelector('input')
                  if (input && input.value.trim() && !isResearching) {
                    startResearch(input.value.trim())
                    input.value = ''
                  }
                }}
                className="relative"
              >
                <input
                  type="text"
                  placeholder="e.g. How does Harness CV work with Prometheus?"
                  disabled={isResearching}
                  className="w-full rounded-2xl px-6 py-5 pr-16 text-white text-base outline-none transition-all duration-500 placeholder:text-white/25 border border-white/10 focus:border-accent/50 focus:shadow-[0_0_40px_-10px_rgba(168,85,247,0.3)]"
                  style={{
                    fontFamily: "'Patrick Hand SC', cursive",
                    letterSpacing: '0.08em',
                    fontWeight: 700,
                    background: 'rgba(15,15,25,0.7)',
                    backdropFilter: 'blur(20px)',
                  }}
                />
                <button
                  type="submit"
                  disabled={isResearching}
                  className="absolute right-2 top-1/2 -translate-y-1/2 p-3.5 rounded-xl text-white transition-all duration-300 disabled:opacity-30 hover:scale-105 active:scale-95"
                  style={{
                    background: 'linear-gradient(135deg, #6366f1, #a855f7)',
                    boxShadow: '0 4px 20px rgba(99,102,241,0.4)',
                  }}
                >
                  {isResearching ? (
                    <svg className="w-5 h-5 animate-spin" fill="none" viewBox="0 0 24 24">
                      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                    </svg>
                  ) : (
                    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                    </svg>
                  )}
                </button>
              </form>

              <div className="flex flex-wrap justify-center gap-2">
                {['Harness vs Argo canary', 'Secret rotation strategies', 'Prometheus CV setup'].map((s, i) => (
                  <button
                    key={i}
                    onClick={() => startResearch(s)}
                    disabled={isResearching}
                    className="px-4 py-2 rounded-xl text-xs text-white/50 uppercase tracking-[0.12em] transition-all duration-300 border border-white/5 hover:border-white/20 hover:text-white/80 hover:bg-white/5 disabled:opacity-30 font-hand font-bold"
                  >
                    {s}
                  </button>
                ))}
              </div>

              {!config.apiKey && (
                <div className="p-4 rounded-xl border border-warm/20 bg-warm/[0.06]">
                  <p className="text-xs font-hand font-bold tracking-[0.1em] text-warm/70">
                    No API key set. Open Settings (⚙️) to configure your LLM provider.
                  </p>
                </div>
              )}
            </div>
          </div>
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
