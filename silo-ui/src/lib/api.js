const BASE = '/api'

export async function listGames() {
  const r = await fetch(`${BASE}/catalog`)
  if (!r.ok) throw new Error(await r.text())
  return r.json()
}

export async function getGame(id) {
  const r = await fetch(`${BASE}/catalog/${id}`)
  if (!r.ok) throw new Error(await r.text())
  return r.json()
}

export async function patchGame(id, fields) {
  const r = await fetch(`${BASE}/catalog/${id}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(fields),
  })
  if (!r.ok) throw new Error(await r.text())
  return r.json()
}

export async function deleteGame(id) {
  const r = await fetch(`${BASE}/catalog/${id}`, { method: 'DELETE' })
  if (!r.ok) throw new Error(await r.text())
}

export async function startPull(gameId) {
  const r = await fetch(`${BASE}/pull/${gameId}`, { method: 'POST' })
  if (!r.ok) throw new Error(await r.text())
  return r.json() // { job_id }
}

/**
 * Subscribe to SSE pull progress events.
 * @param {string} jobId
 * @param {(event: {stage: string, message: string, run_game_id?: number}) => void} onEvent
 * @returns {EventSource}
 */
export function subscribePull(jobId, onEvent) {
  const es = new EventSource(`${BASE}/pull/${jobId}/events`)
  es.onmessage = (e) => {
    try {
      onEvent(JSON.parse(e.data))
    } catch {}
  }
  return es
}
