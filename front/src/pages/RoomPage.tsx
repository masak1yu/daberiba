/**
 * ルームページ — タイムライン表示 + メッセージ送信
 */
import { type FormEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { useAuthStore } from '../stores/auth'
import { useRoomsStore } from '../stores/rooms'
import { STORAGE_KEY } from '../api/client'
import { sendReadReceipt } from '../api/messages'
import { leaveRoom } from '../api/rooms'
import { uploadMedia } from '../api/profile'
import { sendTyping } from '../api/sync'
import { sendReaction, redactEvent } from '../api/roomState'
import { useSwipeBack } from '../hooks/useSwipeBack'
import AppShell from '../components/layout/AppShell'
import Timeline from '../components/room/Timeline'
import MembersList from '../components/room/MembersList'
import RoomSettingsModal from '../components/room/RoomSettingsModal'

export default function RoomPage() {
  const { roomId } = useParams<{ roomId: string }>()
  const navigate = useNavigate()

  // 左端スワイプで前の画面（ルーム一覧）に戻る
  const goBack = useCallback(() => navigate('/'), [navigate])
  useSwipeBack(goBack)

  const userId = useAuthStore((s) => s.userId)
  const client = useAuthStore((s) => s.client)
  const timelines = useRoomsStore((s) => s.timelines)
  const rooms = useRoomsStore((s) => s.rooms)
  const prevBatches = useRoomsStore((s) => s.prevBatches)
  const historyLoading = useRoomsStore((s) => s.historyLoading)
  const loadHistory = useRoomsStore((s) => s.loadHistory)
  const allReactions = useRoomsStore((s) => s.reactions)
  const allTyping = useRoomsStore((s) => s.typing)
  const allMemberNames = useRoomsStore((s) => s.memberNames)
  const allMemberAvatars = useRoomsStore((s) => s.memberAvatars)
  const markRoomRead = useRoomsStore((s) => s.markRoomRead)
  const storeRedactEvent = useRoomsStore((s) => s.redactEvent)
  const storeApplyEdit = useRoomsStore((s) => s.applyEdit)

  const [input, setInput] = useState('')
  const [sending, setSending] = useState(false)
  const [uploading, setUploading] = useState(false)
  const [showMembers, setShowMembers] = useState(false)
  const [showRoomSettings, setShowRoomSettings] = useState(false)
  const [confirmLeave, setConfirmLeave] = useState(false)
  const [leaving, setLeaving] = useState(false)
  const txnRef = useRef(0)
  const typingTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const decodedRoomId = roomId ? decodeURIComponent(roomId) : ''
  const events = useMemo(() => timelines[decodedRoomId] ?? [], [timelines, decodedRoomId])
  const room = rooms[decodedRoomId]
  const hasMore = Boolean(prevBatches[decodedRoomId])
  const isHistoryLoading = historyLoading[decodedRoomId] ?? false
  const reactions = allReactions[decodedRoomId]
  const memberNames = allMemberNames[decodedRoomId]
  const memberAvatars = allMemberAvatars[decodedRoomId]
  // 自分以外のタイピング中ユーザー（displayName 優先で表示）
  const typingUsers = (allTyping[decodedRoomId] ?? [])
    .filter((id) => id !== userId)
    .map((id) => memberNames?.[id] ?? id)

  // ルーム入室時に未読カウントをリセット
  useEffect(() => {
    markRoomRead(decodedRoomId)
  }, [decodedRoomId, markRoomRead])

  // ルーム入室時・新着イベント受信時に既読送信
  const lastEventIdRef = useRef<string | undefined>(undefined)
  useEffect(() => {
    const lastEvent = events.at(-1)
    if (!lastEvent?.event_id) return
    if (lastEvent.event_id === lastEventIdRef.current) return
    lastEventIdRef.current = lastEvent.event_id

    markRoomRead(decodedRoomId)
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (homeserver && token) {
      void sendReadReceipt(homeserver, token, decodedRoomId, lastEvent.event_id)
    }
  }, [decodedRoomId, events, markRoomRead])

  const handleLoadMore = useCallback(() => {
    void loadHistory(decodedRoomId)
  }, [decodedRoomId, loadHistory])

  // 入力中に typing=true を送信し、8s 無入力で typing=false を送る
  function handleInputChange(value: string) {
    setInput(value)

    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token || !userId) return

    if (typingTimerRef.current) clearTimeout(typingTimerRef.current)

    if (value) {
      void sendTyping(homeserver, token, decodedRoomId, userId, true)
      typingTimerRef.current = setTimeout(() => {
        void sendTyping(homeserver, token, decodedRoomId, userId, false)
      }, 8_000)
    } else {
      void sendTyping(homeserver, token, decodedRoomId, userId, false)
    }
  }

  // ルーム離脱時に typing=false を送信してタイマーをクリア
  useEffect(() => {
    return () => {
      if (typingTimerRef.current) clearTimeout(typingTimerRef.current)
      const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
      const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
      if (homeserver && token && userId) {
        void sendTyping(homeserver, token, decodedRoomId, userId, false)
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [decodedRoomId])

  async function handleLeave() {
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return

    setLeaving(true)
    try {
      await leaveRoom(homeserver, token, decodedRoomId)
      navigate('/')
    } catch {
      setLeaving(false)
      setConfirmLeave(false)
    }
  }

  async function handleSend(e: FormEvent) {
    e.preventDefault()
    const text = input.trim()
    if (!text || sending || !client) return

    setSending(true)
    const txnId = `m${Date.now()}.${++txnRef.current}`
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const accessToken = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !accessToken) {
      setSending(false)
      return
    }

    try {
      // 送信直前に typing=false を即時送信
      if (userId) void sendTyping(homeserver, accessToken, decodedRoomId, userId, false)
      if (typingTimerRef.current) {
        clearTimeout(typingTimerRef.current)
        typingTimerRef.current = null
      }

      // PUT /_matrix/client/v3/rooms/{roomId}/send/m.room.message/{txnId}
      const url = `${homeserver}/_matrix/client/v3/rooms/${encodeURIComponent(decodedRoomId)}/send/m.room.message/${txnId}`
      const res = await fetch(url, {
        method: 'PUT',
        headers: { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ msgtype: 'm.text', body: text }),
      })
      if (res.ok) setInput('')
    } catch {
      // 送信失敗は silent — sync で再取得される
    } finally {
      setSending(false)
    }
  }

  async function handleFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0]
    if (!file) return

    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const accessToken = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !accessToken) return

    setUploading(true)
    try {
      const mxc = await uploadMedia(homeserver, accessToken, file)
      const isImage = file.type.startsWith('image/')
      const isVideo = file.type.startsWith('video/')
      const isAudio = file.type.startsWith('audio/')
      const msgtype = isImage ? 'm.image' : isVideo ? 'm.video' : isAudio ? 'm.audio' : 'm.file'

      const txnId = `m${Date.now()}.${++txnRef.current}`
      const url = `${homeserver}/_matrix/client/v3/rooms/${encodeURIComponent(decodedRoomId)}/send/m.room.message/${txnId}`
      await fetch(url, {
        method: 'PUT',
        headers: { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ msgtype, body: file.name, url: mxc }),
      })
    } catch {
      // 失敗は silent
    } finally {
      setUploading(false)
      if (fileInputRef.current) fileInputRef.current.value = ''
    }
  }

  async function handleReact(eventId: string, emoji: string) {
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return
    try {
      await sendReaction(homeserver, token, decodedRoomId, eventId, emoji)
    } catch {
      // 失敗は silent
    }
  }

  async function handleDelete(eventId: string) {
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return
    storeRedactEvent(decodedRoomId, eventId)
    try {
      await redactEvent(homeserver, token, decodedRoomId, eventId)
    } catch {
      // 失敗は silent — sync で再同期される
    }
  }

  async function handleEdit(eventId: string, newBody: string) {
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return
    const newContent = { msgtype: 'm.text', body: newBody }
    storeApplyEdit(decodedRoomId, eventId, newContent)
    try {
      const txnId = `edit.${Date.now()}.${++txnRef.current}`
      const url = `${homeserver}/_matrix/client/v3/rooms/${encodeURIComponent(decodedRoomId)}/send/m.room.message/${txnId}`
      await fetch(url, {
        method: 'PUT',
        headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({
          ...newContent,
          'm.relates_to': { rel_type: 'm.replace', event_id: eventId },
          'm.new_content': newContent,
        }),
      })
    } catch {
      // 失敗は silent
    }
  }

  return (
    <>
      <AppShell
        title={room?.name ?? decodedRoomId}
        showBack
        onBack={() => navigate('/')}
        headerRight={
          <div className="ml-2 flex items-center gap-1">
            <button
              onClick={() => setShowRoomSettings(true)}
              className="text-gray-400 hover:text-white text-sm px-1"
              title="ルーム設定"
            >
              ⚙
            </button>
            <button
              onClick={() => setShowMembers(true)}
              className="text-gray-400 hover:text-white text-lg"
              title="メンバー一覧"
            >
              👥
            </button>
            <button
              onClick={() => setConfirmLeave(true)}
              className="text-gray-500 hover:text-red-400 text-sm px-1"
              title="ルームを退出"
            >
              退出
            </button>
          </div>
        }
      >
        <div className="flex h-full flex-col">
          <div className="min-h-0 flex-1">
            <Timeline
              events={events}
              myUserId={userId}
              reactions={reactions}
              memberNames={memberNames}
              memberAvatars={memberAvatars}
              hasMore={hasMore}
              historyLoading={isHistoryLoading}
              onLoadMore={handleLoadMore}
              onReact={handleReact}
              onDelete={(id) => void handleDelete(id)}
              onEdit={(id, body) => void handleEdit(id, body)}
            />
          </div>

          {/* タイピングインジケーター */}
          {typingUsers.length > 0 && (
            <div className="shrink-0 px-4 py-1 text-xs text-gray-500">
              {typingUsers.length === 1
                ? `${typingUsers[0]} が入力中…`
                : `${typingUsers.length} 人が入力中…`}
            </div>
          )}

          <form
            onSubmit={(e) => void handleSend(e)}
            className="shrink-0 border-t border-gray-800 p-3"
            style={{ paddingBottom: 'max(env(safe-area-inset-bottom), 0.75rem)' }}
          >
            <div className="flex gap-2">
              {/* ファイル添付ボタン */}
              <button
                type="button"
                onClick={() => fileInputRef.current?.click()}
                disabled={uploading}
                className="shrink-0 rounded-lg bg-gray-800 px-3 py-2 text-gray-400 hover:bg-gray-700 hover:text-white disabled:opacity-50"
                title="ファイルを添付"
              >
                {uploading ? '…' : '📎'}
              </button>
              <input
                ref={fileInputRef}
                type="file"
                className="hidden"
                onChange={(e) => void handleFileChange(e)}
              />

              <input
                type="text"
                value={input}
                onChange={(e) => handleInputChange(e.target.value)}
                placeholder="メッセージを入力…"
                className="min-w-0 flex-1 rounded-lg bg-gray-800 px-4 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
              />
              <button
                type="submit"
                disabled={sending || !input.trim()}
                className="rounded-lg bg-indigo-600 px-4 py-2 text-white transition hover:bg-indigo-500 disabled:opacity-50"
              >
                送信
              </button>
            </div>
          </form>
        </div>
      </AppShell>

      {showMembers && <MembersList roomId={decodedRoomId} onClose={() => setShowMembers(false)} />}

      {showRoomSettings && (
        <RoomSettingsModal
          roomId={decodedRoomId}
          currentName={room?.name}
          currentTopic={room?.topic}
          onClose={() => setShowRoomSettings(false)}
        />
      )}

      {/* 退出確認ダイアログ */}
      {confirmLeave && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 px-4"
          onClick={(e) => {
            if (e.target === e.currentTarget) setConfirmLeave(false)
          }}
        >
          <div className="w-full max-w-xs rounded-2xl bg-gray-900 p-6 shadow-xl">
            <h2 className="mb-2 text-base font-bold">ルームを退出しますか？</h2>
            <p className="mb-5 text-sm text-gray-400">
              {room?.name ?? decodedRoomId} から退出します。
            </p>
            <div className="flex gap-2">
              <button
                onClick={() => setConfirmLeave(false)}
                disabled={leaving}
                className="flex-1 rounded-lg border border-gray-700 py-2 text-sm text-gray-400 hover:bg-gray-800 disabled:opacity-50"
              >
                キャンセル
              </button>
              <button
                onClick={() => void handleLeave()}
                disabled={leaving}
                className="flex-1 rounded-lg bg-red-700 py-2 text-sm text-white hover:bg-red-600 disabled:opacity-50"
              >
                {leaving ? '退出中…' : '退出する'}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  )
}
