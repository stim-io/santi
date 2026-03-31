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
