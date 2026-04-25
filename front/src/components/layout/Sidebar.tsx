import { useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { useRoomsStore, type RoomSummary } from '../../stores/rooms'
import { userColor } from '../../utils/userColor'
import CreateRoomModal from '../room/CreateRoomModal'
import PublicRoomsModal from '../room/PublicRoomsModal'

type FilterTab = 'all' | 'unread'

function RoomItem({
  room,
  isActive,
  onSelect,
}: {
  room: RoomSummary
  isActive: boolean
  onSelect: (id: string) => void
}) {
  const [hovered, setHovered] = useState(false)
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
  const hasBadge = room.highlightCount > 0 || room.notificationCount > 0

  return (
    <div
      className="relative mx-1 my-0.5 rounded-lg"
      style={{
        background: isActive ? '#343a46' : hovered ? '#343a46' : 'transparent',
      }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <button
        onClick={() => onSelect(room.roomId)}
        className="flex w-full items-center gap-2.5 px-2 py-2 text-left"
        style={{ color: isActive ? '#e9edf1' : '#c5cdd9' }}
      >
        {/* ルームアバター */}
        <div
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-sm font-bold uppercase select-none"
          style={{ background: userColor(label) }}
        >
          {label.charAt(0)}
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex items-baseline justify-between gap-1">
            <span className="truncate text-sm" style={{ fontWeight: hasBadge ? 600 : 400 }}>
              {label}
            </span>
            {time && !hovered && (
              <span className="shrink-0 text-xs" style={{ color: '#636e7d' }}>
                {time}
              </span>
            )}
          </div>
          {lastBody && (
            <p className="truncate text-xs" style={{ color: '#8d99a6' }}>
              {lastBody}
            </p>
          )}
        </div>

        {/* バッジ（ホバー時は非表示） */}
        {!hovered && (
          <>
            {room.highlightCount > 0 ? (
              <span
                className="shrink-0 min-w-[18px] rounded-full px-1.5 py-0.5 text-center text-xs font-bold"
                style={{ background: '#e53935', color: 'white' }}
              >
                {room.highlightCount}
              </span>
            ) : room.notificationCount > 0 ? (
              <span
                className="shrink-0 min-w-[18px] rounded-full px-1.5 py-0.5 text-center text-xs font-bold"
                style={{ background: '#0dbd8b', color: 'white' }}
              >
                {room.notificationCount}
              </span>
            ) : null}
          </>
        )}
      </button>

      {/* ホバー時: ⋯ メニューボタン */}
      {hovered && (
        <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-0.5">
          <button
            onClick={(e) => e.stopPropagation()}
            className="flex h-6 w-6 items-center justify-center rounded transition-colors hover:bg-white/10"
            style={{ color: '#8d99a6' }}
            title="メニュー"
          >
            <svg className="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M5 12h.01M12 12h.01M19 12h.01M6 12a1 1 0 11-2 0 1 1 0 012 0zm7 0a1 1 0 11-2 0 1 1 0 012 0zm7 0a1 1 0 11-2 0 1 1 0 012 0z"
              />
            </svg>
          </button>
        </div>
      )}
    </div>
  )
}

export default function Sidebar() {
  const navigate = useNavigate()
  const { roomId: activeRoomIdEncoded } = useParams<{ roomId?: string }>()
  const activeRoomId = activeRoomIdEncoded ? decodeURIComponent(activeRoomIdEncoded) : undefined

  const rooms = useRoomsStore((s) => s.rooms)
  const syncing = useRoomsStore((s) => s.syncing)
  const error = useRoomsStore((s) => s.error)
  const markRoomRead = useRoomsStore((s) => s.markRoomRead)

  const [showCreate, setShowCreate] = useState(false)
  const [showPublic, setShowPublic] = useState(false)
  const [filter, setFilter] = useState<FilterTab>('all')
  const [search, setSearch] = useState('')

  const sorted = Object.values(rooms).sort(
    (a, b) => (b.lastEvent?.origin_server_ts ?? 0) - (a.lastEvent?.origin_server_ts ?? 0)
  )

  const filtered = sorted.filter((r) => {
    if (filter === 'unread' && r.notificationCount === 0 && r.highlightCount === 0) return false
    if (search) {
      const label = (r.name ?? r.roomId).toLowerCase()
      if (!label.includes(search.toLowerCase())) return false
    }
    return true
  })

  function handleSelect(roomId: string) {
    markRoomRead(roomId)
    navigate(`/room/${encodeURIComponent(roomId)}`)
  }

  return (
    <>
      <div
        className="flex w-[330px] shrink-0 flex-col"
        style={{ background: '#21262d', borderRight: '1px solid #2d3440' }}
      >
        {/* 検索バー */}
        <div className="px-3 pt-3 pb-2">
          <div
            className="flex items-center gap-2 rounded-lg px-3 py-2"
            style={{ background: '#15191e', border: '1px solid #2d3440' }}
          >
            <svg
              className="h-4 w-4 shrink-0"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              style={{ color: '#8d99a6' }}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
              />
            </svg>
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="検索"
              className="flex-1 bg-transparent text-sm focus:outline-none"
              style={{ color: '#e9edf1' }}
            />
            <kbd
              className="shrink-0 rounded px-1.5 py-0.5 text-xs"
              style={{ background: '#2d3440', color: '#636e7d' }}
            >
              ⌘K
            </kbd>
          </div>
        </div>

        {/* スペース/アプリ名ヘッダー */}
        <div className="flex items-center justify-between px-4 py-2">
          <button
            className="flex items-center gap-1.5 text-sm font-semibold transition-colors hover:opacity-80"
            style={{ color: '#e9edf1' }}
          >
            daberiba
            <svg
              className="h-3.5 w-3.5"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              style={{ color: '#8d99a6' }}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2.5}
                d="M19 9l-7 7-7-7"
              />
            </svg>
          </button>
          <div className="flex items-center gap-0.5">
            <button
              className="rounded p-1.5 transition-colors hover:bg-white/10"
              style={{ color: '#8d99a6' }}
              title="メニュー"
            >
              <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M5 12h.01M12 12h.01M19 12h.01M6 12a1 1 0 11-2 0 1 1 0 012 0zm7 0a1 1 0 11-2 0 1 1 0 012 0zm7 0a1 1 0 11-2 0 1 1 0 012 0z"
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
                  d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"
                />
              </svg>
            </button>
          </div>
        </div>

        {/* フィルタータブ */}
        <div className="flex gap-2 overflow-x-auto px-3 pb-2" style={{ scrollbarWidth: 'none' }}>
          {(['all', 'unread'] as const).map((tab) => (
            <button
              key={tab}
              onClick={() => setFilter(tab)}
              className="shrink-0 rounded-full px-3 py-1 text-xs font-medium transition-colors"
              style={
                filter === tab
                  ? { background: '#0dbd8b', color: 'white' }
                  : { background: '#2d3440', color: '#8d99a6' }
              }
            >
              {tab === 'all' ? 'すべて' : '未読'}
            </button>
          ))}
          <button
            onClick={() => setShowPublic(true)}
            className="shrink-0 rounded-full px-3 py-1 text-xs font-medium transition-colors"
            style={{ background: '#2d3440', color: '#8d99a6' }}
            title="パブリックルームを探す"
          >
            ルームを探す
          </button>
        </div>

        {/* エラーバー */}
        {error && (
          <div
            className="mx-3 mb-2 rounded-lg px-3 py-2 text-xs"
            style={{ background: '#7f1d1d', color: '#fca5a5' }}
          >
            {error}
          </div>
        )}

        {/* ルーム一覧 */}
        <div className="min-h-0 flex-1 overflow-y-auto">
          {syncing && sorted.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 text-center px-6">
              <div
                className="mb-3 h-5 w-5 animate-spin rounded-full border-2"
                style={{ borderColor: '#2d3440', borderTopColor: '#0dbd8b' }}
              />
              <p className="text-sm" style={{ color: '#8d99a6' }}>
                同期中…
              </p>
            </div>
          ) : filtered.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 text-center px-6">
              <div className="mb-4 text-4xl opacity-30">💬</div>
              <p className="mb-1 text-sm font-medium" style={{ color: '#e9edf1' }}>
                {search ? '見つかりません' : 'チャットがありません'}
              </p>
              {!search && (
                <p className="text-xs" style={{ color: '#8d99a6' }}>
                  誰かにメッセージを送るか、ルームを作成してください
                </p>
              )}
              {!search && (
                <div className="mt-6 flex flex-col gap-2 w-full">
                  <button
                    onClick={() => setShowPublic(true)}
                    className="flex items-center justify-center gap-2 rounded-full border px-4 py-2 text-sm transition-colors hover:bg-white/5"
                    style={{ borderColor: '#3d4555', color: '#e9edf1' }}
                  >
                    <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                      />
                    </svg>
                    チャットを開始
                  </button>
                  <button
                    onClick={() => setShowCreate(true)}
                    className="flex items-center justify-center gap-2 rounded-full border px-4 py-2 text-sm transition-colors hover:bg-white/5"
                    style={{ borderColor: '#3d4555', color: '#e9edf1' }}
                  >
                    <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M7 20l4-16m2 16l4-16M6 9h14M4 15h14"
                      />
                    </svg>
                    新しいルーム
                  </button>
                </div>
              )}
            </div>
          ) : (
            filtered.map((room) => (
              <RoomItem
                key={room.roomId}
                room={room}
                isActive={room.roomId === activeRoomId}
                onSelect={handleSelect}
              />
            ))
          )}
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
