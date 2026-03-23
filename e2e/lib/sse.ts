export async function readSseData(response: Response): Promise<string[]> {
  const reader = response.body?.getReader()
  if (!reader) throw new Error('missing response body')

  const decoder = new TextDecoder()
  let buffer = ''
  const data: string[] = []

  while (true) {
    const { done, value } = await reader.read()
    if (done) break

    buffer += decoder.decode(value, { stream: true })

    while (true) {
      const idx = buffer.indexOf('\n\n')
      if (idx === -1) break
      const frame = buffer.slice(0, idx)
      buffer = buffer.slice(idx + 2)

      for (const line of frame.split('\n')) {
        if (line.startsWith('data: ')) {
          data.push(line.slice(6))
        }
      }
    }
  }

  return data
}
