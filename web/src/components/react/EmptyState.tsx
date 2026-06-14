import { useState } from 'react'

interface EmptyStateProps {
  onSubmit: (question: string) => void
  isResearching: boolean
}

const suggestions = [
  "How does Harness handle canary deployments vs Argo Rollouts?",
  "What's the best strategy for secret rotation in multi-cloud?",
  "Compare Harness CD with Spinnaker for Kubernetes",
  "How does Harness Continuous Verification work with Prometheus?",
]

export function EmptyState({ onSubmit, isResearching }: EmptyStateProps) {
  const [question, setQuestion] = useState('')

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (question.trim() && !isResearching) {
      onSubmit(question.trim())
      setQuestion('')
    }
  }

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-4 sm:p-8 relative overflow-hidden mesh-bg">
      <div className="absolute inset-0 mesh-grid" />
      <div className="orb-glow w-96 h-96 bg-[#818cf8]/30 -top-48 -right-48 animate-float" />
      <div className="orb-glow w-80 h-80 bg-[#6366f1]/20 -bottom-40 -left-40 animate-float-slow" />
      <div className="orb-glow w-64 h-64 bg-white/[0.04] top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2" />

      <div className="relative z-10 max-w-xl w-full space-y-10 animate-fade-in">
        <div className="text-center space-y-4 animate-slide-up">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl mb-2 animate-glow" style={{
            background: 'rgba(255,255,255,0.08)',
            border: '1px solid rgba(255,255,255,0.15)',
            backdropFilter: 'blur(8px)',
          }}>
            <svg className="w-8 h-8 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
            </svg>
          </div>
          <h1 className="text-4xl sm:text-5xl text-white" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.18em', fontWeight: 700, textShadow: '0 0 40px rgba(255,255,255,0.15)' }}>
            SCAFF
          </h1>
          <p className="text-white/70 text-sm sm:text-base" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.15em', fontWeight: 700 }}>
            DEEP RESEARCH — CITED ANSWERS — CONFIDENCE VERIFIED
          </p>
        </div>

        <form onSubmit={handleSubmit} className="relative animate-slide-up" style={{ animationDelay: '0.1s' }}>
          <input
            type="text"
            value={question}
            onChange={(e) => setQuestion(e.target.value)}
            placeholder="ASK A RESEARCH QUESTION..."
            disabled={isResearching}
            className="w-full rounded-2xl px-6 py-4 pr-14 text-white text-base sm:text-lg outline-none transition-all duration-300 placeholder:text-white/40"
            style={{
              fontFamily: "'Patrick Hand SC', cursive",
              letterSpacing: '0.12em',
              fontWeight: 700,
              background: 'rgba(255,255,255,0.06)',
              border: '1px solid rgba(255,255,255,0.15)',
              backdropFilter: 'blur(12px)',
              transition: 'border-color 0.3s ease, box-shadow 0.3s ease',
            }}
            onFocus={e => { e.currentTarget.style.borderColor = 'rgba(255,255,255,0.5)'; e.currentTarget.style.boxShadow = '0 0 30px rgba(255,255,255,0.08)' }}
            onBlur={e => { e.currentTarget.style.borderColor = 'rgba(255,255,255,0.15)'; e.currentTarget.style.boxShadow = 'none' }}
          />
          <button
            type="submit"
            disabled={isResearching || !question.trim()}
            className="absolute right-2 top-1/2 -translate-y-1/2 p-3 rounded-xl text-white disabled:opacity-30 transition-all duration-300 hover:bg-white/20"
            style={{
              background: 'rgba(255,255,255,0.1)',
              border: '1px solid rgba(255,255,255,0.15)',
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

        <div className="space-y-3 animate-slide-up" style={{ animationDelay: '0.2s' }}>
          <p className="text-xs text-white/50 text-center" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.18em', fontWeight: 700 }}>
            SUGGESTED
          </p>
          <div className="grid gap-2">
            {suggestions.map((s, i) => (
              <button
                key={i}
                onClick={() => onSubmit(s)}
                disabled={isResearching}
                className="text-left p-3.5 rounded-xl text-sm disabled:opacity-40 transition-all duration-300 hover:bg-white/10 hover:border-white/25 hover:text-white"
                style={{
                  fontFamily: "'Patrick Hand SC', cursive",
                  letterSpacing: '0.1em',
                  fontWeight: 700,
                  color: 'rgba(255,255,255,0.7)',
                  background: 'rgba(255,255,255,0.04)',
                  border: '1px solid rgba(255,255,255,0.08)',
                  animation: `slideUp 0.4s ease-out ${0.3 + i * 0.1}s both`,
                }}
              >
                {s}
              </button>
            ))}
          </div>
        </div>

        <div className="hidden sm:flex items-center justify-center gap-6 animate-fade-in" style={{ animationDelay: '0.6s', fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.18em', fontWeight: 700, color: 'rgba(255,255,255,0.35)', fontSize: 11 }}>
          <span>VERIFIED CLAIMS</span>
          <span className="w-1 h-1 rounded-full" style={{ background: 'rgba(255,255,255,0.35)' }} />
          <span>HARNESS PIPELINE</span>
          <span className="w-1 h-1 rounded-full" style={{ background: 'rgba(255,255,255,0.35)' }} />
          <span>CITED SOURCES</span>
        </div>
      </div>
    </div>
  )
}
