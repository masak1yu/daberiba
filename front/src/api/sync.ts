import { type MatrixClient } from 'matrix-js-sdk'

// Matrix sync レスポンスの最小型定義
export interface MatrixEvent {
  type: string
  event_id?: string
  sender?: string
  origin_server_ts?: number
  content: Record<string, unknown>
  state_key?: string
  /** m.room.redaction イベントが削除対象 event_id を持つ */
  redacts?: string
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
 * - バックグラウンド復帰時（visibilitychange）に即リトライ
 * 返り値の関数を呼ぶと停止する。
 */
export function startSyncLoop(
  client: MatrixClient,
  onUpdate: (data: SyncResponse) => void,
  onError?: (err: unknown) => void
): () => void {
  let active = true
  let since: string | undefined
  // バックグラウンド復帰を通知するための resolve 関数
  let wakeResolve: (() => void) | null = null

  function onVisibilityChange() {
    if (document.visibilityState === 'visible' && wakeResolve) {
      wakeResolve()
      wakeResolve = null
    }
  }
  document.addEventListener('visibilitychange', onVisibilityChange)

  async function loop() {
    while (active) {
      try {
        // バックグラウンド中は short timeout で polling（長時間 fetch でハングしない）
        const timeout = document.visibilityState === 'hidden' ? 5_000 : 30_000
        const data = await syncOnce(client, since, timeout)
        since = data.next_batch
        if (active) onUpdate(data)
      } catch (err) {
        if (!active) break
        onError?.(err)
        // バックグラウンド時は visibilitychange まで待機、フォアグラウンド時は 3s 待ってリトライ
        if (document.visibilityState === 'hidden') {
          await new Promise<void>((resolve) => {
            wakeResolve = resolve
            // 最大 60s 待機してもフォアグラウンドに戻らなければリトライ
            setTimeout(resolve, 60_000)
          })
        } else {
          await new Promise((r) => setTimeout(r, 3_000))
        }
      }
    }
  }

  loop()
  return () => {
    active = false
    document.removeEventListener('visibilitychange', onVisibilityChange)
  }
}
