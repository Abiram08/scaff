import ReactMarkdown from 'react-markdown'
import { ConfidenceBadge } from './ConfidenceBadge'

interface FindingCardProps {
  finding: {
    content: string
    confidence: 'high' | 'medium' | 'low' | 'contested'
    sources?: string[]
    conflict_note?: string
  }
  index: number
}

export function FindingCard({ finding, index }: FindingCardProps) {
  return (
    <div className="p-5 rounded-xl animate-slide-up" style={{
      animationDelay: `${index * 80}ms`,
      background: 'rgba(25, 25, 40, 0.9)',
      backdropFilter: 'blur(12px)',
      border: '1px solid rgba(34, 0, 255, 0.15)',
    }}>
      <div className="flex items-start justify-between gap-4 mb-4">
        <div className="flex items-center gap-3">
          <span className="flex items-center justify-center w-8 h-8 rounded-lg text-sm text-white"
            style={{ background: 'rgba(34, 0, 255, 0.2)' }}>
            {index}
          </span>
          <ConfidenceBadge confidence={finding.confidence} />
        </div>
        {finding.sources && finding.sources.length > 0 && (
          <div className="flex items-center gap-1.5 text-xs text-white/30" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.1em' }}>
            <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
            </svg>
            <span>{finding.sources.length} SOURCES</span>
          </div>
        )}
      </div>

      <div className="prose-relay text-white/80">
        <ReactMarkdown>{finding.content}</ReactMarkdown>
      </div>

      {finding.conflict_note && (
        <div className="mt-4 p-4 rounded-xl" style={{
          background: 'rgba(255, 200, 0, 0.08)',
          border: '1px solid rgba(255, 200, 0, 0.3)',
        }}>
          <div className="flex items-start gap-3">
            <svg className="w-5 h-5 text-yellow-500 mt-0.5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
            <p className="text-sm text-yellow-300/80" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.06em' }}>
              {finding.conflict_note}
            </p>
          </div>
        </div>
      )}
    </div>
  )
}
