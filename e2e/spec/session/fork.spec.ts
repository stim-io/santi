import {
  compactSession,
  compactSessionRaw,
  createSession,
  forkSession,
  forkSessionRaw,
  getSession,
  readErrorMessage,
  sendSession
} from '../../lib/client'
import { readSseData } from '../../lib/sse'

describe('session fork', () => {
  it('returns stable lineage fields and idempotent result', async () => {
    const session = await createSession()
    const response = await sendSession(session.id, 'seed forkable session')
    const events = await readSseData(response)

    expect(events.some((event) => event.includes('response.completed'))).toBe(true)

    const first = await forkSession(session.id, 1, 'req_fork_idempotent')
    const second = await forkSession(session.id, 1, 'req_fork_idempotent')

    expect(first.parent_session_id).toBe(session.id)
    expect(first.fork_point).toBe(1)
    expect(first.new_session_id).toMatch(/^sess_/)
    expect(second).toEqual(first)

    const childSession = await getSession(first.new_session_id)
    expect(childSession.parent_session_id).toBe(session.id)
    expect(childSession.fork_point).toBe(1)
  })

  it('keeps child session usable after fork and allows later compact on child-owned messages', async () => {
    const session = await createSession()
    const response = await sendSession(session.id, 'seed child session')
    const events = await readSseData(response)

    expect(events.some((event) => event.includes('response.completed'))).toBe(true)

    const fork = await forkSession(session.id, 1, 'req_fork_compact_child')

    const childSend = await sendSession(fork.new_session_id, 'child still works')
    const childEvents = await readSseData(childSend)

    expect(childEvents.some((event) => event.includes('response.completed'))).toBe(true)

    const compact = await compactSession(fork.new_session_id, 'child compact summary')

    expect(compact.summary).toBe('child compact summary')
  })

  it('rejects invalid fork point with a stable 400 contract', async () => {
    const session = await createSession()
    const response = await sendSession(session.id, 'seed invalid fork point case')
    const events = await readSseData(response)

    expect(events.some((event) => event.includes('response.completed'))).toBe(true)

    const forkResponse = await forkSessionRaw(session.id, 999, 'req_invalid_fork_point')
    const message = await readErrorMessage(forkResponse)

    expect(forkResponse.status).toBe(400)
    expect(message).toContain('illegal fork_point')
  })

  it('returns 409 when fork races with an in-flight send on the same parent session', async () => {
    const session = await createSession()
    const stream = await sendSession(session.id, 'hold the session lock for fork conflict')

    const forkResponse = await forkSessionRaw(session.id, 1, 'req_busy_fork')
    const message = await readErrorMessage(forkResponse)

    expect(forkResponse.status).toBe(409)
    expect(message).toContain('fork already in progress')

    const events = await readSseData(stream)
    expect(events.some((event) => event.includes('response.completed'))).toBe(true)
  })

  it('returns 409 when compact races with an in-flight send on the same session', async () => {
    const session = await createSession()
    const stream = await sendSession(session.id, 'hold the session lock for compact conflict')

    const compactResponse = await compactSessionRaw(session.id, 'competing compact summary')
    const message = await readErrorMessage(compactResponse)

    expect(compactResponse.status).toBe(409)
    expect(message).toContain('compact already in progress')

    const events = await readSseData(stream)
    expect(events.some((event) => event.includes('response.completed'))).toBe(true)
  })
})
