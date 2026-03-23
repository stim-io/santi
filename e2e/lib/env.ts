export function requireEnv(name: string): string {
  const value = process.env[name]
  if (!value) throw new Error(`missing env: ${name}`)
  return value
}

export function baseUrl(): string {
  return process.env.SANTI_BASE_URL ?? 'http://127.0.0.1:8080'
}
