import { useEffect } from 'react'
import { useAuthStore } from '../stores/auth'
import { useRoomsStore } from '../stores/rooms'

export default function HomePage() {
  const { client, userId, logout } = useAuthStore((s) => ({
    client: s.client,
    userId: s.userId,
    logout: s.logout,
  }))
  const { rooms, syncStatus, syncError, startSync, stopSync } = useRoomsStore()

  useEffect(() => {
    if (!client) return
    startSync(client)
    return () => stopSync()
  }, [client, startSync, stopSync])

  return (
    <div className="flex h-full flex-col bg-gray-950 text-white">
      {/* ヘッダー */}
      <header className="flex shrink-0 items-center justify-between border-b border-gray-800 px-4 py-3">
        <span className="font-semibold">daberiba</span>
        <div className="flex items-center gap-3">
          <span className="max-w-[200px] truncate text-sm text-gray-400">{userId}</span>
          <button
            onClick={() => logout()}
            className="rounded bg-gray-800 px-3 py-1 text-sm hover:bg-gray-700"
          >
            ログアウト
          </button>
        </div>
      </header>

      {/* sync ステータス */}
      {syncError && <div className="bg-red-900/50 px-4 py-2 text-sm text-red-300">{syncError}</div>}

      {/* ルーム一覧 */}
      <main className="flex-1 overflow-y-auto">
        {syncStatus === 'syncing' && rooms.length === 0 && (
          <div className="flex h-32 items-center justify-center text-gray-500 text-sm">
            読み込み中…
          </div>
        )}
        {rooms.length === 0 && syncStatus !== 'syncing' && (
          <div className="flex h-32 items-center justify-center text-gray-500 text-sm">
            参加中のルームはありません
          </div>
        )}
        <ul>
          {rooms.map((room) => (
            <li key={room.roomId}>
              <button className="flex w-full items-center gap-3 border-b border-gray-800/50 px-4 py-3 text-left hover:bg-gray-900">
                {/* アバタープレースホルダー */}
                <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-indigo-700 text-sm font-bold uppercase">
                  {room.name.slice(0, 1)}
                </div>

                <div className="min-w-0 flex-1">
                  <div className="flex items-center justify-between gap-2">
                    <span className="truncate font-medium">{room.name}</span>
                    {room.lastMessageTs > 0 && (
                      <span className="shrink-0 text-xs text-gray-500">
                        {formatTime(room.lastMessageTs)}
                      </span>
                    )}
                  </div>
                  {room.lastMessage && (
                    <p className="truncate text-sm text-gray-400">{room.lastMessage}</p>
                  )}
                </div>

                {/* 未読バッジ */}
                {room.notificationCount > 0 && (
                  <span
                    className={`shrink-0 rounded-full px-2 py-0.5 text-xs font-bold ${
                      room.highlightCount > 0
                        ? 'bg-red-500 text-white'
                        : 'bg-gray-700 text-gray-300'
                    }`}
                  >
                    {room.notificationCount}
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
      </main>
    </div>
  )
}

function formatTime(ts: number): string {
  const d = new Date(ts)
  const now = new Date()
  if (d.toDateString() === now.toDateString()) {
    return d.toLocaleTimeString('ja-JP', { hour: '2-digit', minute: '2-digit' })
  }
  return d.toLocaleDateString('ja-JP', { month: 'numeric', day: 'numeric' })
}
