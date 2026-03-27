/**
 * パブリックルーム API — 一覧取得・参加
 */

export interface PublicRoom {
  room_id: string
  name?: string
  topic?: string
  num_joined_members: number
  world_readable: boolean
  guest_can_join: boolean
  avatar_url?: string
}

export interface PublicRoomsResponse {
  chunk: PublicRoom[]
  next_batch?: string
  total_room_count_estimate?: number
}

/** GET /_matrix/client/v3/publicRooms */
export async function fetchPublicRooms(
  homeserver: string,
  token: string,
  filter?: string,
  limit = 20
): Promise<PublicRoomsResponse> {
  const params = new URLSearchParams({ limit: String(limit) })
  if (filter) params.set('filter', filter)
  const res = await fetch(`${homeserver}/_matrix/client/v3/publicRooms?${params}`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  if (!res.ok) throw new Error(`publicRooms failed: ${res.status}`)
  return res.json() as Promise<PublicRoomsResponse>
}

/** POST /_matrix/client/v3/join/{roomId} */
export async function joinRoom(homeserver: string, token: string, roomId: string): Promise<string> {
  const res = await fetch(`${homeserver}/_matrix/client/v3/join/${encodeURIComponent(roomId)}`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: '{}',
  })
  if (!res.ok) {
    const err = (await res.json().catch(() => ({}))) as { error?: string }
    throw new Error(err.error ?? `join failed: ${res.status}`)
  }
  const data = (await res.json()) as { room_id: string }
  return data.room_id
}
