interface ConfidenceBadgeProps {
  confidence: 'high' | 'medium' | 'low' | 'contested'
}

const styles = {
  high: { background: 'rgba(34, 255, 100, 0.15)', border: '1px solid rgba(34, 255, 100, 0.4)', color: '#22ff64', label: 'HIGH', icon: '✓' },
  medium: { background: 'rgba(255, 200, 34, 0.15)', border: '1px solid rgba(255, 200, 34, 0.4)', color: '#ffc822', label: 'MEDIUM', icon: '!' },
  low: { background: 'rgba(255, 100, 34, 0.15)', border: '1px solid rgba(255, 100, 34, 0.4)', color: '#ff6422', label: 'LOW', icon: '?' },
  contested: { background: 'rgba(255, 34, 34, 0.15)', border: '1px solid rgba(255, 34, 34, 0.4)', color: '#ff2222', label: 'CONTESTED', icon: '⚡' },
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
