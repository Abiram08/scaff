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

export function SettingsModal({ open, onClose, currentProvider, currentKey, onSave }: SettingsModalProps) {
  const [provider, setProvider] = useState(currentProvider)
  const [apiKey, setApiKey] = useState(currentKey)

  if (!open) return null

  const handleSave = () => {
    onSave(provider, apiKey)
    onClose()
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center modal-overlay animate-fade-in" onClick={onClose}>
      <div
        className="w-full max-w-lg mx-4 rounded-2xl p-8 space-y-6 animate-slide-up"
        style={{
          background: 'linear-gradient(135deg, #1a1a2e, #16213e)',
          border: '1px solid rgba(34, 0, 255, 0.3)',
          boxShadow: '0 20px 60px rgba(0,0,0,0.5)',
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
          <div className="grid grid-cols-5 gap-2">
            {PROVIDERS.map(p => (
              <button
                key={p.id}
                onClick={() => setProvider(p.id)}
                className="py-3 px-2 rounded-xl text-xs text-white uppercase transition-all duration-200"
                style={{
                  fontFamily: "'Patrick Hand SC', cursive",
                  letterSpacing: '0.1em',
                  fontWeight: 700,
                  background: provider === p.id ? 'rgba(34, 0, 255, 0.3)' : 'rgba(255,255,255,0.05)',
                  border: provider === p.id ? '1px solid rgba(34, 0, 255, 0.5)' : '1px solid rgba(255,255,255,0.08)',
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
            <input
              type="password"
              value={apiKey}
              onChange={e => setApiKey(e.target.value)}
              placeholder={`Paste your ${PROVIDERS.find(p => p.id === provider)?.label} API key...`}
              className="w-full bg-white/5 border border-white/10 rounded-xl px-4 py-3 text-white text-sm outline-none transition-all duration-300 placeholder:text-white/25 focus:border-[#2200FF] focus:shadow-[0_0_20px_rgba(34,0,255,0.2)]"
              style={{ fontFamily: "'Patrick Hand SC', cursive", letterSpacing: '0.08em', fontWeight: 700 }}
            />
          )}
        </div>

        <div className="flex gap-3 pt-2">
          <button
            onClick={onClose}
            className="flex-1 py-3 rounded-xl text-sm text-white/60 uppercase transition-all duration-300"
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
            className="flex-1 py-3 rounded-xl text-sm text-white uppercase transition-all duration-300"
            style={{
              fontFamily: "'Patrick Hand SC', cursive",
              letterSpacing: '0.12em',
              fontWeight: 700,
              background: 'linear-gradient(135deg, #2200FF, #4422FF)',
              boxShadow: '0 4px 20px rgba(34, 0, 255, 0.3)',
            }}
            onMouseEnter={e => { e.currentTarget.style.boxShadow = '0 8px 30px rgba(34, 0, 255, 0.5)' }}
            onMouseLeave={e => { e.currentTarget.style.boxShadow = '0 4px 20px rgba(34, 0, 255, 0.3)' }}
          >
            SAVE
          </button>
        </div>
      </div>
    </div>
  )
}
