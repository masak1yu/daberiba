import { type MatrixClient } from 'matrix-js-sdk'

// Matrix sync レスポンスの最小型定義
export interface MatrixEvent {
  type: string
  event_id?: string
  sender?: string
  origin_server_ts?: number
  content: Record<string, unknown>
  state_key?: string
}

export interface JoinedRoom {
  timeline: {
    events: MatrixEvent[]
    limited?: boolean
    prev_batch?: string
  }
  state: { events: MatrixEvent[] }
  account_data: { events: MatrixEvent[] }
  ephemeral: { events: MatrixEvent[] }
  unread_notifications: {
    notification_count?: number
    highlight_count?: number
  }
}

export interface SyncResponse {
  next_batch: string
  rooms: {
    join: Record<string, JoinedRoom>
    leave?: Record<string, unknown>
  }
}

/**
 * PUT /_matrix/client/v3/rooms/{roomId}/typing/{userId}
 * typing=true のとき timeout_ms ミリ秒後に自動クリアされる（サーバー側）
 */
export async function sendTyping(
  homeserver: string,
  token: string,
  roomId: string,
  userId: string,
  typing: boolean,
  timeoutMs = 10_000
): Promise<void> {
  const body = typing ? { typing: true, timeout: timeoutMs } : { typing: false }
  await fetch(
    `${homeserver}/_matrix/client/v3/rooms/${encodeURIComponent(roomId)}/typing/${encodeURIComponent(userId)}`,
    {
      method: 'PUT',
      headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    }
  )
}

/** 1回分の /sync リクエスト */
export async function syncOnce(
  client: MatrixClient,
  since?: string,
  timeoutMs = 30_000
): Promise<SyncResponse> {
  const base = client.getHomeserverUrl()
  const token = client.getAccessToken()
  const params = new URLSearchParams({ timeout: String(timeoutMs) })
  if (since) params.set('since', since)

  const res = await fetch(`${base}/_matrix/client/v3/sync?${params}`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  if (!res.ok) throw new Error(`sync failed: ${res.status}`)
  return res.json() as Promise<SyncResponse>
}

/**
 * 継続的な sync ループを開始する。
 * 返り値の関数を呼ぶと停止する。
 */
export function startSyncLoop(
  client: MatrixClient,
  onUpdate: (data: SyncResponse) => void,
  onError?: (err: unknown) => void
): () => void {
  let active = true
  let since: string | undefined

  async function loop() {
    while (active) {
      try {
        const data = await syncOnce(client, since)
        since = data.next_batch
        if (active) onUpdate(data)
      } catch (err) {
        if (!active) break
        onError?.(err)
        // エラー後は 3 秒待ってリトライ
        await new Promise((r) => setTimeout(r, 3_000))
      }
    }
  }

  loop()
  return () => {
    active = false
  }
}
