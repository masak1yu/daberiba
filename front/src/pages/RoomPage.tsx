/**
 * ルームページ — タイムライン表示 + メッセージ送信
 */
import { type FormEvent, useCallback, useEffect, useRef, useState } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { useAuthStore } from '../stores/auth'
import { useRoomsStore } from '../stores/rooms'
import { STORAGE_KEY } from '../api/client'
import { sendReadReceipt } from '../api/messages'
import { useSwipeBack } from '../hooks/useSwipeBack'
import AppShell from '../components/layout/AppShell'
import Timeline from '../components/room/Timeline'
import MembersList from '../components/room/MembersList'

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

  const [input, setInput] = useState('')
  const [sending, setSending] = useState(false)
  const [showMembers, setShowMembers] = useState(false)
  const txnRef = useRef(0)

  const decodedRoomId = roomId ? decodeURIComponent(roomId) : ''
  const events = timelines[decodedRoomId] ?? []
  const room = rooms[decodedRoomId]
  const hasMore = Boolean(prevBatches[decodedRoomId])
  const isHistoryLoading = historyLoading[decodedRoomId] ?? false

  // ルーム入室時・新着イベント受信時に既読送信
  const lastEventIdRef = useRef<string | undefined>()
  useEffect(() => {
    const lastEvent = events.at(-1)
    if (!lastEvent?.event_id) return
    if (lastEvent.event_id === lastEventIdRef.current) return
    lastEventIdRef.current = lastEvent.event_id

    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (homeserver && token) {
      void sendReadReceipt(homeserver, token, decodedRoomId, lastEvent.event_id)
    }
  }, [decodedRoomId, events])

  const handleLoadMore = useCallback(() => {
    void loadHistory(decodedRoomId)
  }, [decodedRoomId, loadHistory])

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

  return (
    <>
    <AppShell
      title={room?.name ?? decodedRoomId}
      showBack
      onBack={() => navigate('/')}
      headerRight={
        <button
          onClick={() => setShowMembers(true)}
          className="ml-2 text-gray-400 hover:text-white text-lg"
          title="メンバー一覧"
        >
          👥
        </button>
      }
    >
      <div className="flex h-full flex-col">
        <div className="min-h-0 flex-1">
          <Timeline
            events={events}
            myUserId={userId}
            hasMore={hasMore}
            historyLoading={isHistoryLoading}
            onLoadMore={handleLoadMore}
          />
        </div>
        <form
          onSubmit={(e) => void handleSend(e)}
          className="shrink-0 border-t border-gray-800 p-3"
          style={{ paddingBottom: 'max(env(safe-area-inset-bottom), 0.75rem)' }}
        >
          <div className="flex gap-2">
            <input
              type="text"
              value={input}
              onChange={(e) => setInput(e.target.value)}
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

    {showMembers && (
      <MembersList roomId={decodedRoomId} onClose={() => setShowMembers(false)} />
    )}
    </>
  )
}
