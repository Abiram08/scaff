import { useState } from 'react'

interface LandingPageProps {
  onSubmit: (question: string) => void
  isResearching: boolean
  hasSessions: boolean
  onViewSessions: () => void
}

const features = [
  {
    title: 'VERIFIED CLAIMS',
    desc: 'Every claim cross-checked across sources. Confidence scored high/medium/low so you know what to trust.',
    color: '#10b981',
    icon: (
      <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
    ),
  },
  {
    title: 'PARALLEL SEARCH',
    desc: 'Harness docs + web searches fire simultaneously. No sequential bottlenecks — answers arrive faster.',
    color: '#a855f7',
    icon: (
      <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
      </svg>
    ),
  },
  {
    title: 'STREAMING RESULTS',
    desc: 'Findings appear as they\'re verified. Mid-stream, use /deepen or /redirect to steer the research.',
    color: '#f59e0b',
    icon: (
      <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6" />
      </svg>
    ),
  },
  {
    title: 'TOKEN AWARE',
    desc: 'Every LLM call tracked. See token usage and cost in real-time. No surprises on your bill.',
    color: '#06b6d4',
    icon: (
      <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
      </svg>
    ),
  },
]

const steps = [
  { num: '01', title: 'ASK', desc: 'Type your research question in natural language. No special syntax needed.' },
  { num: '02', title: 'RESEARCH', desc: 'Agent decomposes, searches Harness docs + web, verifies claims, and streams findings.' },
  { num: '03', title: 'REVIEW', desc: 'Read the synthesized report with confidence annotations. Deepen or redirect as needed.' },
]

