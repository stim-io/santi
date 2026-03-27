import { createSession, sendSession } from '../../lib/client'
import { readSseData } from '../../lib/sse'

describe('session send', () => {
  it('completes the session send SSE flow', async () => {
    // This is the first black-box path we want to stabilize.
    // It should eventually cover:
    // create session -> send session -> real local provider -> SSE -> persistence.
    // For now the test intentionally asserts only the externally visible SSE contract.

    const session = await createSession()
    expect(session.id).toMatch(/^sess_/) 

    const response = await sendSession(session.id, 'hello from smoke')
    expect(response.headers.get('content-type') ?? '').toContain('text/event-stream')

    const events = await readSseData(response)

    // If this fails, first inspect tracing around:
    // - session send started
    // - user message persisted
    // - provider request dispatched
    // - session send completed
    expect(events.some((event) => event.includes('response.completed'))).toBe(true)
    expect(events.at(-1)).toBe('[DONE]')
  })
})
