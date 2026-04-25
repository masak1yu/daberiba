/**
 * パブリックルーム検索モーダル — キーワード検索 + 参加
 */
import { useEffect, useRef, useState } from 'react'
import { STORAGE_KEY } from '../../api/client'
import { fetchPublicRooms, joinRoom, type PublicRoom } from '../../api/publicRooms'

interface Props {
  onJoined: (roomId: string) => void
  onClose: () => void
}

function RoomRow({
  room,
  onJoin,
  joining,
}: {
  room: PublicRoom
  onJoin: (roomId: string) => void
  joining: boolean
}) {
  return (
    <li className="flex items-center gap-3 px-4 py-3">
      {/* 頭文字アバター */}
      <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-indigo-700 text-sm font-bold uppercase select-none">
        {(room.name ?? room.room_id).charAt(0)}
      </div>
      <div className="min-w-0 flex-1">
        <p className="truncate font-medium text-white">{room.name ?? room.room_id}</p>
        {room.topic && <p className="truncate text-xs text-gray-400">{room.topic}</p>}
        <p className="text-xs text-gray-500">{room.num_joined_members} 人</p>
      </div>
      <button
        onClick={() => onJoin(room.room_id)}
        disabled={joining}
        className="shrink-0 rounded-lg bg-indigo-600 px-3 py-1.5 text-xs text-white hover:bg-indigo-500 disabled:opacity-50"
      >
        参加
      </button>
    </li>
  )
}

export default function PublicRoomsModal({ onJoined, onClose }: Props) {
  const [query, setQuery] = useState('')
  const [rooms, setRooms] = useState<PublicRoom[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [joiningId, setJoiningId] = useState<string | null>(null)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // 初回 + クエリ変更時にデバウンス検索
  useEffect(() => {
    if (timerRef.current) clearTimeout(timerRef.current)
    timerRef.current = setTimeout(() => {
      const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
      const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
      if (!homeserver || !token) return

      setLoading(true)
      setError(null)
      fetchPublicRooms(homeserver, token, query || undefined)
        .then((data) => setRooms(data.chunk))
        .catch((err: unknown) =>
          setError(err instanceof Error ? err.message : '取得に失敗しました')
        )
        .finally(() => setLoading(false))
    }, 300)

    return () => {
      if (timerRef.current) clearTimeout(timerRef.current)
    }
  }, [query])

  async function handleJoin(roomId: string) {
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return

    setJoiningId(roomId)
    try {
      const joined = await joinRoom(homeserver, token, roomId)
      onJoined(joined)
    } catch (err) {
      setError(err instanceof Error ? err.message : '参加に失敗しました')
      setJoiningId(null)
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 px-4"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div className="flex w-full max-w-md flex-col rounded-2xl bg-gray-900 shadow-xl max-h-[80vh]">
        {/* ヘッダー */}
        <div className="flex items-center justify-between border-b border-gray-800 px-4 py-3">
          <h2 className="font-bold">パブリックルームを探す</h2>
          <button onClick={onClose} className="text-xl leading-none text-gray-400 hover:text-white">
            ×
          </button>
        </div>

        {/* 検索 */}
        <div className="p-3">
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="ルーム名・トピックで検索"
            autoFocus
            className="w-full rounded-lg bg-gray-800 px-4 py-2 text-white placeholder-gray-600 focus:outline-none focus:ring-2 focus:ring-indigo-500"
          />
        </div>

        {/* 結果 */}
        <div className="min-h-0 flex-1 overflow-y-auto">
          {loading && (
            <div className="flex justify-center py-8">
              <div className="h-5 w-5 animate-spin rounded-full border-2 border-gray-500 border-t-transparent" />
            </div>
          )}
          {error && <p className="px-4 py-3 text-sm text-red-400">{error}</p>}
          {!loading && !error && rooms.length === 0 && (
            <p className="px-4 py-8 text-center text-sm text-gray-500">ルームが見つかりません</p>
          )}
          {!loading && rooms.length > 0 && (
            <ul className="divide-y divide-gray-800">
              {rooms.map((room) => (
                <RoomRow
                  key={room.room_id}
                  room={room}
                  onJoin={(id) => void handleJoin(id)}
                  joining={joiningId === room.room_id}
                />
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  )
}
