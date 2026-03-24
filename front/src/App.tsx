import { useEffect } from 'react'
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { useAuthStore } from './stores/auth'
import { useUiStore } from './stores/ui'
import RequireAuth from './components/common/RequireAuth'
import LoginPage from './pages/LoginPage'
import HomePage from './pages/HomePage'
import RoomPage from './pages/RoomPage'

export default function App() {
  const hydrate = useAuthStore((s) => s.hydrate)
  const startNetworkWatch = useUiStore((s) => s.startNetworkWatch)

  useEffect(() => {
    hydrate()
    // オンライン/オフライン監視を開始する
    const stop = startNetworkWatch()
    return stop
  }, [hydrate, startNetworkWatch])

  return (
    <BrowserRouter>
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route
          path="/"
          element={
            <RequireAuth>
              <HomePage />
            </RequireAuth>
          }
        />
        <Route
          path="/room/:roomId"
          element={
            <RequireAuth>
              <RoomPage />
            </RequireAuth>
          }
        />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  )
}
