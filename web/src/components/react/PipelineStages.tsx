interface PipelineStagesProps {
  status: string
}

const stages = [
  { key: 'planning', label: 'PLAN', icon: '◇' },
  { key: 'searching', label: 'SEARCH', icon: '⌕' },
  { key: 'verifying', label: 'VERIFY', icon: '✓' },
  { key: 'synthesizing', label: 'SYNTH', icon: '⟐' },
  { key: 'done', label: 'DONE', icon: '●' },
]

const statusOrder = ['planning', 'searching', 'verifying', 'synthesizing', 'done']

export function PipelineStages({ status }: PipelineStagesProps) {
  const currentIdx = statusOrder.indexOf(status)

  return (
    <div className="p-5 rounded-xl" style={{
      background: 'rgba(25, 25, 40, 0.9)',
      backdropFilter: 'blur(12px)',
      border: '1px solid rgba(99, 102, 241, 0.15)',
    }}>
      <div className="flex items-center justify-between">
        {stages.map((stage, i) => {
          const stageIdx = statusOrder.indexOf(stage.key)
          const isActive = stage.key === status
          const isComplete = stageIdx < currentIdx

          return (
            <div key={stage.key} className="flex items-center">
              <div className="flex flex-col items-center gap-2">
                <div style={{
                  width: 48,
                  height: 48,
                  borderRadius: 12,
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  fontSize: 14,
                  transition: 'all 0.4s ease',
                  ...(isActive ? {
                    background: 'linear-gradient(135deg, #6366f1, #818cf8)',
                    color: 'white',
                    boxShadow: '0 0 25px rgba(99, 102, 241, 0.5)',
                    transform: 'scale(1.1)',
                  } : isComplete ? {
                    background: 'rgba(99, 102, 241, 0.2)',
                    color: '#818cf8',
                  } : {
                    background: 'rgba(30, 30, 45, 0.8)',
                    color: 'rgba(255, 255, 255, 0.3)',
                  }),
                }}>
                  {isComplete ? (
                    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                    </svg>
                  ) : (
                    <span>{stage.icon}</span>
                  )}
                </div>
                <span style={{
                  fontSize: 12,
                  fontFamily: "'Patrick Hand SC', cursive",
                  letterSpacing: '0.15em',
                  color: isActive ? '#818cf8' : isComplete ? 'rgba(255,255,255,0.6)' : 'rgba(255,255,255,0.2)',
                }}>
                  {stage.label}
                </span>
              </div>
              {i < stages.length - 1 && (
                <div style={{
                  width: 56,
                  height: 2,
                  margin: '0 12px',
                  marginTop: -20,
                  background: isComplete ? '#6366f1' : 'rgba(255,255,255,0.1)',
                }} />
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}
