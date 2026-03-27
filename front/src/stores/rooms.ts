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

/** room_id → userId → displayName（未設定なら undefined） */
export type MemberNames = Record<string, string | undefined>

/** room_id → userId → avatar_url mxc URI（未設定なら undefined） */
export type MemberAvatars = Record<string, string | undefined>

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
  /** room_id → userId → displayName */
  memberNames: Record<string, MemberNames>
  /** room_id → userId → avatar_url */
  memberAvatars: Record<string, MemberAvatars>
  syncing: boolean
  error: string | null
}

interface RoomsActions {
  applySyncResponse: (resp: SyncResponse) => void
  loadHistory: (roomId: string) => Promise<void>
  /** ルームを開いたときに未読カウントをローカルでリセットする */
  markRoomRead: (roomId: string) => void
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
  memberNames: {},
  memberAvatars: {},
  syncing: false,
  error: null,
}

/** m.reaction イベントからリアクションを集計して既存の Reactions にマージする */
function mergeReactions(base: Reactions, events: MatrixEvent[]): Reactions {
  const next = { ...base }
  for (const ev of events) {
    if (ev.type !== 'm.reaction') continue
    const rel = (ev.content as { 'm.relates_to'?: { event_id?: string; key?: string } })[
      'm.relates_to'
    ]
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

/** m.room.member イベントから displayName マップを更新する */
function mergeMemberNames(base: MemberNames, events: MatrixEvent[]): MemberNames {
  const next = { ...base }
  for (const ev of events) {
    if (ev.type !== 'm.room.member' || !ev.state_key) continue
    const displayname = (ev.content as { displayname?: string }).displayname
    // displayname がない（または空）場合は既存を維持
    if (displayname) next[ev.state_key] = displayname
  }
  return next
}

/** m.room.member イベントから avatar_url マップを更新する */
function mergeMemberAvatars(base: MemberAvatars, events: MatrixEvent[]): MemberAvatars {
  const next = { ...base }
  for (const ev of events) {
    if (ev.type !== 'm.room.member' || !ev.state_key) continue
    const avatar_url = (ev.content as { avatar_url?: string }).avatar_url
    if (avatar_url) next[ev.state_key] = avatar_url
  }
  return next
}

export const useRoomsStore = create<RoomsState & RoomsActions>((set, get) => ({
  ...INITIAL,

  applySyncResponse(resp) {
    const {
      rooms: prev,
      timelines: prevTimelines,
      prevBatches: prevPB,
      reactions: prevReactions,
      typing: prevTyping,
      memberNames: prevMemberNames,
      memberAvatars: prevMemberAvatars,
    } = get()
    const nextRooms = { ...prev }
    const nextTimelines = { ...prevTimelines }
    const nextPrevBatches = { ...prevPB }
    const nextReactions = { ...prevReactions }
    const nextTyping = { ...prevTyping }
    const nextMemberNames = { ...prevMemberNames }
    const nextMemberAvatars = { ...prevMemberAvatars }

    for (const [roomId, room] of Object.entries(resp.rooms?.join ?? {})) {
      const stateEvents = room.state?.events ?? []
      const timelineEvents = room.timeline.events ?? []
      const allEvents = [...stateEvents, ...timelineEvents]

      // ルーム名
      const nameEv = allEvents.findLast((e) => e.type === 'm.room.name')
      const name = nameEv ? String((nameEv.content as { name?: string }).name ?? '') : undefined

      // displayName / avatar_url マップ（state + timeline の m.room.member から）
      nextMemberNames[roomId] = mergeMemberNames(nextMemberNames[roomId] ?? {}, allEvents)
      nextMemberAvatars[roomId] = mergeMemberAvatars(nextMemberAvatars[roomId] ?? {}, allEvents)

      // m.reaction をリアクション集計へ、それ以外のメッセージイベントをタイムラインへ
      const reactionEvents = timelineEvents.filter((e) => e.type === 'm.reaction')
      const msgEvents = timelineEvents.filter(
        (e) => e.state_key === undefined && e.type !== 'm.reaction'
      )

      nextTimelines[roomId] = [...(nextTimelines[roomId] ?? []), ...msgEvents]
      nextReactions[roomId] = mergeReactions(nextReactions[roomId] ?? {}, reactionEvents)

      // ephemeral: m.typing
      const typingEv = (room.ephemeral?.events ?? []).find((e) => e.type === 'm.typing')
      if (typingEv) {
        nextTyping[roomId] = (typingEv.content as { user_ids?: string[] }).user_ids ?? []
      }

      // limited=true のときだけ prev_batch を保存
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
      memberNames: nextMemberNames,
      memberAvatars: nextMemberAvatars,
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
      const reversed = [...resp.chunk].reverse()
      const newMsgEvents = reversed.filter(
        (e) => e.state_key === undefined && e.type !== 'm.reaction'
      )
      const newReactionEvents = reversed.filter((e) => e.type === 'm.reaction')
      // 過去ログからも m.room.member を拾う
      const newMemberEvents = reversed.filter((e) => e.type === 'm.room.member')
      set((s) => ({
        timelines: { ...s.timelines, [roomId]: [...newMsgEvents, ...(timelines[roomId] ?? [])] },
        prevBatches: { ...s.prevBatches, [roomId]: resp.end },
        reactions: {
          ...s.reactions,
          [roomId]: mergeReactions(reactions[roomId] ?? {}, newReactionEvents),
        },
        memberNames: {
          ...s.memberNames,
          [roomId]: mergeMemberNames(s.memberNames[roomId] ?? {}, newMemberEvents),
        },
        memberAvatars: {
          ...s.memberAvatars,
          [roomId]: mergeMemberAvatars(s.memberAvatars[roomId] ?? {}, newMemberEvents),
        },
      }))
    } catch {
      // 失敗は silent
    } finally {
      set((s) => ({ historyLoading: { ...s.historyLoading, [roomId]: false } }))
    }
  },

  markRoomRead(roomId) {
    set((s) => {
      const room = s.rooms[roomId]
      if (!room) return {}
      return {
        rooms: {
          ...s.rooms,
          [roomId]: { ...room, notificationCount: 0, highlightCount: 0 },
        },
      }
    })
  },

  setSyncing: (v) => set({ syncing: v }),
  setError: (e) => set({ error: e }),
  reset: () => set({ ...INITIAL }),
}))
