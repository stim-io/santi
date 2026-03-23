import { baseUrl } from './env'

export async function createSession(): Promise<{ id: string }> {
  const response = await fetch(`${baseUrl()}/api/v1/sessions`, {
    method: 'POST'
  })
  if (!response.ok) {
    throw new Error(`create session failed: ${response.status} ${await response.text()}`)
  }
  return response.json()
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
