/**
 * ルーム操作 API — createRoom, fetchMembers
 */

export interface CreateRoomResponse {
  room_id: string
}

export interface RoomMember {
  userId: string
  displayName?: string
  membership: string
}

/** POST /_matrix/client/v3/createRoom */
export async function createRoom(
  homeserver: string,
  token: string,
  name: string,
  preset: 'private_chat' | 'public_chat' | 'trusted_private_chat' = 'private_chat'
): Promise<CreateRoomResponse> {
  const res = await fetch(`${homeserver}/_matrix/client/v3/createRoom`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, preset }),
  })
  if (!res.ok) {
    const err = (await res.json().catch(() => ({}))) as { error?: string }
    throw new Error(err.error ?? `createRoom failed: ${res.status}`)
  }
  return res.json() as Promise<CreateRoomResponse>
}

/** GET /_matrix/client/v3/rooms/{roomId}/members */
export async function fetchMembers(
  homeserver: string,
  token: string,
  roomId: string
): Promise<RoomMember[]> {
  const res = await fetch(
    `${homeserver}/_matrix/client/v3/rooms/${encodeURIComponent(roomId)}/members`,
    { headers: { Authorization: `Bearer ${token}` } }
  )
  if (!res.ok) throw new Error(`members failed: ${res.status}`)
  const data = (await res.json()) as {
    chunk: { state_key: string; content: { membership: string; displayname?: string } }[]
  }
  return data.chunk
    .filter((ev) => ev.content.membership === 'join')
    .map((ev) => ({
      userId: ev.state_key,
      displayName: ev.content.displayname,
      membership: ev.content.membership,
    }))
}
