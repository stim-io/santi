import { baseUrl } from './env'

export type ErrorBody = { error?: { message?: string } }

export async function createSession(): Promise<{ id: string }> {
  const response = await fetch(`${baseUrl()}/api/v1/sessions`, {
    method: 'POST'
  })
  if (!response.ok) {
    throw new Error(`create session failed: ${response.status} ${await response.text()}`)
  }
  return response.json()
}

export async function getSession(
  sessionId: string
): Promise<{ id: string; parent_session_id: string | null; fork_point: number | null; created_at: string }> {
  const response = await fetch(`${baseUrl()}/api/v1/sessions/${sessionId}`)

  if (!response.ok) {
    throw new Error(`get session failed: ${response.status} ${await response.text()}`)
  }

  return response.json()
}

export async function listSessionEffects(
  sessionId: string
): Promise<{
  effects: Array<{
    id: string
    session_id: string
    effect_type: string
    idempotency_key: string
    status: string
    source_hook_id: string
    source_turn_id: string
    result_ref: string | null
    error_text: string | null
    created_at: string
    updated_at: string
  }>
}> {
  const response = await fetch(`${baseUrl()}/api/v1/sessions/${sessionId}/effects`)

  if (!response.ok) {
    throw new Error(`list session effects failed: ${response.status} ${await response.text()}`)
  }

  return response.json()
}

export async function reloadHooks(hooks: Array<{
  id: string
  enabled: boolean
  hook_point: 'turn_completed'
  kind: 'compact_threshold' | 'compact_handoff' | 'fork_handoff_threshold'
  params: Record<string, unknown>
}>): Promise<{ hook_count: number }> {
  const response = await fetch(`${baseUrl()}/api/v1/admin/hooks`, {
    method: 'PUT',
    headers: {
      'content-type': 'application/json'
    },
    body: JSON.stringify({ hooks })
  })

  if (!response.ok) {
    throw new Error(`reload hooks failed: ${response.status} ${await response.text()}`)
  }

  return response.json()
}

export async function forkSession(
  sessionId: string,
  forkPoint: number,
  requestId: string
): Promise<{ new_session_id: string; parent_session_id: string; fork_point: number }> {
  const response = await fetch(`${baseUrl()}/api/v1/sessions/${sessionId}/fork`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json'
    },
    body: JSON.stringify({
      fork_point: forkPoint,
      request_id: requestId
    })
  })

  if (!response.ok) {
    throw new Error(`fork session failed: ${response.status} ${await response.text()}`)
  }

  return response.json()
}

export async function forkSessionRaw(
  sessionId: string,
  forkPoint: number,
  requestId: string
): Promise<Response> {
  return fetch(`${baseUrl()}/api/v1/sessions/${sessionId}/fork`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json'
    },
    body: JSON.stringify({
      fork_point: forkPoint,
      request_id: requestId
    })
  })
}

export async function compactSession(
  sessionId: string,
  summary: string
): Promise<{ id: string; turn_id: string; summary: string }> {
  const response = await fetch(`${baseUrl()}/api/v1/sessions/${sessionId}/compact`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json'
    },
    body: JSON.stringify({ summary })
  })

  if (!response.ok) {
    throw new Error(`compact session failed: ${response.status} ${await response.text()}`)
  }

  return response.json()
}

export async function compactSessionRaw(sessionId: string, summary: string): Promise<Response> {
  return fetch(`${baseUrl()}/api/v1/sessions/${sessionId}/compact`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json'
    },
    body: JSON.stringify({ summary })
  })
}

export async function sendSession(sessionId: string, text: string): Promise<Response> {
  const response = await fetch(`${baseUrl()}/api/v1/sessions/${sessionId}/send`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json'
    },
    body: JSON.stringify({
      content: [{ type: 'text', text }]
    })
  })

  if (!response.ok) {
    throw new Error(`send session failed: ${response.status} ${await response.text()}`)
  }

  return response
}

export async function readErrorMessage(response: Response): Promise<string> {
  const raw = await response.text()

  try {
    const parsed = JSON.parse(raw) as ErrorBody
    return parsed.error?.message ?? raw
  } catch {
    return raw
  }
}
