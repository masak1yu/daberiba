import { useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { useAuthStore } from '../../stores/auth'
import { useRoomsStore, type RoomSummary } from '../../stores/rooms'
import Avatar from '../common/Avatar'
import CreateRoomModal from '../room/CreateRoomModal'
import PublicRoomsModal from '../room/PublicRoomsModal'

// ユーザー ID や表示名から決定論的な色を返す
function stringToColor(str: string): string {
  let hash = 0
  for (let i = 0; i < str.length; i++) {
    hash = str.charCodeAt(i) + ((hash << 5) - hash)
  }
  const palette = [
    '#5c6bc0',
    '#7c4dff',
    '#00897b',
    '#e53935',
    '#f4511e',
    '#039be5',
    '#8e24aa',
    '#43a047',
  ]
  return palette[Math.abs(hash) % palette.length]!
}

function RoomItem({
  room,
  isActive,
  onSelect,
}: {
  room: RoomSummary
  isActive: boolean
  onSelect: (id: string) => void
}) {
  const label = room.name ?? room.roomId
  const lastBody = room.lastEvent
    ? String((room.lastEvent.content as { body?: string }).body ?? '')
    : ''
  const time = room.lastEvent
    ? new Date(room.lastEvent.origin_server_ts ?? 0).toLocaleTimeString('ja-JP', {
        hour: '2-digit',
        minute: '2-digit',
      })
    : ''

  return (
    <button
      onClick={() => onSelect(room.roomId)}
      className="group flex w-full items-center gap-2.5 rounded-lg px-2 py-1.5 text-left transition-colors mx-1"
      style={{
        width: 'calc(100% - 8px)',
        background: isActive ? '#3d4555' : 'transparent',
      }}
      onMouseEnter={(e) => {
        if (!isActive) e.currentTarget.style.background = '#2d3440'
      }}
      onMouseLeave={(e) => {
        if (!isActive) e.currentTarget.style.background = 'transparent'
      }}
    >
      {/* ルームアバター */}
      <div
        className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-xs font-bold uppercase select-none"
        style={{ background: stringToColor(label) }}
      >
        {label.charAt(0)}
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex items-baseline justify-between gap-1">
          <span
            className="truncate text-sm font-medium"
            style={{ color: isActive ? '#e9edf1' : '#c5cdd9' }}
          >
            {label}
          </span>
          {time && (
            <span className="shrink-0 text-xs" style={{ color: '#636e7d' }}>
              {time}
            </span>
          )}
        </div>
        {lastBody && (
          <p className="truncate text-xs" style={{ color: '#636e7d' }}>
            {lastBody}
          </p>
        )}
      </div>

      {room.highlightCount > 0 ? (
        <span
          className="shrink-0 rounded-full px-1.5 py-0.5 text-xs font-bold"
          style={{ background: '#e53935', color: 'white' }}
        >
          {room.highlightCount}
        </span>
      ) : room.notificationCount > 0 ? (
        <span
          className="shrink-0 rounded-full px-1.5 py-0.5 text-xs font-bold"
          style={{ background: '#0dbd8b', color: 'white' }}
        >
          {room.notificationCount}
        </span>
      ) : null}
    </button>
  )
}

export default function Sidebar() {
  const navigate = useNavigate()
  const { roomId: activeRoomIdEncoded } = useParams<{ roomId?: string }>()
  const activeRoomId = activeRoomIdEncoded ? decodeURIComponent(activeRoomIdEncoded) : undefined

  const userId = useAuthStore((s) => s.userId)
  const logout = useAuthStore((s) => s.logout)
  const rooms = useRoomsStore((s) => s.rooms)
  const syncing = useRoomsStore((s) => s.syncing)
  const error = useRoomsStore((s) => s.error)
  const markRoomRead = useRoomsStore((s) => s.markRoomRead)

  const [showCreate, setShowCreate] = useState(false)
  const [showPublic, setShowPublic] = useState(false)

  const sorted = Object.values(rooms).sort(
    (a, b) => (b.lastEvent?.origin_server_ts ?? 0) - (a.lastEvent?.origin_server_ts ?? 0)
  )

  function handleSelect(roomId: string) {
    markRoomRead(roomId)
    navigate(`/room/${encodeURIComponent(roomId)}`)
  }

  return (
    <>
      <div
        className="flex w-[260px] shrink-0 flex-col"
        style={{ background: '#21262d', borderRight: '1px solid #2d3440' }}
      >
        {/* ヘッダー */}
        <div
          className="flex items-center justify-between px-4 py-3"
          style={{ borderBottom: '1px solid #2d3440' }}
        >
          <span className="text-sm font-semibold" style={{ color: '#0dbd8b' }}>
            daberiba
          </span>
          <div className="flex items-center gap-0.5">
            <button
              onClick={() => setShowPublic(true)}
              className="rounded p-1.5 transition-colors hover:bg-white/10"
              style={{ color: '#8d99a6' }}
              title="パブリックルームを探す"
            >
              <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                />
              </svg>
            </button>
            <button
              onClick={() => setShowCreate(true)}
              className="rounded p-1.5 transition-colors hover:bg-white/10"
              style={{ color: '#8d99a6' }}
              title="新しいルームを作成"
            >
              <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M12 4v16m8-8H4"
                />
              </svg>
            </button>
          </div>
        </div>

        {/* エラーバー */}
        {error && (
          <div className="px-3 py-1.5 text-xs" style={{ background: '#7f1d1d', color: '#fca5a5' }}>
            {error}
          </div>
        )}

        {/* ルームラベル */}
        <div className="px-3 pb-1 pt-3">
          <span
            className="text-xs font-semibold uppercase tracking-wider"
            style={{ color: '#636e7d' }}
          >
            ルーム
          </span>
        </div>

        {/* ルーム一覧 */}
        <div className="min-h-0 flex-1 overflow-y-auto py-1">
          {syncing && sorted.length === 0 ? (
            <div className="flex items-center gap-2 px-3 py-2 text-sm" style={{ color: '#8d99a6' }}>
              <div
                className="h-3 w-3 animate-spin rounded-full border"
                style={{ borderColor: '#636e7d', borderTopColor: 'transparent' }}
              />
              同期中…
            </div>
          ) : sorted.length === 0 ? (
            <div className="px-3 py-2 text-xs" style={{ color: '#636e7d' }}>
              参加中のルームがありません
            </div>
          ) : (
            sorted.map((room) => (
              <RoomItem
                key={room.roomId}
                room={room}
                isActive={room.roomId === activeRoomId}
                onSelect={handleSelect}
              />
            ))
          )}
        </div>

        {/* ユーザーパネル */}
        <div
          className="flex items-center gap-2 px-3 py-2"
          style={{ borderTop: '1px solid #2d3440' }}
        >
          <Avatar userId={userId ?? ''} displayName={userId ?? ''} size="sm" />
          <span className="min-w-0 flex-1 truncate text-xs" style={{ color: '#8d99a6' }}>
            {userId}
          </span>
          <button
            onClick={() => navigate('/settings')}
            className="rounded p-1 transition-colors hover:bg-white/10"
            style={{ color: '#636e7d' }}
            title="設定"
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
            onClick={() => void logout()}
            className="rounded p-1 transition-colors hover:bg-white/10"
            style={{ color: '#636e7d' }}
            title="ログアウト"
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

      {showCreate && (
        <CreateRoomModal
          onCreated={(roomId) => {
            setShowCreate(false)
            navigate(`/room/${encodeURIComponent(roomId)}`)
          }}
          onClose={() => setShowCreate(false)}
        />
      )}

      {showPublic && (
        <PublicRoomsModal
          onJoined={(roomId) => {
            setShowPublic(false)
            markRoomRead(roomId)
            navigate(`/room/${encodeURIComponent(roomId)}`)
          }}
          onClose={() => setShowPublic(false)}
        />
      )}
    </>
  )
}
