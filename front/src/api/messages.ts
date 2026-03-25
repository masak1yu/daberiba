/**
 * GET /rooms/{roomId}/messages — 過去ログ取得
 */
import type { MatrixEvent } from './sync'

export interface MessagesResponse {
  chunk: MatrixEvent[]
  start: string
  end?: string // undefined = これ以上遡れない
}

/**
 * dir=b で roomId の過去メッセージを取得する。
 * chunk は新しい順（返ってきたまま）なので呼び出し側で逆順にすること。
 */
export async function fetchHistory(
  homeserver: string,
  token: string,
  roomId: string,
  from: string,
  limit = 30
): Promise<MessagesResponse> {
  const params = new URLSearchParams({ from, dir: 'b', limit: String(limit) })
  const res = await fetch(
    `${homeserver}/_matrix/client/v3/rooms/${encodeURIComponent(roomId)}/messages?${params}`,
    { headers: { Authorization: `Bearer ${token}` } }
  )
  if (!res.ok) throw new Error(`messages failed: ${res.status}`)
  return res.json() as Promise<MessagesResponse>
}

/**
 * POST /rooms/{roomId}/receipt/m.read/{eventId} — 既読送信
 */
export async function sendReadReceipt(
  homeserver: string,
  token: string,
  roomId: string,
  eventId: string
): Promise<void> {
  const url = `${homeserver}/_matrix/client/v3/rooms/${encodeURIComponent(roomId)}/receipt/m.read/${encodeURIComponent(eventId)}`
  await fetch(url, {
    method: 'POST',
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: '{}',
  })
}
