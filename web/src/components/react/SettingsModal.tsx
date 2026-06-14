import { useState } from 'react'

const PROVIDERS = [
  { id: 'openai', label: 'OpenAI', prefix: 'sk-', color: '#00A67E' },
  { id: 'anthropic', label: 'Anthropic', prefix: 'sk-ant-', color: '#D4A574' },
  { id: 'gemini', label: 'Gemini', prefix: 'AIza', color: '#4285F4' },
  { id: 'groq', label: 'Groq', prefix: 'gsk_', color: '#F97316' },
  { id: 'ollama', label: 'Ollama', prefix: '', color: '#FFFFFF' },
]

interface SettingsModalProps {
  open: boolean
  onClose: () => void
  currentProvider: string
  currentKey: string
  onSave: (provider: string, key: string) => void
}

type KeyStatus = 'idle' | 'testing' | 'valid' | 'invalid'

export function SettingsModal({ open, onClose, currentProvider, currentKey, onSave }: SettingsModalProps) {
  const [provider, setProvider] = useState(currentProvider)
  const [apiKey, setApiKey] = useState(currentKey)
  const [keyStatus, setKeyStatus] = useState<KeyStatus>('idle')
  const [keyMessage, setKeyMessage] = useState('')

  if (!open) return null

  const handleTestKey = async () => {
    if (!apiKey.trim() || provider === 'ollama') return
    setKeyStatus('testing')
    setKeyMessage('')
    try {
      const resp = await fetch('/api/v1/validate-key', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ provider, api_key: apiKey }),
      })
      const data = await resp.json()
      if (data.valid) {
        setKeyStatus('valid')
        setKeyMessage(`Key works! (${data.usage?.input_tokens || 0} tokens used)`)
      } else {
        setKeyStatus('invalid')
        setKeyMessage(data.error || 'Key validation failed')
      }
    } catch {
      setKeyStatus('invalid')
      setKeyMessage('Could not reach server to validate key')
    }
  }

  const handleSave = () => {
    onSave(provider, apiKey)
    onClose()
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center modal-overlay animate-fade-in p-4" onClick={onClose}>
      <div
        className="w-full max-w-lg rounded-2xl p-6 sm:p-8 space-y-6 animate-slide-up"
        style={{
          background: 'linear-gradient(135deg, #12121e, #1a1a2e)',
          border: '1px solid rgba(99, 102, 241, 0.3)',
          boxShadow: '0 20px 60px rgba(0,0,0,0.5), 0 0 40px rgba(99, 102, 241, 0.08)',
        }}
        onClick={e => e.stopPropagation()}
      >
        <div className="flex items-center justify-between">
          <h2 className="text-white text-xl" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.18em', fontWeight: 700 }}>
            SETTINGS
          </h2>
          <button onClick={onClose} className="text-white/40 hover:text-white transition-colors text-xl leading-none">&times;</button>
        </div>

        <div className="space-y-3">
          <p className="text-xs text-white/50" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.15em', fontWeight: 700 }}>
            LLM PROVIDER
          </p>
          <div className="grid grid-cols-3 sm:grid-cols-5 gap-2">
            {PROVIDERS.map(p => (
              <button
                key={p.id}
                onClick={() => { setProvider(p.id); setKeyStatus('idle'); setKeyMessage('') }}
                className="py-3 px-2 rounded-xl text-xs text-white uppercase transition-all duration-200"
                style={{
                  fontFamily: "'Patrick Hand SC', cursive",
                  letterSpacing: '0.1em',
                  fontWeight: 700,
                  background: provider === p.id ? 'rgba(99, 102, 241, 0.25)' : 'rgba(255,255,255,0.05)',
                  border: provider === p.id ? '1px solid rgba(99, 102, 241, 0.5)' : '1px solid rgba(255,255,255,0.08)',
                  color: provider === p.id ? '#fff' : 'rgba(255,255,255,0.6)',
                }}
              >
                {p.label.toUpperCase()}
              </button>
            ))}
          </div>
        </div>

        <div className="space-y-2">
          <p className="text-xs text-white/50" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.15em', fontWeight: 700 }}>
            API KEY
          </p>
          {provider === 'ollama' ? (
            <p className="text-sm text-white/60" style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.08em' }}>
              Ollama runs locally — no API key needed. Make sure the Ollama service is running on port 11434.
            </p>
          ) : (
            <>
              <input
                type="password"
                value={apiKey}
                onChange={e => { setApiKey(e.target.value); setKeyStatus('idle'); setKeyMessage('') }}
                placeholder={`Paste your ${PROVIDERS.find(p => p.id === provider)?.label} API key...`}
                className="w-full bg-white/5 border border-white/10 rounded-xl px-4 py-3 text-white text-sm outline-none transition-all duration-300 placeholder:text-white/25 focus:border-[#6366f1] focus:shadow-[0_0_20px_rgba(99,102,241,0.2)]"
                style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.08em', fontWeight: 700 }}
              />
              <div className="flex items-center gap-3">
                <button
                  onClick={handleTestKey}
                  disabled={!apiKey.trim() || keyStatus === 'testing'}
                  className="px-4 py-2 rounded-xl text-xs text-white uppercase transition-all duration-300 disabled:opacity-40 hover:brightness-125"
                  style={{
                    fontFamily: "'Patrick Hand SC', cursive",
                    letterSpacing: '0.1em',
                    fontWeight: 700,
                    background: keyStatus === 'valid'
                      ? 'rgba(34, 255, 100, 0.2)'
                      : keyStatus === 'invalid'
                      ? 'rgba(255, 34, 34, 0.2)'
                      : 'rgba(255,255,255,0.08)',
                    border: `1px solid ${
                      keyStatus === 'valid' ? 'rgba(34, 255, 100, 0.4)'
                      : keyStatus === 'invalid' ? 'rgba(255, 34, 34, 0.4)'
                      : 'rgba(255,255,255,0.15)'
                    }`,
                  }}
                >
                  {keyStatus === 'testing' ? 'TESTING...' : 'TEST KEY'}
                </button>
                {keyMessage && (
                  <span className="text-xs" style={{
                    fontFamily: "'Patrick Hand SC', cursive",
                    letterSpacing: '0.08em',
                    fontWeight: 700,
                    color: keyStatus === 'valid' ? '#10b981' : keyStatus === 'invalid' ? '#f97316' : 'rgba(255,255,255,0.5)',
                  }}>
                    {keyStatus === 'valid' ? '✓ ' : keyStatus === 'invalid' ? '✗ ' : ''}
                    {keyMessage}
                  </span>
                )}
              </div>
            </>
          )}
        </div>

        <div className="flex gap-3 pt-2">
          <button
            onClick={onClose}
            className="flex-1 py-3 rounded-xl text-sm text-white/60 uppercase transition-all duration-300 hover:bg-white/10"
            style={{
              fontFamily: "'Patrick Hand SC', cursive",
              letterSpacing: '0.12em',
              fontWeight: 700,
              background: 'rgba(255,255,255,0.05)',
              border: '1px solid rgba(255,255,255,0.1)',
            }}
          >
            CANCEL
          </button>
          <button
            onClick={handleSave}
            className="flex-1 py-3 rounded-xl text-sm text-white uppercase transition-all duration-300 hover:brightness-125"
            style={{
              fontFamily: "'Patrick Hand SC', cursive",
              letterSpacing: '0.12em',
              fontWeight: 700,
              background: 'linear-gradient(135deg, #6366f1, #818cf8)',
              boxShadow: '0 4px 20px rgba(99, 102, 241, 0.4)',
            }}
          >
            SAVE
          </button>
        </div>
      </div>
    </div>
  )
}
