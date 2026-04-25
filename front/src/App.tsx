import { useEffect } from 'react'
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { useAuthStore } from './stores/auth'
import { useUiStore } from './stores/ui'
import RequireAuth from './components/common/RequireAuth'
import ClientLayout from './components/layout/ClientLayout'
import LoginPage from './pages/LoginPage'
import HomePage from './pages/HomePage'
import RoomPage from './pages/RoomPage'
import SettingsPage from './pages/SettingsPage'

export default function App() {
  const hydrate = useAuthStore((s) => s.hydrate)
  const startNetworkWatch = useUiStore((s) => s.startNetworkWatch)

  useEffect(() => {
    hydrate()
    const stop = startNetworkWatch()
    return stop
  }, [hydrate, startNetworkWatch])

  return (
    <BrowserRouter>
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route
          element={
            <RequireAuth>
              <ClientLayout />
            </RequireAuth>
          }
        >
          <Route path="/" element={<HomePage />} />
          <Route path="/room/:roomId" element={<RoomPage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  )
}
