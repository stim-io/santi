#!/usr/bin/env node

function parseArgs(argv) {
  const args = {
    sessionId: null,
    create: false,
    raw: false,
    wait: false,
  }

  for (const arg of argv) {
    if (arg === '--create') {
      args.create = true
    } else if (arg === '--raw') {
      args.raw = true
    } else if (arg === '--wait') {
      args.wait = true
    } else if (!args.sessionId) {
      args.sessionId = arg
    } else {
      throw new Error(`unexpected argument: ${arg}`)
    }
  }

  return args
}

async function readStdin() {
  const chunks = []
  for await (const chunk of process.stdin) {
    chunks.push(typeof chunk === 'string' ? Buffer.from(chunk) : chunk)
  }
  return Buffer.concat(chunks).toString('utf8')
}

function baseUrl() {
  return process.env.SANTI_BASE_URL ?? 'http://127.0.0.1:18081'
}

async function createSession(url) {
  const response = await fetch(`${url}/api/v1/sessions`, {
    method: 'POST',
  })

  if (!response.ok) {
    throw new Error((await response.text()) || `HTTP ${response.status}`)
  }

  const body = await response.json()
  if (!body?.id) {
    throw new Error('missing session id in create response')
  }

  process.stdout.write(`${body.id}\n`)
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

function readErrorMessage(bodyText) {
  if (!bodyText) return null

  try {
    const parsed = JSON.parse(bodyText)
    return parsed?.error?.message ?? null
  } catch {
    return bodyText
  }
}

async function postSend(url, sessionId, content) {
  return fetch(`${url}/api/v1/sessions/${sessionId}/send`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      content: [{ type: 'text', text: content }],
    }),
  })
}

async function sendMessage(url, sessionId, content, raw, wait) {
  let warnedBusy = false

  while (true) {
    const response = await postSend(url, sessionId, content)

    if (response.ok) {
      await renderStream(response, raw)
      return
    }

    const bodyText = (await response.text()).trim()
    const errorMessage = readErrorMessage(bodyText)
    if (response.status === 409) {
      if (!wait) {
        throw new Error(
          `session send already in progress (409). Wait for the current turn to finish, or rerun with --wait.${
            errorMessage ? ` Server said: ${errorMessage}` : ''
          }`,
        )
      }

      if (!warnedBusy) {
        process.stderr.write('session busy; waiting for current turn to finish...\n')
        warnedBusy = true
      }

      await sleep(350)
      continue
    }

    throw new Error(errorMessage || `HTTP ${response.status}`)
  }
}

async function renderStream(response, raw) {
  const reader = response.body?.getReader()
  if (!reader) {
    throw new Error('missing response body')
  }

  const decoder = new TextDecoder()
  let buffer = ''
  let sawText = false

  while (true) {
    const { done, value } = await reader.read()
    if (done) break

    buffer += decoder.decode(value, { stream: true })

    while (true) {
      const idx = buffer.indexOf('\n')
      if (idx === -1) break

      const line = buffer.slice(0, idx).trim()
      buffer = buffer.slice(idx + 1)

      if (!line.startsWith('data: ')) continue

      const payload = line.slice(6)
      if (payload === '[DONE]') continue

      if (raw) {
        process.stdout.write(`${payload}\n`)
        continue
      }

      let event
      try {
        event = JSON.parse(payload)
      } catch {
        process.stderr.write(`${payload}\n`)
        process.exitCode = 1
        return
      }

      const eventType = event?.type
      if (eventType === 'response.output_text.delta') {
        const delta = event?.delta ?? ''
        if (delta) {
          process.stdout.write(delta)
          sawText = true
        }
      } else if (eventType === 'error') {
        process.stderr.write(`${JSON.stringify(event)}\n`)
        process.exitCode = 1
        return
      }
    }
  }

  if (!raw && sawText) {
    process.stdout.write('\n')
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  const url = baseUrl()

  if (args.create) {
    if (args.sessionId) {
      throw new Error('session_id should not be provided with --create')
    }
    await createSession(url)
    return
  }

  if (!args.sessionId) {
    throw new Error("usage: printf 'hello' | ./scripts/dev/send.mjs <session_id> [--raw] [--wait]")
  }

  const content = await readStdin()
  if (!content) {
    throw new Error('expected stdin content')
  }

  await sendMessage(url, args.sessionId, content, args.raw, args.wait)
}

main().catch((error) => {
  process.stderr.write(`${error.message}\n`)
  process.exitCode = 1
})
