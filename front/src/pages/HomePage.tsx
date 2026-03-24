import { useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuthStore } from '../stores/auth'
import { useRoomsStore } from '../stores/rooms'
import { startSyncLoop } from '../api/sync'
import AppShell from '../components/layout/AppShell'
import RoomList from '../components/room/RoomList'

export default function HomePage() {
  const client = useAuthStore((s) => s.client)
  const { applySyncResponse, setSyncing, setError, reset } = useRoomsStore((s) => ({
    applySyncResponse: s.applySyncResponse,
    setSyncing: s.setSyncing,
    setError: s.setError,
    reset: s.reset,
  }))
  const navigate = useNavigate()

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
    <AppShell>
      <RoomList onSelect={(roomId) => navigate(`/room/${encodeURIComponent(roomId)}`)} />
    </AppShell>
  )
}
