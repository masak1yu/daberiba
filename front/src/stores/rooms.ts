/**
 * ルーム一覧・タイムラインストア（Zustand）
 *
 * sync レスポンスを処理してルーム情報とタイムラインを更新する。
 */
import { create } from 'zustand'
import type { MatrixEvent, SyncResponse } from '../api/sync'
import { fetchHistory } from '../api/messages'
import { STORAGE_KEY } from '../api/client'

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
  /** room_id → /messages 遡り用 prev_batch トークン（undefined = これ以上ない） */
  prevBatches: Record<string, string | undefined>
  /** room_id → 過去ログ読み込み中フラグ */
  historyLoading: Record<string, boolean>
  syncing: boolean
  error: string | null
}

interface RoomsActions {
  applySyncResponse: (resp: SyncResponse) => void
  loadHistory: (roomId: string) => Promise<void>
  setSyncing: (v: boolean) => void
  setError: (e: string | null) => void
  reset: () => void
}

const INITIAL: RoomsState = {
  since: undefined,
  rooms: {},
  timelines: {},
  prevBatches: {},
  historyLoading: {},
  syncing: false,
  error: null,
}

export const useRoomsStore = create<RoomsState & RoomsActions>((set, get) => ({
  ...INITIAL,

  applySyncResponse(resp) {
    const { rooms: prev, timelines: prevTimelines, prevBatches: prevPB } = get()
    const nextRooms = { ...prev }
    const nextTimelines = { ...prevTimelines }
    const nextPrevBatches = { ...prevPB }

    for (const [roomId, room] of Object.entries(resp.rooms?.join ?? {})) {
      // 状態イベントからルーム名を抽出
      const allEvents = [...(room.state?.events ?? []), ...(room.timeline.events ?? [])]
      const nameEv = allEvents.findLast((e) => e.type === 'm.room.name')
      const name = nameEv ? String((nameEv.content as { name?: string }).name ?? '') : undefined

      // タイムラインはメッセージイベント（state_key なし）のみ
      const msgEvents = (room.timeline.events ?? []).filter((e) => e.state_key === undefined)
      nextTimelines[roomId] = [...(nextTimelines[roomId] ?? []), ...msgEvents]

      // limited=true のときだけ prev_batch を保存（= 過去ログが遡れる）
      if (room.timeline.limited && room.timeline.prev_batch) {
        nextPrevBatches[roomId] = room.timeline.prev_batch
      }

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

    set({ since: resp.next_batch, rooms: nextRooms, timelines: nextTimelines, prevBatches: nextPrevBatches })
  },

  async loadHistory(roomId) {
    const { prevBatches, historyLoading, timelines } = get()
    const from = prevBatches[roomId]
    if (!from || historyLoading[roomId]) return

    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return

    set((s) => ({ historyLoading: { ...s.historyLoading, [roomId]: true } }))
    try {
      const resp = await fetchHistory(homeserver, token, roomId, from)
      // chunk は新しい順 → 逆順にして先頭に追加
      const newEvents = [...resp.chunk].reverse().filter((e) => e.state_key === undefined)
      set((s) => ({
        timelines: { ...s.timelines, [roomId]: [...newEvents, ...(timelines[roomId] ?? [])] },
        prevBatches: { ...s.prevBatches, [roomId]: resp.end },
      }))
    } catch {
      // 失敗は silent — ユーザーが再度スクロールすればリトライされる
    } finally {
      set((s) => ({ historyLoading: { ...s.historyLoading, [roomId]: false } }))
    }
  },

  setSyncing: (v) => set({ syncing: v }),
  setError: (e) => set({ error: e }),
  reset: () => set({ ...INITIAL }),
}))
