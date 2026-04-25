import { type ReactNode } from 'react'
import { Navigate } from 'react-router-dom'
import { useAuthStore } from '../../stores/auth'

/** 未認証時に /login へリダイレクトするラッパー */
export default function RequireAuth({ children }: { children: ReactNode }) {
  const client = useAuthStore((s) => s.client)
  if (!client) return <Navigate to="/login" replace />
  return <>{children}</>
}
