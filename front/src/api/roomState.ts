/**
 * ルーム状態イベント送信 API
 */

/** PUT /_matrix/client/v3/rooms/{roomId}/state/{eventType} */
async function putRoomState(
  homeserver: string,
  token: string,
  roomId: string,
  eventType: string,
  content: Record<string, unknown>,
  stateKey = ''
): Promise<void> {
  const url = stateKey
    ? `${homeserver}/_matrix/client/v3/rooms/${encodeURIComponent(roomId)}/state/${eventType}/${encodeURIComponent(stateKey)}`
    : `${homeserver}/_matrix/client/v3/rooms/${encodeURIComponent(roomId)}/state/${eventType}`
  const res = await fetch(url, {
    method: 'PUT',
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: JSON.stringify(content),
  })
  if (!res.ok) {
    const err = (await res.json().catch(() => ({}))) as { error?: string }
    throw new Error(err.error ?? `putRoomState failed: ${res.status}`)
  }
}

export async function setRoomName(
  homeserver: string,
  token: string,
  roomId: string,
  name: string
): Promise<void> {
  return putRoomState(homeserver, token, roomId, 'm.room.name', { name })
}

export async function setRoomTopic(
  homeserver: string,
  token: string,
  roomId: string,
  topic: string
): Promise<void> {
  return putRoomState(homeserver, token, roomId, 'm.room.topic', { topic })
}

/**
 * PUT /rooms/{roomId}/send/m.reaction/{txnId}
 * m.reaction は state ではなく通常イベントだが便宜上ここに置く
 */
export async function sendReaction(
  homeserver: string,
  token: string,
  roomId: string,
  targetEventId: string,
  emoji: string
): Promise<void> {
  const txnId = `react.${Date.now()}.${Math.random().toString(36).slice(2)}`
  const url = `${homeserver}/_matrix/client/v3/rooms/${encodeURIComponent(roomId)}/send/m.reaction/${txnId}`
  const res = await fetch(url, {
    method: 'PUT',
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({
      'm.relates_to': { rel_type: 'm.annotation', event_id: targetEventId, key: emoji },
    }),
  })
  if (!res.ok) {
    const err = (await res.json().catch(() => ({}))) as { error?: string }
    throw new Error(err.error ?? `sendReaction failed: ${res.status}`)
  }
}
