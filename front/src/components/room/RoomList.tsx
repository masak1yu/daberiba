/**
 * ルーム一覧 — 最終メッセージ時刻でソートして表示
 */
import { useRoomsStore, type RoomSummary } from '../../stores/rooms'

interface Props {
  onSelect: (roomId: string) => void
}

function RoomItem({ room, onSelect }: { room: RoomSummary; onSelect: (id: string) => void }) {
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
    <li>
      <button
        onClick={() => onSelect(room.roomId)}
        className="flex w-full items-center gap-3 px-4 py-3 text-left transition hover:bg-gray-800 active:bg-gray-700"
      >
        {/* 頭文字アバター */}
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-indigo-700 text-sm font-bold uppercase select-none">
          {label.charAt(0)}
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-2">
            <span className="truncate font-medium text-white">{label}</span>
            {time && <span className="shrink-0 text-xs text-gray-500">{time}</span>}
          </div>
          {lastBody && <p className="truncate text-sm text-gray-400">{lastBody}</p>}
        </div>

        {room.highlightCount > 0 ? (
          <span className="shrink-0 rounded-full bg-red-600 px-2 py-0.5 text-xs font-bold text-white">
            {room.highlightCount}
          </span>
        ) : room.notificationCount > 0 ? (
          <span className="shrink-0 rounded-full bg-indigo-600 px-2 py-0.5 text-xs font-bold text-white">
            {room.notificationCount}
          </span>
        ) : null}
      </button>
    </li>
  )
}

export default function RoomList({ onSelect }: Props) {
  const rooms = useRoomsStore((s) => s.rooms)
  const syncing = useRoomsStore((s) => s.syncing)

  const sorted = Object.values(rooms).sort(
    (a, b) => (b.lastEvent?.origin_server_ts ?? 0) - (a.lastEvent?.origin_server_ts ?? 0)
  )

  if (syncing && sorted.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-gray-500">
        <span className="animate-pulse">同期中…</span>
      </div>
    )
  }

  if (sorted.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-gray-500 text-sm">
        参加中のルームがありません
      </div>
    )
  }

  return (
    <ul className="h-full divide-y divide-gray-800 overflow-y-auto">
      {sorted.map((room) => (
        <RoomItem key={room.roomId} room={room} onSelect={onSelect} />
      ))}
    </ul>
  )
}
