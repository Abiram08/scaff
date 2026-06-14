interface ConfidenceBadgeProps {
  confidence: 'high' | 'medium' | 'low' | 'contested'
}

const styles = {
  high: { background: 'rgba(16, 185, 129, 0.15)', border: '1px solid rgba(16, 185, 129, 0.4)', color: '#10b981', label: 'HIGH', icon: '✓' },
  medium: { background: 'rgba(245, 158, 11, 0.15)', border: '1px solid rgba(245, 158, 11, 0.4)', color: '#f59e0b', label: 'MEDIUM', icon: '!' },
  low: { background: 'rgba(249, 115, 22, 0.15)', border: '1px solid rgba(249, 115, 22, 0.4)', color: '#f97316', label: 'LOW', icon: '?' },
  contested: { background: 'rgba(239, 68, 68, 0.15)', border: '1px solid rgba(239, 68, 68, 0.4)', color: '#ef4444', label: 'CONTESTED', icon: '⚡' },
}

export function ConfidenceBadge({ confidence }: ConfidenceBadgeProps) {
  const s = styles[confidence]

  return (
    <span style={{
      display: 'inline-flex',
      alignItems: 'center',
      gap: 6,
      padding: '4px 12px',
      borderRadius: 20,
      fontSize: 11,
      fontFamily: "'Patrick Hand SC', cursive",
      letterSpacing: '0.1em',
      textTransform: 'uppercase',
      ...s,
    }}>
      <span style={{ fontSize: 10 }}>{s.icon}</span>
      {s.label}
    </span>
  )
}