export function LandingPage({ onSubmit, isResearching, hasSessions, onViewSessions }: LandingPageProps) {
  const [question, setQuestion] = useState('')

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (question.trim() && !isResearching) {
      onSubmit(question.trim())
      setQuestion('')
    }
  }

  return (
    <div className="flex-1 overflow-y-auto page-enter-active">
      {/* ── Hero Section ── */}
      <section className="relative min-h-[70vh] flex items-center justify-center px-4 sm:px-8 py-20 overflow-hidden">
        {/* Multi-color animated orbs */}
        <div className="orb-glow w-[500px] h-[500px] bg-[#6366f1]/25 -top-40 -right-40 animate-float" />
        <div className="orb-glow w-[400px] h-[400px] bg-[#a855f7]/20 -bottom-32 -left-32 animate-float-slow" />
        <div className="orb-glow w-[300px] h-[300px] bg-[#f59e0b]/15 top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2" />
        <div className="absolute inset-0 mesh-grid opacity-60" />

        {/* Gradient overlay at bottom for transition */}
        <div className="absolute bottom-0 left-0 right-0 h-40 bg-gradient-to-t from-dark-950 to-transparent z-10" />

        <div className="relative z-10 max-w-2xl w-full text-center space-y-8">
          <div className="space-y-4 animate-fade-in">
            <div className="inline-flex items-center gap-3 px-5 py-2 rounded-full text-xs uppercase tracking-[0.2em] text-white/60 border border-white/10 bg-white/5 backdrop-blur-sm mb-4">
              <span className="w-2 h-2 rounded-full bg-[#10b981] animate-pulse" />
              DEEP RESEARCH AGENT — V0.1
            </div>
            <h1 className="text-6xl sm:text-7xl text-white font-bold tracking-[0.15em]" style={{ fontFamily: "'Patrick Hand SC', cursive", textShadow: '0 0 60px rgba(99,102,241,0.3)' }}>
              SCAFF
            </h1>
            <p className="text-lg sm:text-xl text-white/60 max-w-lg mx-auto" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.1em', fontWeight: 700 }}>
              Research that ships like software.
            </p>
          </div>

          {/* Search / Input */}
          <form onSubmit={handleSubmit} className="relative max-w-xl mx-auto animate-slide-up" style={{ animationDelay: '0.15s' }}>
            <input
              type="text"
              value={question}
              onChange={e => setQuestion(e.target.value)}
              placeholder="Ask a research question..."
              disabled={isResearching}
              className="w-full rounded-2xl px-6 py-5 pr-16 text-white text-lg outline-none transition-all duration-500 placeholder:text-white/30 border border-white/10 focus:border-[#a855f7]/50 focus:shadow-[0_0_40px_-10px_rgba(168,85,247,0.3)]"
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
              disabled={isResearching || !question.trim()}
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

          <p className="text-xs text-white/25 animate-fade-in" style={{ animationDelay: '0.3s', fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.15em', fontWeight: 700 }}>
            Multi-provider LLM · Harness docs corpus · Verified claims · Cross-session memory
          </p>

          {/* Suggested quick picks */}
          <div className="flex flex-wrap justify-center gap-2 animate-fade-in" style={{ animationDelay: '0.4s' }}>
            {['Harness vs Argo canary', 'Secret rotation strategies', 'Prometheus CV setup'].map((s, i) => (
              <button
                key={i}
                onClick={() => onSubmit(s)}
                disabled={isResearching}
                className="px-4 py-2 rounded-xl text-xs text-white/50 uppercase tracking-[0.12em] transition-all duration-300 border border-white/5 hover:border-white/20 hover:text-white/80 hover:bg-white/5 disabled:opacity-30"
                style={{ fontFamily: "'Patrick Hand SC', cursive", fontWeight: 700 }}
              >
                {s}
              </button>
            ))}
          </div>

          {hasSessions && (
            <button
              onClick={onViewSessions}
              className="inline-flex items-center gap-2 text-xs text-[#a855f7] hover:text-[#c084fc] transition-colors animate-fade-in"
              style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.15em', fontWeight: 700, animationDelay: '0.5s' }}
            >
              <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 6v6m0 0v6m0-6h6m-6 0H6" />
              </svg>
              VIEW PAST RESEARCH
            </button>
          )}
        </div>
      </section>

      {/* ── Features Section ── */}
      <section className="px-4 sm:px-8 pb-24 relative z-20">
        <div className="max-w-5xl mx-auto">
          <div className="text-center mb-16">
            <h2 className="text-2xl text-white/90 uppercase tracking-[0.25em]" style={{ fontFamily: "'Patrick Hand SC', cursive", fontWeight: 700 }}>
              Why Scaff?
            </h2>
            <div className="w-16 h-0.5 mx-auto mt-4 rounded-full" style={{ background: 'linear-gradient(90deg, #6366f1, #a855f7, #f59e0b)' }} />
          </div>

          <div className="grid sm:grid-cols-2 gap-4">
            {features.map((f, i) => (
              <div
                key={i}
                className="card-hover group p-6 rounded-2xl"
                style={{
                  background: 'rgba(15,15,25,0.6)',
                  border: '1px solid rgba(255,255,255,0.06)',
                  backdropFilter: 'blur(12px)',
                }}
              >
                <div className="flex items-start gap-4">
                  <div
                    className="w-12 h-12 rounded-xl flex items-center justify-center flex-shrink-0 transition-colors duration-300"
                    style={{
                      background: `${f.color}15`,
                      color: f.color,
                    }}
                  >
                    {f.icon}
                  </div>
                  <div className="space-y-2">
                    <h3 className="text-sm text-white uppercase tracking-[0.15em]" style={{ fontFamily: "'Patrick Hand SC', cursive", fontWeight: 700 }}>
                      {f.title}
                    </h3>
                    <p className="text-sm text-white/50 leading-relaxed" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.06em', fontWeight: 700 }}>
                      {f.desc}
                    </p>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* ── How It Works Section ── */}
      <section className="px-4 sm:px-8 pb-24">
        <div className="max-w-3xl mx-auto">
          <div className="text-center mb-16">
            <h2 className="text-2xl text-white/90 uppercase tracking-[0.25em]" style={{ fontFamily: "'Patrick Hand SC', cursive", fontWeight: 700 }}>
              How It Works
            </h2>
            <div className="w-16 h-0.5 mx-auto mt-4 rounded-full" style={{ background: 'linear-gradient(90deg, #06b6d4, #6366f1, #a855f7)' }} />
          </div>

          <div className="space-y-8">
            {steps.map((step, i) => (
              <div key={i} className="flex items-start gap-6 group">
                <div className="flex flex-col items-center">
                  <div
                    className="w-12 h-12 rounded-xl flex items-center justify-center text-sm font-bold transition-all duration-300 group-hover:scale-110"
                    style={{
                      background: `linear-gradient(135deg, ${
                        i === 0 ? '#6366f1' : i === 1 ? '#a855f7' : '#10b981'
                      }, ${
                        i === 0 ? '#a855f7' : i === 1 ? '#f59e0b' : '#06b6d4'
                      })`,
                      boxShadow: `0 0 30px ${
                        i === 0 ? 'rgba(99,102,241,0.3)' : i === 1 ? 'rgba(168,85,247,0.3)' : 'rgba(16,185,129,0.3)'
                      }`,
                    }}
                  >
                    <span className="text-white text-xs">{step.num}</span>
                  </div>
                  {i < steps.length - 1 && (
                    <div className="w-0.5 h-8 my-2 rounded-full bg-white/10" />
                  )}
                </div>
                <div className="pt-2.5 space-y-1">
                  <h3 className="text-white uppercase tracking-[0.15em]" style={{ fontFamily: "'Patrick Hand SC', cursive", fontWeight: 700, fontSize: 15 }}>
                    {step.title}
                  </h3>
                  <p className="text-sm text-white/50 leading-relaxed" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.06em', fontWeight: 700 }}>
                    {step.desc}
                  </p>
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* ── Footer ── */}
      <footer className="px-8 pb-8 text-center">
        <p className="text-xs text-white/20 uppercase tracking-[0.2em]" style={{ fontFamily: "'Patrick Hand SC', cursive", fontWeight: 700 }}>
          Scaff — Research that ships like software
        </p>
      </footer>
    </div>
  )
}
