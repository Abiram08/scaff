import { useState } from 'react'

interface Source {
  url: string
  title: string
  relevance?: number
  chunk_id?: string
}

interface SourceListProps {
  sources: Source[]
}

export function SourceList({ sources }: SourceListProps) {
  const [expanded, setExpanded] = useState(false)

  if (!sources || sources.length === 0) return null

  const displaySources = expanded ? sources : sources.slice(0, 3)

  return (
    <div className="glass-card p-5">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-sm text-white/40 uppercase" style={{ letterSpacing: '0.2em' }}>
          SOURCES ({sources.length})
        </h3>
      </div>

      <div className="space-y-2">
        {displaySources.map((source, i) => (
          <div
            key={i}
            className="flex items-start gap-3 p-3 rounded-xl hover:bg-relay/10 transition-colors group"
          >
            <span className="flex items-center justify-center w-7 h-7 rounded-lg bg-relay/20 text-xs font-bold text-relay-light flex-shrink-0">
              {i + 1}
            </span>
            <div className="flex-1 min-w-0">
              <a
                href={source.url}
                target="_blank"
                rel="noopener noreferrer"
                className="text-sm text-relay-light hover:text-white truncate block group-hover:underline"
                style={{ letterSpacing: '0.05em' }}
              >
                {source.title || source.url}
              </a>
              <p className="text-xs text-white/30 truncate mt-1">
                {new URL(source.url).hostname}
              </p>
            </div>
            {source.relevance !== undefined && (
              <span className="text-xs text-white/20 flex-shrink-0">
                {Math.round(source.relevance * 100)}%
              </span>
            )}
          </div>
        ))}
      </div>

      {sources.length > 3 && (
        <button
          onClick={() => setExpanded(!expanded)}
          className="mt-4 text-xs text-relay-light hover:text-white transition-colors"
          style={{ letterSpacing: '0.1em' }}
        >
          {expanded ? 'SHOW LESS' : `SHOW ALL ${sources.length} SOURCES`}
        </button>
      )}
    </div>
  )
}
