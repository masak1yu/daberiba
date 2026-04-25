import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useShallow } from 'zustand/react/shallow'
import { useAuthStore } from '../stores/auth'
import { useRoomsStore } from '../stores/rooms'
import { startSyncLoop } from '../api/sync'
import AppShell from '../components/layout/AppShell'
import RoomList from '../components/room/RoomList'
import CreateRoomModal from '../components/room/CreateRoomModal'
import PublicRoomsModal from '../components/room/PublicRoomsModal'
import ProfileModal from '../components/common/ProfileModal'

export default function HomePage() {
  const client = useAuthStore((s) => s.client)
  const userId = useAuthStore((s) => s.userId)
  const { applySyncResponse, setSyncing, setError, reset, markRoomRead } = useRoomsStore(
    useShallow((s) => ({
      applySyncResponse: s.applySyncResponse,
      setSyncing: s.setSyncing,
      setError: s.setError,
      reset: s.reset,
      markRoomRead: s.markRoomRead,
    }))
  )
  const navigate = useNavigate()
  const [showCreate, setShowCreate] = useState(false)
  const [showPublic, setShowPublic] = useState(false)
  const [showProfile, setShowProfile] = useState(false)

  useEffect(() => {
    if (!client) return
    setSyncing(true)

    const stop = startSyncLoop(
      client,
      (data) => applySyncResponse(data),
      (err) => setError(err instanceof Error ? err.message : String(err))
    )

    return () => {
      stop()
      reset()
    }
  }, [client, applySyncResponse, setSyncing, setError, reset])

  return (
    <>
      <AppShell
        headerRight={
          <div className="ml-2 flex items-center gap-1.5">
            {/* 設定 */}
            <button
              onClick={() => navigate('/settings')}
              className="text-gray-400 hover:text-white text-sm"
              title="設定"
            >
              ⚙
            </button>
            {/* パブリックルーム検索 */}
            <button
              onClick={() => setShowPublic(true)}
              className="text-gray-400 hover:text-white text-sm"
              title="パブリックルームを探す"
            >
              🔍
            </button>
            {/* 新規ルーム作成 */}
            <button
              onClick={() => setShowCreate(true)}
              className="flex h-7 w-7 items-center justify-center rounded-full bg-indigo-600 text-lg leading-none text-white hover:bg-indigo-500"
              title="新しいルームを作成"
            >
              +
            </button>
          </div>
        }
      >
        <RoomList
          onSelect={(roomId) => {
            markRoomRead(roomId)
            navigate(`/room/${encodeURIComponent(roomId)}`)
          }}
        />
      </AppShell>

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

      {showProfile && userId && (
        <ProfileModal userId={userId} onClose={() => setShowProfile(false)} />
      )}
    </>
  )
}
