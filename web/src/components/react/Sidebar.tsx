import type { Session, TokenStats } from './types'

interface SidebarProps {
  sessions: Session[]
  activeSessionId: string | null
  onSelectSession: (id: string) => void
  onNewResearch: () => void
  isHome: boolean
  provider: string
  onOpenSettings: () => void
  mobileOpen: boolean
  onToggleMobile: () => void
  tokenStats: TokenStats | null
}

export function Sidebar({ sessions, activeSessionId, onSelectSession, onNewResearch, isHome, provider, onOpenSettings, mobileOpen, onToggleMobile, tokenStats }: SidebarProps) {
  const bg = isHome ? '#6366f1' : 'rgba(15, 15, 25, 0.85)'
  const borderColor = isHome ? 'rgba(255,255,255,0.15)' : 'rgba(99, 102, 241, 0.2)'
  const textMuted = isHome ? 'rgba(255,255,255,0.55)' : 'rgba(255,255,255,0.4)'
  const textVeryMuted = isHome ? 'rgba(255,255,255,0.3)' : 'rgba(255,255,255,0.3)'

  return (
    <>
      <button
        onClick={onToggleMobile}
        className="sidebar-toggle fixed top-4 left-4 z-50 p-3 rounded-xl text-white border border-white/20 transition-all duration-200 hover:bg-white/10"
        style={{
          background: 'rgba(20, 20, 30, 0.85)',
          backdropFilter: 'blur(20px)',
        }}
      >
        <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d={mobileOpen ? "M6 18L18 6M6 6l12 12" : "M4 6h16M4 12h16M4 18h16"} />
        </svg>
      </button>

      {mobileOpen && (
        <div className="sidebar-overlay fixed inset-0 z-30 lg:hidden" onClick={onToggleMobile} />
      )}

      <aside className={`w-80 flex flex-col sidebar-mobile ${mobileOpen ? 'open' : ''}`} style={{
        background: bg,
        backdropFilter: isHome ? 'none' : 'blur(20px)',
        borderRight: `1px solid ${borderColor}`,
      }}>
        <div className="p-5" style={{ borderBottom: `1px solid ${borderColor}` }}>
          <div className="flex items-center gap-4">
      <div className="w-12 h-12 rounded-xl flex items-center justify-center" style={{
            background: isHome ? 'rgba(255,255,255,0.15)' : '#6366f1',
            boxShadow: isHome ? 'none' : '0 0 20px rgba(99, 102, 241, 0.4)',
          }}>
              <svg className="w-7 h-7 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
              </svg>
            </div>
            <div>
              <h1 className="text-2xl text-white" style={{
                fontFamily: "'Patrick Hand SC', cursive",
                letterSpacing: '0.18em',
                fontWeight: 700,
              }}>RELAY</h1>
              <p className="text-xs" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.15em', fontWeight: 700, color: textMuted }}>DEEP RESEARCH</p>
            </div>
          </div>
        </div>

        <div className="p-4">
          <button
            onClick={onNewResearch}
            className="w-full flex items-center justify-center gap-2 px-6 py-3 rounded-xl text-white uppercase transition-all duration-300 hover:brightness-125"
            style={{
              background: isHome ? 'rgba(255,255,255,0.15)' : 'linear-gradient(135deg, #6366f1, #818cf8)',
              fontFamily: "'Patrick Hand SC', cursive",
              letterSpacing: '0.15em',
              fontWeight: 700,
            }}
          >
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
            </svg>
            NEW RESEARCH
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-3 space-y-2">
          {sessions.length === 0 ? (
            <div className="text-center py-10 text-sm animate-fade-in" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.15em', fontWeight: 700, color: textVeryMuted }}>
              NO SESSIONS YET
            </div>
          ) : (
            sessions.map(session => (
              <button
                key={session.id}
                onClick={() => onSelectSession(session.id)}
                className={`w-full text-left p-4 rounded-xl transition-all duration-300 group ${
                  activeSessionId === session.id ? '' : 'hover:bg-white/5'
                }`}
                style={{
                  background: activeSessionId === session.id ? (isHome ? 'rgba(255,255,255,0.12)' : 'rgba(99, 102, 241, 0.15)') : 'transparent',
                  border: activeSessionId === session.id ? `1px solid ${isHome ? 'rgba(255,255,255,0.3)' : 'rgba(99, 102, 241, 0.4)'}` : '1px solid transparent',
                  boxShadow: activeSessionId === session.id ? (isHome ? 'none' : '0 0 20px rgba(99, 102, 241, 0.2)') : 'none',
                }}
              >
                <div className="flex items-start gap-3">
                  <StatusDot status={session.status} />
                  <div className="flex-1 min-w-0">
                    <p className="text-sm text-white truncate group-hover:text-white/90" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.1em', fontWeight: 700 }}>
                      {session.query}
                    </p>
                    <p className="text-xs mt-1" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.12em', fontWeight: 700, color: textMuted }}>
                      {formatTime(session.createdAt)}
                      {session.findings.length > 0 && (
                        <span className="ml-2" style={{ color: isHome ? 'rgba(255,255,255,0.7)' : '#818cf8' }}>
                          {session.findings.length} findings
                        </span>
                      )}
                    </p>
                  </div>
                </div>
              </button>
            ))
          )}
        </div>

      {tokenStats && tokenStats.total_calls > 0 && (
        <div className="px-4 py-3 space-y-1" style={{ borderTop: `1px solid ${borderColor}` }}>
          <div className="flex items-center justify-between text-xs" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.1em', fontWeight: 700, color: textVeryMuted }}>
            <span>TOKENS USED</span>
            <span className="text-white/60">{tokenStats.total_tokens.toLocaleString()}</span>
          </div>
          <div className="flex items-center justify-between text-xs" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.1em', fontWeight: 700, color: textVeryMuted }}>
            <span>API CALLS</span>
            <span className="text-white/60">{tokenStats.total_calls}</span>
          </div>
          {tokenStats.estimated_cost_usd > 0 && (
            <div className="flex items-center justify-between text-xs" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.1em', fontWeight: 700, color: textVeryMuted }}>
              <span>EST. COST</span>
              <span className="text-white/60">${tokenStats.estimated_cost_usd.toFixed(4)}</span>
            </div>
          )}
        </div>
      )}

      <div className="p-4 flex items-center justify-between" style={{ borderTop: `1px solid ${borderColor}` }}>
        <div className="flex items-center gap-2 text-xs" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.12em', fontWeight: 700, color: textMuted }}>
          <div className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
          <span>{provider.toUpperCase()}</span>
        </div>
        <button
          onClick={onOpenSettings}
          className="p-2 rounded-lg transition-all duration-200 hover:bg-relay/15 hover:text-white"
          style={{ color: textMuted }}
        >
          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
        </button>
      </div>
      </aside>
    </>
  )
}

function StatusDot({ status }: { status: string }) {
  const colors: Record<string, string> = {
    planning: '#fbbf24',
    searching: '#6366f1',
    verifying: '#a855f7',
    synthesizing: '#06b6d4',
    done: '#10b981',
    error: '#ef4444',
  }
  const color = colors[status] || 'rgba(255,255,255,0.3)'
  const isAnimated = status !== 'done' && status !== 'error'

  return (
    <div style={{
      width: 12,
      height: 12,
      borderRadius: '50%',
      marginTop: 4,
      background: color,
      animation: isAnimated ? 'pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite' : 'none',
    }} />
  )
}

function formatTime(date: Date): string {
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}
