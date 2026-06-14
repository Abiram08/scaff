export interface Session {
  id: string
  query: string
  status: string
  findings: any[]
  synthesis: any
  createdAt: Date
  input_tokens?: number
  output_tokens?: number
}

export interface TokenStats {
  total_calls: number
  total_input_tokens: number
  total_output_tokens: number
  total_tokens: number
  estimated_cost_usd: number
  calls: Array<{
    provider: string
    model: string
    input_tokens: number
    output_tokens: number
    stage: string
    duration_ms: number
    timestamp: string
  }>
}
