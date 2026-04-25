import { type FormEvent, useCallback, useEffect, useRef, useState } from 'react'
import { useParams } from 'react-router-dom'
import { useShallow } from 'zustand/react/shallow'
import { useAuthStore } from '../stores/auth'
import { useRoomsStore } from '../stores/rooms'
import { STORAGE_KEY } from '../api/client'
import { sendReadReceipt } from '../api/messages'
import { leaveRoom } from '../api/rooms'
import { uploadMedia } from '../api/profile'
import { sendTyping, type MatrixEvent } from '../api/sync'
import { sendReaction, redactEvent } from '../api/roomState'
import Timeline from '../components/room/Timeline'
import MembersList from '../components/room/MembersList'
import RoomSettingsModal from '../components/room/RoomSettingsModal'
import { userColor } from '../utils/userColor'
import { useUiStore } from '../stores/ui'

const EMPTY_EVENTS: MatrixEvent[] = []

export default function RoomPage() {
  const { roomId } = useParams<{ roomId: string }>()
  const decodedRoomId = roomId ? decodeURIComponent(roomId) : ''

  const userId = useAuthStore((s) => s.userId)
  const events = useRoomsStore((s) => s.timelines[decodedRoomId] ?? EMPTY_EVENTS)
  const room = useRoomsStore((s) => s.rooms[decodedRoomId])
  const hasMore = useRoomsStore((s) => Boolean(s.prevBatches[decodedRoomId]))
  const isHistoryLoading = useRoomsStore((s) => s.historyLoading[decodedRoomId] ?? false)
  const loadHistory = useRoomsStore((s) => s.loadHistory)
  const reactions = useRoomsStore((s) => s.reactions[decodedRoomId])
  const typingUsers = useRoomsStore(
    useShallow((s) =>
      (s.typing[decodedRoomId] ?? [])
        .filter((id) => id !== userId)
        .map((id) => s.memberNames[decodedRoomId]?.[id] ?? id)
    )
  )
  const memberNames = useRoomsStore((s) => s.memberNames[decodedRoomId])
  const memberAvatars = useRoomsStore((s) => s.memberAvatars[decodedRoomId])
  const markRoomRead = useRoomsStore((s) => s.markRoomRead)
  const storeRedactEvent = useRoomsStore((s) => s.redactEvent)
  const storeApplyEdit = useRoomsStore((s) => s.applyEdit)

  const showToast = useUiStore((s) => s.showToast)

  const [input, setInput] = useState('')
  const [sending, setSending] = useState(false)
  const [uploading, setUploading] = useState(false)
  const [showMembers, setShowMembers] = useState(false)
  const [showRoomSettings, setShowRoomSettings] = useState(false)
  const [confirmLeave, setConfirmLeave] = useState(false)
  const [leaving, setLeaving] = useState(false)
  const [showEmojiPicker, setShowEmojiPicker] = useState(false)
  const txnRef = useRef(0)
  const typingTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

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

  function handleEmojiInsert(emoji: string) {
    const ta = textareaRef.current
    if (!ta) {
      handleInputChange(input + emoji)
      return
    }
    const start = ta.selectionStart ?? input.length
    const end = ta.selectionEnd ?? input.length
    const next = input.slice(0, start) + emoji + input.slice(end)
    handleInputChange(next)
    setShowEmojiPicker(false)
    // フォーカスとカーソル位置を復元
    requestAnimationFrame(() => {
      ta.focus()
      ta.setSelectionRange(start + emoji.length, start + emoji.length)
    })
  }

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
    } catch (err) {
      showToast(`退出に失敗しました: ${err instanceof Error ? err.message : String(err)}`, 'error')
      setConfirmLeave(false)
    } finally {
      setLeaving(false)
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
      showToast('認証情報がありません。再ログインしてください。', 'error')
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
      if (res.ok) {
        setInput('')
      } else {
        const body = await res.json().catch(() => ({}))
        const msg = (body as { error?: string }).error ?? `HTTP ${res.status}`
        showToast(`送信失敗: ${msg}`, 'error')
      }
    } catch (err) {
      showToast(`送信エラー: ${err instanceof Error ? err.message : String(err)}`, 'error')
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
    } catch (err) {
      showToast(
        `ファイル送信に失敗しました: ${err instanceof Error ? err.message : String(err)}`,
        'error'
      )
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
    } catch (err) {
      showToast(
        `リアクション送信に失敗しました: ${err instanceof Error ? err.message : String(err)}`,
        'error'
      )
    }
  }

  async function handleDelete(eventId: string) {
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return
    storeRedactEvent(decodedRoomId, eventId)
    try {
      await redactEvent(homeserver, token, decodedRoomId, eventId)
    } catch (err) {
      showToast(`削除に失敗しました: ${err instanceof Error ? err.message : String(err)}`, 'error')
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
    } catch (err) {
      showToast(`編集に失敗しました: ${err instanceof Error ? err.message : String(err)}`, 'error')
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
          {/* ルームアバター */}
          <div
            className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full text-base font-bold select-none"
            style={{ background: userColor(decodedRoomId) }}
          >
            {(room?.name ?? decodedRoomId).charAt(0).toUpperCase()}
          </div>

          <div className="min-w-0 flex-1">
            <h1
              className="truncate text-base font-semibold leading-tight"
              style={{ color: '#e9edf1' }}
            >
              {room?.name ?? decodedRoomId}
            </h1>
            {room?.topic && (
              <p className="truncate text-xs" style={{ color: '#8d99a6' }}>
                {room.topic}
              </p>
            )}
          </div>

          {/* アクションアイコン群 */}
          <div className="flex items-center gap-0.5">
            <button
              onClick={() => setShowMembers(true)}
              className="flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-sm transition-colors hover:bg-white/10"
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
              onClick={() => setShowRoomSettings(true)}
              className="rounded-lg p-1.5 transition-colors hover:bg-white/10"
              style={{ color: '#8d99a6' }}
              title="ルーム情報"
            >
              <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                />
              </svg>
            </button>
            <button
              onClick={() => setConfirmLeave(true)}
              className="rounded-lg p-1.5 transition-colors hover:bg-white/10"
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

        {/* コンポーザー（Element 風） */}
        <div className="shrink-0 px-4 pb-4 pt-1">
          <div
            className="rounded-xl"
            style={{ background: '#21262d', border: '1px solid #2d3440' }}
          >
            <form onSubmit={(e) => void handleSend(e)}>
              <div className="flex items-end px-4 py-3">
                {/* 緑ドット */}
                <div
                  className="mb-1 mr-3 h-2 w-2 shrink-0 rounded-full"
                  style={{ background: '#0dbd8b' }}
                />

                {/* テキスト入力 */}
                <textarea
                  ref={textareaRef}
                  value={input}
                  onChange={(e) => {
                    handleInputChange(e.target.value)
                    e.target.style.height = 'auto'
                    e.target.style.height = Math.min(e.target.scrollHeight, 120) + 'px'
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && e.shiftKey) {
                      e.preventDefault()
                      void handleSend(e as unknown as FormEvent)
                    }
                  }}
                  placeholder="メッセージを入力…"
                  rows={1}
                  className="min-w-0 flex-1 resize-none bg-transparent py-0 text-sm focus:outline-none"
                  style={{ color: '#e9edf1', lineHeight: '1.5' }}
                />

                {/* 右側アクション */}
                <div className="relative ml-2 flex shrink-0 items-center gap-0.5">
                  {/* 絵文字ピッカー */}
                  {showEmojiPicker && (
                    <div
                      className="absolute bottom-full right-0 mb-2 grid w-max grid-cols-8 gap-0.5 rounded-xl p-2 shadow-2xl"
                      style={{
                        background: '#21262d',
                        border: '1px solid #2d3440',
                        zIndex: 50,
                      }}
                    >
                      {[
                        '😀',
                        '😂',
                        '🥹',
                        '😊',
                        '😎',
                        '🥰',
                        '😍',
                        '🤩',
                        '😅',
                        '😭',
                        '😤',
                        '🤔',
                        '🫠',
                        '😶',
                        '🥲',
                        '😬',
                        '👍',
                        '👎',
                        '👏',
                        '🙌',
                        '🤝',
                        '🙏',
                        '💪',
                        '✌️',
                        '❤️',
                        '🔥',
                        '✨',
                        '🎉',
                        '💯',
                        '⚡',
                        '💀',
                        '👀',
                      ].map((emoji) => (
                        <button
                          key={emoji}
                          type="button"
                          onClick={() => handleEmojiInsert(emoji)}
                          className="rounded p-1 text-lg hover:bg-white/10"
                        >
                          {emoji}
                        </button>
                      ))}
                    </div>
                  )}

                  {/* 絵文字ボタン */}
                  <button
                    type="button"
                    onClick={() => setShowEmojiPicker((v) => !v)}
                    className="rounded p-1.5 transition-colors hover:bg-white/10"
                    style={{ color: showEmojiPicker ? '#0dbd8b' : '#8d99a6' }}
                    title="絵文字"
                  >
                    <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={1.8}
                        d="M14.828 14.828a4 4 0 01-5.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                      />
                    </svg>
                  </button>

                  {/* ファイル添付 */}
                  {!input.trim() && (
                    <button
                      type="button"
                      onClick={() => fileInputRef.current?.click()}
                      disabled={uploading}
                      className="rounded p-1.5 transition-colors hover:bg-white/10 disabled:opacity-40"
                      style={{ color: '#8d99a6' }}
                      title="ファイルを添付"
                    >
                      {uploading ? (
                        <div
                          className="h-5 w-5 animate-spin rounded-full border"
                          style={{ borderColor: '#636e7d', borderTopColor: 'transparent' }}
                        />
                      ) : (
                        <svg
                          className="h-5 w-5"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke="currentColor"
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            strokeWidth={1.8}
                            d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13"
                          />
                        </svg>
                      )}
                    </button>
                  )}

                  {/* 送信ボタン（入力があるときのみ — Element 準拠） */}
                  {input.trim() && (
                    <button
                      type="submit"
                      disabled={sending}
                      className="flex h-8 w-8 items-center justify-center rounded-full transition-opacity disabled:opacity-50"
                      style={{ background: '#0dbd8b', color: 'white' }}
                      title="送信"
                    >
                      <svg className="h-4 w-4" viewBox="0 0 24 24" fill="currentColor">
                        <path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z" />
                      </svg>
                    </button>
                  )}
                </div>
              </div>
            </form>
            <input
              ref={fileInputRef}
              type="file"
              className="hidden"
              onChange={(e) => void handleFileChange(e)}
            />
          </div>
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
