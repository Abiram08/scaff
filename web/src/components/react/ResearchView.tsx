import { useEffect, useRef } from 'react'
import ReactMarkdown from 'react-markdown'
import type { Session } from '../App'
import { FindingCard } from './FindingCard'
import { ConfidenceBadge } from './ConfidenceBadge'
import { PipelineStages } from './PipelineStages'

interface ResearchViewProps {
  session: Session
  onUpdateSession: (updates: Partial<Session>) => void
}

export function ResearchView({ session, onUpdateSession }: ResearchViewProps) {
  const wsRef = useRef<WebSocket | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const ws = new WebSocket(`${protocol}//${window.location.host}/api/v1/research/${session.id}/stream`)
    wsRef.current = ws

    ws.onmessage = (e) => {
      const event = JSON.parse(e.data)

      switch (event.event) {
        case 'status':
          onUpdateSession({ status: event.data.status })
          break
        case 'plan':
          onUpdateSession({ status: 'searching' })
          break
        case 'finding':
          onUpdateSession({
            findings: [...(session.findings || []), event.data],
          })
          break
        case 'synthesis':
          onUpdateSession({
            status: 'done',
            synthesis: event.data,
          })
          break
        case 'error':
          onUpdateSession({ status: 'error' })
          break
        case 'done':
          break
      }
    }

    ws.onerror = () => {
      onUpdateSession({ status: 'error' })
    }

    return () => {
      ws.close()
    }
  }, [session.id])

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [session.findings])

  const sendCommand = (command: string, data: any = {}) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ command, ...data }))
    }
  }

  const isStreaming = ['planning', 'searching', 'verifying', 'synthesizing'].includes(session.status)

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <header className="p-4 sm:p-5 border-b border-[#6366f1]/20" style={{
        background: 'rgba(20, 20, 30, 0.85)',
        backdropFilter: 'blur(20px)',
      }}>
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
          <div className="flex items-center gap-4 min-w-0">
            <h2 className="text-base sm:text-lg text-white truncate" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.08em' }}>
              {session.query}
            </h2>
            <ConfidenceBadge confidence={getOverallConfidence(session)} />
          </div>
          <div className="flex items-center gap-2 sm:gap-3 flex-shrink-0">
            {isStreaming && (
              <div className="flex items-center gap-2 text-xs sm:text-sm text-[#818cf8]" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.1em' }}>
                <div className="w-2 h-2 rounded-full bg-[#6366f1] animate-pulse" />
                <span className="hidden sm:inline">{session.status.toUpperCase()}...</span>
              </div>
            )}
            <button
              onClick={() => sendCommand('deepen', { topic: 'latest findings' })}
              className="px-3 sm:px-4 py-2 rounded-lg text-xs sm:text-sm text-[#a855f7] border border-[#a855f7]/30 hover:bg-[#a855f7]/10 transition-colors"
              style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.1em' }}
            >
              /DEEPEN
            </button>
            <button
              onClick={() => sendCommand('redirect', { query: 'alternative angle' })}
              className="px-3 sm:px-4 py-2 rounded-lg text-xs sm:text-sm text-[#a855f7] border border-[#a855f7]/30 hover:bg-[#a855f7]/10 transition-colors"
              style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.1em' }}
            >
              /REDIRECT
            </button>
          </div>
        </div>
      </header>

      <div ref={scrollRef} className="flex-1 overflow-y-auto p-6 space-y-6">
        <PipelineStages status={session.status} />

        {session.findings && session.findings.length > 0 && (
          <section className="space-y-4">
            <h3 className="text-xs text-white/40 uppercase" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.2em' }}>
              FINDINGS ({session.findings.length})
            </h3>
            <div className="grid gap-4">
              {session.findings.map((finding: any, i: number) => (
                <FindingCard key={i} finding={finding} index={i + 1} />
              ))}
            </div>
          </section>
        )}

        {session.synthesis && (
          <section className="space-y-5 animate-slide-up">
            <div className="p-6 rounded-xl" style={{
              background: 'rgba(15, 15, 25, 0.9)',
              backdropFilter: 'blur(16px)',
              border: '1px solid rgba(99, 102, 241, 0.15)',
              boxShadow: '0 0 30px rgba(99, 102, 241, 0.15)',
            }}>
              <h3 className="text-xs text-[#818cf8] uppercase mb-4" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.2em' }}>
                TL;DR
              </h3>
              <p className="text-white text-xl leading-relaxed" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.06em' }}>
                {session.synthesis.tldr}
              </p>
            </div>

            <div className="p-6 rounded-xl" style={{
              background: 'rgba(15, 15, 25, 0.85)',
              backdropFilter: 'blur(16px)',
              border: '1px solid rgba(99, 102, 241, 0.15)',
            }}>
              <h3 className="text-xs text-white/40 uppercase mb-5" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.2em' }}>
                FULL REPORT
              </h3>
              <div className="prose-relay">
                <ReactMarkdown>{session.synthesis.body}</ReactMarkdown>
              </div>
            </div>
          </section>
        )}

        {['planning', 'searching', 'verifying', 'synthesizing'].includes(session.status) && (
          <div className="flex items-center justify-center py-16">
            <div className="flex items-center gap-4 text-white/50" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.1em' }}>
              <svg className="w-6 h-6 animate-spin text-[#6366f1]" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
              </svg>
              <span>{getStatusMessage(session.status)}</span>
            </div>
          </div>
        )}

        {session.status === 'error' && (
          <div className="p-6 rounded-xl border border-red-500/30" style={{
            background: 'rgba(255, 0, 0, 0.08)',
            backdropFilter: 'blur(12px)',
          }}>
            <div className="flex items-start gap-3 text-red-400" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.1em' }}>
              <svg className="w-5 h-5 mt-0.5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <div className="space-y-1">
                <p className="font-bold">RESEARCH FAILED</p>
                {typeof session.synthesis === 'string' && (
                  <p className="text-sm text-red-300/80 font-normal">{session.synthesis}</p>
                )}
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

function getStatusMessage(status: string): string {
  const messages: Record<string, string> = {
    planning: 'DECOMPOSING QUERY INTO SUB-QUESTIONS...',
    searching: 'RUNNING PARALLEL SEARCHES...',
    verifying: 'CROSS-CHECKING CLAIMS...',
    synthesizing: 'BUILDING FINAL REPORT...',
  }
  return messages[status] || 'PROCESSING...'
}

function getOverallConfidence(session: Session): 'high' | 'medium' | 'low' | 'contested' {
  if (!session.findings || session.findings.length === 0) return 'medium'
  const counts = { high: 0, medium: 0, low: 0, contested: 0 }
  session.findings.forEach((f: any) => {
    counts[f.confidence as keyof typeof counts]++
  })
  if (counts.contested > 0) return 'contested'
  if (counts.high > counts.medium) return 'high'
  if (counts.medium > 0) return 'medium'
  return 'low'
}
