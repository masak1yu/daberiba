/**
 * ルーム一覧・タイムラインストア（Zustand）
 *
 * sync レスポンスを処理してルーム情報とタイムラインを更新する。
 */
import { create } from 'zustand'
import type { MatrixEvent, SyncResponse } from '../api/sync'

export interface RoomSummary {
  roomId: string
  name?: string
  lastEvent?: MatrixEvent
  notificationCount: number
  highlightCount: number
}

interface RoomsState {
  /** 次の sync に使う since トークン */
  since: string | undefined
  rooms: Record<string, RoomSummary>
  /** room_id → タイムラインイベント（昇順） */
  timelines: Record<string, MatrixEvent[]>
  syncing: boolean
  error: string | null
}

interface RoomsActions {
  applySyncResponse: (resp: SyncResponse) => void
  setSyncing: (v: boolean) => void
  setError: (e: string | null) => void
  reset: () => void
}

const INITIAL: RoomsState = {
  since: undefined,
  rooms: {},
  timelines: {},
  syncing: false,
  error: null,
}

export const useRoomsStore = create<RoomsState & RoomsActions>((set, get) => ({
  ...INITIAL,

  applySyncResponse(resp) {
    const { rooms: prev, timelines: prevTimelines } = get()
    const nextRooms = { ...prev }
    const nextTimelines = { ...prevTimelines }

    for (const [roomId, room] of Object.entries(resp.rooms?.join ?? {})) {
      // 状態イベントからルーム名を抽出
      const allEvents = [...(room.state?.events ?? []), ...(room.timeline.events ?? [])]
      const nameEv = allEvents.findLast((e) => e.type === 'm.room.name')
      const name = nameEv ? String((nameEv.content as { name?: string }).name ?? '') : undefined

      // タイムラインはメッセージイベント（state_key なし）のみ
      const msgEvents = (room.timeline.events ?? []).filter((e) => e.state_key === undefined)
      nextTimelines[roomId] = [...(nextTimelines[roomId] ?? []), ...msgEvents]

      nextRooms[roomId] = {
        roomId,
        name: name || prev[roomId]?.name,
        lastEvent: msgEvents.at(-1) ?? prev[roomId]?.lastEvent,
        notificationCount:
          room.unread_notifications?.notification_count ?? prev[roomId]?.notificationCount ?? 0,
        highlightCount:
          room.unread_notifications?.highlight_count ?? prev[roomId]?.highlightCount ?? 0,
      }
    }

    set({ since: resp.next_batch, rooms: nextRooms, timelines: nextTimelines })
  },

  setSyncing: (v) => set({ syncing: v }),
  setError: (e) => set({ error: e }),
  reset: () => set({ ...INITIAL }),
}))
