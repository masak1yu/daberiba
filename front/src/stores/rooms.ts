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

/**
 * リアクション集計: eventId → { emoji → sender[] }
 * sender[] は重複排除（同一ユーザーが同じ絵文字を複数回送っても 1 カウント）
 */
export type Reactions = Record<string, Record<string, string[]>>

interface RoomsState {
  /** 次の sync に使う since トークン */
  since: string | undefined
  rooms: Record<string, RoomSummary>
  /** room_id → タイムラインイベント（昇順）— m.reaction は含まない */
  timelines: Record<string, MatrixEvent[]>
  /** room_id → /messages 遡り用 prev_batch トークン（undefined = これ以上ない） */
  prevBatches: Record<string, string | undefined>
  /** room_id → 過去ログ読み込み中フラグ */
  historyLoading: Record<string, boolean>
  /** room_id → eventId → { emoji → senders } */
  reactions: Record<string, Reactions>
  /** room_id → 現在タイピング中のユーザー ID 一覧 */
  typing: Record<string, string[]>
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
  reactions: {},
  typing: {},
  syncing: false,
  error: null,
}

/** m.reaction イベントからリアクションを集計して既存の Reactions にマージする */
function mergeReactions(base: Reactions, events: MatrixEvent[]): Reactions {
  const next = { ...base }
  for (const ev of events) {
    if (ev.type !== 'm.reaction') continue
    const rel = (ev.content as { 'm.relates_to'?: { event_id?: string; key?: string } })['m.relates_to']
    if (!rel?.event_id || !rel.key || !ev.sender) continue

    const targetId = rel.event_id
    const emoji = rel.key
    const sender = ev.sender

    const perEvent = { ...(next[targetId] ?? {}) }
    const senders = perEvent[emoji] ?? []
    if (!senders.includes(sender)) {
      perEvent[emoji] = [...senders, sender]
    }
    next[targetId] = perEvent
  }
  return next
}

export const useRoomsStore = create<RoomsState & RoomsActions>((set, get) => ({
  ...INITIAL,

  applySyncResponse(resp) {
    const { rooms: prev, timelines: prevTimelines, prevBatches: prevPB, reactions: prevReactions, typing: prevTyping } = get()
    const nextRooms = { ...prev }
    const nextTimelines = { ...prevTimelines }
    const nextPrevBatches = { ...prevPB }
    const nextReactions = { ...prevReactions }
    const nextTyping = { ...prevTyping }

    for (const [roomId, room] of Object.entries(resp.rooms?.join ?? {})) {
      // 状態イベントからルーム名を抽出
      const allEvents = [...(room.state?.events ?? []), ...(room.timeline.events ?? [])]
      const nameEv = allEvents.findLast((e) => e.type === 'm.room.name')
      const name = nameEv ? String((nameEv.content as { name?: string }).name ?? '') : undefined

      const timelineEvents = room.timeline.events ?? []

      // m.reaction をリアクション集計へ、それ以外のメッセージイベントをタイムラインへ
      const reactionEvents = timelineEvents.filter((e) => e.type === 'm.reaction')
      const msgEvents = timelineEvents.filter((e) => e.state_key === undefined && e.type !== 'm.reaction')

      nextTimelines[roomId] = [...(nextTimelines[roomId] ?? []), ...msgEvents]
      nextReactions[roomId] = mergeReactions(nextReactions[roomId] ?? {}, reactionEvents)

      // ephemeral: m.typing イベントからタイピング中ユーザーを更新
      const typingEv = (room.ephemeral?.events ?? []).find((e) => e.type === 'm.typing')
      if (typingEv) {
        nextTyping[roomId] = (typingEv.content as { user_ids?: string[] }).user_ids ?? []
      }

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

    set({
      since: resp.next_batch,
      rooms: nextRooms,
      timelines: nextTimelines,
      prevBatches: nextPrevBatches,
      reactions: nextReactions,
      typing: nextTyping,
    })
  },

  async loadHistory(roomId) {
    const { prevBatches, historyLoading, timelines, reactions } = get()
    const from = prevBatches[roomId]
    if (!from || historyLoading[roomId]) return

    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return

    set((s) => ({ historyLoading: { ...s.historyLoading, [roomId]: true } }))
    try {
      const resp = await fetchHistory(homeserver, token, roomId, from)
      // chunk は新しい順 → 逆順にして先頭に追加
      const reversed = [...resp.chunk].reverse()
      const newMsgEvents = reversed.filter((e) => e.state_key === undefined && e.type !== 'm.reaction')
      const newReactionEvents = reversed.filter((e) => e.type === 'm.reaction')
      set((s) => ({
        timelines: { ...s.timelines, [roomId]: [...newMsgEvents, ...(timelines[roomId] ?? [])] },
        prevBatches: { ...s.prevBatches, [roomId]: resp.end },
        reactions: {
          ...s.reactions,
          [roomId]: mergeReactions(reactions[roomId] ?? {}, newReactionEvents),
        },
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
