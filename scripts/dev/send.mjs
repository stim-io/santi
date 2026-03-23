#!/usr/bin/env node

function parseArgs(argv) {
  const args = {
    sessionId: null,
    create: false,
    raw: false,
  }

  for (const arg of argv) {
    if (arg === '--create') {
      args.create = true
    } else if (arg === '--raw') {
      args.raw = true
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

async function sendMessage(url, sessionId, content, raw) {
  const response = await fetch(`${url}/api/v1/sessions/${sessionId}/send`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      content: [{ type: 'text', text: content }],
    }),
  })

  if (!response.ok) {
    throw new Error((await response.text()) || `HTTP ${response.status}`)
  }

  await renderStream(response, raw)
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
    throw new Error("usage: printf 'hello' | ./scripts/dev/send.mjs <session_id>")
  }

  const content = await readStdin()
  if (!content) {
    throw new Error('expected stdin content')
  }

  await sendMessage(url, args.sessionId, content, args.raw)
}

main().catch((error) => {
  process.stderr.write(`${error.message}\n`)
  process.exitCode = 1
})
