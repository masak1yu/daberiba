import { type FormEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useParams } from 'react-router-dom'
import { useAuthStore } from '../stores/auth'
import { useRoomsStore } from '../stores/rooms'
import { STORAGE_KEY } from '../api/client'
import { sendReadReceipt } from '../api/messages'
import { leaveRoom } from '../api/rooms'
import { uploadMedia } from '../api/profile'
import { sendTyping } from '../api/sync'
import { sendReaction, redactEvent } from '../api/roomState'
import Timeline from '../components/room/Timeline'
import MembersList from '../components/room/MembersList'
import RoomSettingsModal from '../components/room/RoomSettingsModal'

export default function RoomPage() {
  const { roomId } = useParams<{ roomId: string }>()
  const decodedRoomId = roomId ? decodeURIComponent(roomId) : ''

  const userId = useAuthStore((s) => s.userId)
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

  const events = useMemo(() => timelines[decodedRoomId] ?? [], [timelines, decodedRoomId])
  const room = rooms[decodedRoomId]
  const hasMore = Boolean(prevBatches[decodedRoomId])
  const isHistoryLoading = historyLoading[decodedRoomId] ?? false
  const reactions = allReactions[decodedRoomId]
  const memberNames = allMemberNames[decodedRoomId]
  const memberAvatars = allMemberAvatars[decodedRoomId]
  const typingUsers = (allTyping[decodedRoomId] ?? [])
    .filter((id) => id !== userId)
    .map((id) => memberNames?.[id] ?? id)

  useEffect(() => {
    markRoomRead(decodedRoomId)
  }, [decodedRoomId, markRoomRead])

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
    } catch {
      setLeaving(false)
      setConfirmLeave(false)
    }
  }

  async function handleSend(e: FormEvent) {
    e.preventDefault()
    const text = input.trim()
    if (!text || sending) return
    setSending(true)
    const txnId = `m${Date.now()}.${++txnRef.current}`
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const accessToken = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !accessToken) {
      setSending(false)
      return
    }
    try {
      if (userId) void sendTyping(homeserver, accessToken, decodedRoomId, userId, false)
      if (typingTimerRef.current) {
        clearTimeout(typingTimerRef.current)
        typingTimerRef.current = null
      }
      const url = `${homeserver}/_matrix/client/v3/rooms/${encodeURIComponent(decodedRoomId)}/send/m.room.message/${txnId}`
      const res = await fetch(url, {
        method: 'PUT',
        headers: { Authorization: `Bearer ${accessToken}`, 'Content-Type': 'application/json' },
        body: JSON.stringify({ msgtype: 'm.text', body: text }),
      })
      if (res.ok) setInput('')
    } catch {
      // 送信失敗は silent
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
      // 失敗は silent
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
      <div className="flex h-full flex-col" style={{ background: '#15191e' }}>
        {/* ルームヘッダー */}
        <div
          className="flex shrink-0 items-center gap-3 px-4 py-3"
          style={{ borderBottom: '1px solid #2d3440', background: '#15191e' }}
        >
          <div className="min-w-0 flex-1">
            <h1 className="truncate text-base font-semibold" style={{ color: '#e9edf1' }}>
              {room?.name ?? decodedRoomId}
            </h1>
            {room?.topic && (
              <p className="truncate text-xs" style={{ color: '#8d99a6' }}>
                {room.topic}
              </p>
            )}
          </div>
          <div className="flex items-center gap-0.5">
            <button
              onClick={() => setShowRoomSettings(true)}
              className="rounded p-1.5 transition-colors hover:bg-white/10"
              style={{ color: '#8d99a6' }}
              title="ルーム設定"
            >
              <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
                />
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
                />
              </svg>
            </button>
            <button
              onClick={() => setShowMembers(true)}
              className="rounded p-1.5 transition-colors hover:bg-white/10"
              style={{ color: '#8d99a6' }}
              title="メンバー一覧"
            >
              <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z"
                />
              </svg>
            </button>
            <button
              onClick={() => setConfirmLeave(true)}
              className="rounded p-1.5 transition-colors hover:bg-white/10"
              style={{ color: '#636e7d' }}
              title="ルームを退出"
            >
              <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"
                />
              </svg>
            </button>
          </div>
        </div>

        {/* タイムライン */}
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
          <div className="shrink-0 px-4 py-1 text-xs" style={{ color: '#8d99a6' }}>
            {typingUsers.length === 1
              ? `${typingUsers[0]} が入力中…`
              : `${typingUsers.length} 人が入力中…`}
          </div>
        )}

        {/* コンポーザー */}
        <div className="shrink-0 px-4 pb-4 pt-2">
          <form onSubmit={(e) => void handleSend(e)}>
            <div
              className="flex items-end gap-2 rounded-xl px-3 py-2"
              style={{ background: '#21262d', border: '1px solid #2d3440' }}
            >
              {/* ファイル添付 */}
              <button
                type="button"
                onClick={() => fileInputRef.current?.click()}
                disabled={uploading}
                className="mb-0.5 shrink-0 rounded p-1 transition-colors hover:bg-white/10 disabled:opacity-40"
                style={{ color: '#8d99a6' }}
                title="ファイルを添付"
              >
                {uploading ? (
                  <div
                    className="h-4 w-4 animate-spin rounded-full border"
                    style={{ borderColor: '#636e7d', borderTopColor: 'transparent' }}
                  />
                ) : (
                  <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"
                    />
                  </svg>
                )}
              </button>
              <input
                ref={fileInputRef}
                type="file"
                className="hidden"
                onChange={(e) => void handleFileChange(e)}
              />

              {/* テキスト入力 */}
              <textarea
                value={input}
                onChange={(e) => {
                  handleInputChange(e.target.value)
                  e.target.style.height = 'auto'
                  e.target.style.height = Math.min(e.target.scrollHeight, 120) + 'px'
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault()
                    void handleSend(e as unknown as FormEvent)
                  }
                }}
                placeholder="メッセージを入力…"
                rows={1}
                className="min-w-0 flex-1 resize-none bg-transparent py-0.5 text-sm focus:outline-none"
                style={{ color: '#e9edf1', lineHeight: '1.5' }}
              />

              {/* 送信ボタン */}
              <button
                type="submit"
                disabled={sending || !input.trim()}
                className="mb-0.5 shrink-0 rounded-lg px-3 py-1.5 text-sm font-medium transition-colors disabled:opacity-40"
                style={{
                  background: input.trim() ? '#0dbd8b' : '#2d3440',
                  color: input.trim() ? 'white' : '#636e7d',
                }}
              >
                送信
              </button>
            </div>
          </form>
        </div>
      </div>

      {showMembers && <MembersList roomId={decodedRoomId} onClose={() => setShowMembers(false)} />}

      {showRoomSettings && (
        <RoomSettingsModal
          roomId={decodedRoomId}
          currentName={room?.name}
          currentTopic={room?.topic}
          onClose={() => setShowRoomSettings(false)}
        />
      )}

      {confirmLeave && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center px-4"
          style={{ background: 'rgba(0,0,0,0.7)' }}
          onClick={(e) => {
            if (e.target === e.currentTarget) setConfirmLeave(false)
          }}
        >
          <div
            className="w-full max-w-xs rounded-2xl p-6 shadow-2xl"
            style={{ background: '#21262d', border: '1px solid #2d3440' }}
          >
            <h2 className="mb-2 text-base font-bold" style={{ color: '#e9edf1' }}>
              ルームを退出しますか？
            </h2>
            <p className="mb-5 text-sm" style={{ color: '#8d99a6' }}>
              {room?.name ?? decodedRoomId} から退出します。
            </p>
            <div className="flex gap-2">
              <button
                onClick={() => setConfirmLeave(false)}
                disabled={leaving}
                className="flex-1 rounded-lg py-2 text-sm transition-colors disabled:opacity-50"
                style={{ border: '1px solid #2d3440', color: '#8d99a6' }}
              >
                キャンセル
              </button>
              <button
                onClick={() => void handleLeave()}
                disabled={leaving}
                className="flex-1 rounded-lg py-2 text-sm font-medium text-white transition-colors disabled:opacity-50"
                style={{ background: '#e53935' }}
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
