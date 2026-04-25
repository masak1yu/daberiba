import { useEffect } from 'react'
import { Outlet } from 'react-router-dom'
import { useShallow } from 'zustand/react/shallow'
import { useAuthStore } from '../../stores/auth'
import { useRoomsStore } from '../../stores/rooms'
import { startSyncLoop } from '../../api/sync'
import Sidebar from './Sidebar'
import ToastStack from '../common/ToastStack'

export default function ClientLayout() {
  const client = useAuthStore((s) => s.client)
  const { applySyncResponse, setSyncing, setError, reset } = useRoomsStore(
    useShallow((s) => ({
      applySyncResponse: s.applySyncResponse,
      setSyncing: s.setSyncing,
      setError: s.setError,
      reset: s.reset,
    }))
  )

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
    <div className="flex h-dvh overflow-hidden" style={{ background: '#15191e', color: '#e9edf1' }}>
      <Sidebar />
      <main className="flex min-w-0 flex-1 flex-col overflow-hidden">
        <Outlet />
      </main>
      <ToastStack />
    </div>
  )
}
