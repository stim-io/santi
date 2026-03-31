import { createSession, getSession, listSessionEffects, reloadHooks, sendSession } from '../../lib/client'
import { readSseData } from '../../lib/sse'

async function waitForEffect(
  sessionId: string,
  effectType: string,
  attempts = 20
): Promise<{
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
}> {
  for (let i = 0; i < attempts; i += 1) {
    const effects = await listSessionEffects(sessionId)
    const effect = effects.effects.find((item) => item.effect_type === effectType)
    if (effect && (effect.status === 'completed' || effect.status === 'failed')) {
      return effect
    }
    await new Promise((resolve) => setTimeout(resolve, 500))
  }

  throw new Error(`effect ${effectType} not completed in time`)
}

describe('hook fork handoff', () => {
  it('forks and records a completed effect when threshold is hit', async () => {
    await reloadHooks([
      {
        id: 'fork-handoff-threshold-e2e',
        enabled: true,
        hook_point: 'turn_completed',
        kind: 'fork_handoff_threshold',
        params: {
          min_messages_since_last_compact: 1,
          seed_text:
            'Recommend to use compact before continuing. <santi-meta effect="hook_fork_handoff"></santi-meta>'
        }
      }
    ])

    const session = await createSession()
    const response = await sendSession(session.id, 'trigger hook fork handoff')
    const events = await readSseData(response)

    expect(events.some((event) => event.includes('response.completed'))).toBe(true)

    const effect = await waitForEffect(session.id, 'hook_fork_handoff')

    expect(effect.status).toBe('completed')
    expect(effect.result_ref).toMatch(/^sess_/) 

    const childSession = await getSession(effect.result_ref!)
    expect(childSession.parent_session_id).toBe(session.id)
    expect(childSession.fork_point).not.toBeNull()

    await reloadHooks([])
  })
})
