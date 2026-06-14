import { useEffect, useRef, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import type { Session } from './types'
import { ConfidenceBadge } from './ConfidenceBadge'

interface Message {
  role: 'user' | 'assistant' | 'system'
  content: string
  confidence?: 'high' | 'medium' | 'low' | 'contested'
  sources?: string[]
  conflict_note?: string
  isStreaming?: boolean
}

interface ResearchChatProps {
  session: Session
  onUpdateSession: (updates: Partial<Session>) => void
}

const STAGE_MESSAGES: Record<string, string> = {
  planning: 'Decomposing query into sub-questions...',
  searching: 'Running parallel searches across Harness docs and web...',
  verifying: 'Cross-checking claims across sources...',
  synthesizing: 'Building final report...',
}

const STAGE_EMOJIS: Record<string, string> = {
  planning: '◇',
  searching: '⌕',
  verifying: '✓',
  synthesizing: '⟐',
}

export function ResearchChat({ session, onUpdateSession }: ResearchChatProps) {
  const [messages, setMessages] = useState<Message[]>(() => {
    const msgs: Message[] = [
      { role: 'user', content: session.query },
    ]
    return msgs
  })
  const [input, setInput] = useState('')
  const [isStreaming, setIsStreaming] = useState(!!session.status && session.status !== 'done' && session.status !== 'error')
  const wsRef = useRef<WebSocket | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    // Reset messages when session query changes (new session selected)
    setMessages([{ role: 'user', content: session.query }])
    setIsStreaming(session.status !== 'done' && session.status !== 'error')
  }, [session.id])

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [messages, session.status])

  useEffect(() => {
    if (!session.id) return

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const ws = new WebSocket(`${protocol}//${window.location.host}/api/v1/research/${session.id}/stream`)
    wsRef.current = ws

    setIsStreaming(true)
    setMessages(prev => {
      const filtered = prev.filter(m => !(m.role === 'system' && m.isStreaming))
      return [...filtered, { role: 'system', content: 'Planning query...', isStreaming: true }]
    })

    ws.onmessage = (e) => {
      const event = JSON.parse(e.data)

      switch (event.event) {
        case 'status': {
          const status = event.data.status
          const msg = STAGE_MESSAGES[status] || event.data.message || status
          const emoji = STAGE_EMOJIS[status] || '●'
          onUpdateSession({ status })
          setMessages(prev => {
            const filtered = prev.filter(m => !(m.role === 'system' && m.isStreaming))
            return [...filtered, { role: 'system', content: `${emoji} ${msg}`, isStreaming: true }]
          })
          break
        }
        case 'finding': {
          const d = event.data
          setMessages(prev => {
            const filtered = prev.filter(m => !(m.role === 'system' && m.isStreaming))
            return [...filtered, {
              role: 'assistant' as const,
              content: d.content,
              confidence: d.confidence,
              sources: d.sources,
              conflict_note: d.conflict_note,
            }]
          })
          onUpdateSession({
            findings: [...(session.findings || []), d],
          })
          break
        }
        case 'synthesis': {
          const d = event.data
          setMessages(prev => {
            const filtered = prev.filter(m => !(m.role === 'system' && m.isStreaming))
            let body = d.tldr || ''
            if (d.body) body += '\n\n' + d.body
            return [...filtered, { role: 'assistant' as const, content: body }]
          })
          onUpdateSession({ status: 'done', synthesis: d })
          setIsStreaming(false)
          break
        }
        case 'done': {
          setIsStreaming(false)
          setMessages(prev => prev.filter(m => !(m.role === 'system' && m.isStreaming)))
          break
        }
        case 'error': {
          setMessages(prev => {
            const filtered = prev.filter(m => !(m.role === 'system' && m.isStreaming))
            return [...filtered, { role: 'system' as const, content: `✗ Error: ${event.data.error || 'Unknown error'}` }]
          })
          onUpdateSession({ status: 'error' })
          setIsStreaming(false)
          break
        }
      }
    }

    ws.onerror = () => {
      onUpdateSession({ status: 'error' })
      setIsStreaming(false)
    }

    return () => {
      ws.close()
    }
  }, [session.id])

  const sendCommand = (cmd: string, data: any = {}) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ command: cmd, ...data }))
    }
  }

  const handleSendMessage = (e: React.FormEvent) => {
    e.preventDefault()
    if (!input.trim()) return

    if (isStreaming) {
      // Send as deepen command if still researching
      sendCommand('deepen', { topic: input })
      setMessages(prev => [...prev, { role: 'user', content: `/deepen ${input}` }])
    } else {
      // Would start a new research - for now, just add as user message
      setMessages(prev => [...prev, { role: 'user', content: input }])
    }
    setInput('')
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden bg-dark-950 page-enter-active">
      {/* ── Header ── */}
      <header className="flex-shrink-0 px-5 py-4 border-b border-white/5" style={{
        background: 'rgba(15,15,25,0.8)',
        backdropFilter: 'blur(20px)',
      }}>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3 min-w-0">
            <div className={`pulse-ring ${isStreaming ? '' : ''}`}>
              <div className="w-2 h-2 rounded-full" style={{
                background: isStreaming ? '#f59e0b' : session.status === 'error' ? '#ef4444' : '#10b981',
                animation: isStreaming ? 'pulse 2s infinite' : 'none',
              }} />
            </div>
            <h2 className="text-sm text-white/80 truncate tracking-[0.08em]" style={{ fontFamily: "'Patrick Hand SC', cursive", fontWeight: 700 }}>
              {session.query}
            </h2>
          </div>
          {session.input_tokens !== undefined && session.input_tokens + session.output_tokens > 0 && (
            <span className="text-xs text-white/25 flex-shrink-0 tracking-[0.1em]" style={{ fontFamily: "'Patrick Hand SC', cursive", fontWeight: 700 }}>
              {(session.input_tokens + session.output_tokens).toLocaleString()} tokens
            </span>
          )}
        </div>
      </header>

      {/* ── Messages ── */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto px-4 sm:px-8 py-6 space-y-4">
        {messages.map((msg, i) => (
          <div key={i} className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'} animate-fade-in`}>
            <div className={`max-w-[85%] sm:max-w-[70%] ${msg.role === 'user' ? 'order-1' : 'order-1'}`}>
              {msg.role === 'user' ? (
                <div className="p-4 rounded-2xl text-white text-sm" style={{
                  background: 'linear-gradient(135deg, #6366f1, #4f46e5)',
                  border: '1px solid rgba(99,102,241,0.3)',
                  fontFamily: "'Patrick Hand SC', cursive",
                  letterSpacing: '0.06em',
                  fontWeight: 700,
                }}>
                  {msg.content}
                </div>
              ) : msg.role === 'system' ? (
                <div className="flex items-center gap-3 px-4 py-3 rounded-xl text-sm" style={{
                  background: 'rgba(255,255,255,0.03)',
                  border: '1px solid rgba(255,255,255,0.06)',
                  color: msg.isStreaming ? 'rgba(255,255,255,0.5)' : msg.content.startsWith('✗') ? '#ef4444' : 'rgba(255,255,255,0.5)',
                  fontFamily: "'Patrick Hand SC', cursive",
                  letterSpacing: '0.08em',
                  fontWeight: 700,
                }}>
                  {msg.isStreaming && (
                    <div className="flex items-center gap-2 flex-shrink-0">
                      <svg className="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                        <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                        <path className="opacity-75" fill="#818cf8" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                      </svg>
                      <span className="flex gap-1">
                        <span className="typing-dot" />
                        <span className="typing-dot" />
                        <span className="typing-dot" />
                      </span>
                    </div>
                  )}
                  {msg.isStreaming ? msg.content : msg.content}
                </div>
              ) : (
                <div className="space-y-3">
                  {(msg.confidence || msg.sources) && (
                    <div className="flex items-center gap-3">
                      {msg.confidence && <ConfidenceBadge confidence={msg.confidence} />}
                      {msg.sources && msg.sources.length > 0 && (
                        <span className="text-xs text-white/25 tracking-[0.1em]" style={{ fontFamily: "'Patrick Hand SC', cursive", fontWeight: 700 }}>
                          {msg.sources.length} sources
                        </span>
                      )}
                    </div>
                  )}
                  <div className="p-5 rounded-2xl" style={{
                    background: 'rgba(15,15,25,0.7)',
                    border: '1px solid rgba(255,255,255,0.06)',
                    backdropFilter: 'blur(12px)',
                  }}>
                    <div className="prose-relay text-sm leading-relaxed text-white/80">
                      <ReactMarkdown>{msg.content}</ReactMarkdown>
                    </div>
                  </div>
                  {msg.conflict_note && (
                    <div className="p-4 rounded-xl border border-[#f59e0b]/30" style={{ background: 'rgba(245, 158, 11, 0.08)' }}>
                      <div className="flex items-start gap-3 text-sm text-[#f59e0b]/80" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.06em', fontWeight: 700 }}>
                        <svg className="w-5 h-5 mt-0.5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                        </svg>
                        {msg.conflict_note}
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>
        ))}

        {session.status === 'error' && (
          <div className="flex justify-center">
            <div className="p-4 rounded-xl border border-red-500/20 text-sm text-red-400/80" style={{
              background: 'rgba(239, 68, 68, 0.08)',
              fontFamily: "'Patrick Hand SC', cursive",
              letterSpacing: '0.08em',
              fontWeight: 700,
            }}>
              {typeof session.synthesis === 'string' ? session.synthesis : 'Research failed. Check your API key in Settings.'}
            </div>
          </div>
        )}
      </div>

      {/* ── Input Bar ── */}
      <div className="flex-shrink-0 px-4 sm:px-8 py-4 border-t border-white/5" style={{
        background: 'rgba(15,15,25,0.8)',
        backdropFilter: 'blur(20px)',
      }}>
        <form onSubmit={handleSendMessage} className="relative max-w-3xl mx-auto">
          <input
            ref={inputRef}
            type="text"
            value={input}
            onChange={e => setInput(e.target.value)}
            placeholder={isStreaming ? 'Deepen a topic...' : 'Ask a new question...'}
            className="w-full rounded-xl px-5 py-3.5 pr-14 text-white text-sm outline-none transition-all duration-300 border border-white/10 focus:border-[#a855f7]/40 placeholder:text-white/25"
            style={{
              fontFamily: "'Patrick Hand SC', cursive",
              letterSpacing: '0.08em',
              fontWeight: 700,
              background: 'rgba(255,255,255,0.04)',
            }}
          />
          <button
            type="submit"
            disabled={!input.trim()}
            className="absolute right-1.5 top-1/2 -translate-y-1/2 p-2.5 rounded-lg text-white transition-all duration-300 disabled:opacity-30 hover:scale-105 active:scale-95"
            style={{
              background: input.trim() ? 'linear-gradient(135deg, #6366f1, #a855f7)' : 'rgba(255,255,255,0.06)',
            }}
          >
            <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 19V5m0 0l-7 7m7-7l7 7" />
            </svg>
          </button>
        </form>
        <div className="flex items-center justify-center gap-4 mt-2">
          {isStreaming && (
            <button
              onClick={() => sendCommand('deepen', { topic: input || 'latest findings' })}
              className="text-xs text-[#a855f7]/60 hover:text-[#a855f7] transition-colors uppercase tracking-[0.12em]"
              style={{ fontFamily: "'Patrick Hand SC', cursive", fontWeight: 700 }}
            >
              /deepen
            </button>
          )}
          <span className="text-[10px] text-white/15 uppercase tracking-[0.15em]" style={{ fontFamily: "'Patrick Hand SC', cursive", fontWeight: 700 }}>
            Multi-provider · Verified claims
          </span>
        </div>
      </div>
    </div>
  )
}
