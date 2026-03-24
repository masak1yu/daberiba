/**
 * ルームページ — タイムライン表示 + メッセージ送信
 */
import { type FormEvent, useRef, useState } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { useAuthStore } from '../stores/auth'
import { useRoomsStore } from '../stores/rooms'
import { STORAGE_KEY } from '../api/client'
import AppShell from '../components/layout/AppShell'
import Timeline from '../components/room/Timeline'

export default function RoomPage() {
  const { roomId } = useParams<{ roomId: string }>()
  const navigate = useNavigate()
  const userId = useAuthStore((s) => s.userId)
  const client = useAuthStore((s) => s.client)
  const timelines = useRoomsStore((s) => s.timelines)
  const rooms = useRoomsStore((s) => s.rooms)

  const [input, setInput] = useState('')
  const [sending, setSending] = useState(false)
  const txnRef = useRef(0)

  const decodedRoomId = roomId ? decodeURIComponent(roomId) : ''
  const events = timelines[decodedRoomId] ?? []
  const room = rooms[decodedRoomId]

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
    <AppShell title={room?.name ?? decodedRoomId} showBack onBack={() => navigate('/')}>
      <div className="flex h-full flex-col">
        <div className="min-h-0 flex-1 overflow-y-auto">
          <Timeline events={events} myUserId={userId} />
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
  )
}
