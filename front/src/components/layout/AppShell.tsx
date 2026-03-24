/**
 * アプリ共通シェル — iOS SafeArea 対応ヘッダー + コンテンツ領域
 */
import type { ReactNode } from 'react'
import { useAuthStore } from '../../stores/auth'
import { useRoomsStore } from '../../stores/rooms'

interface Props {
  children: ReactNode
  title?: string
  showBack?: boolean
  onBack?: () => void
}

export default function AppShell({ children, title, showBack, onBack }: Props) {
  const userId = useAuthStore((s) => s.userId)
  const logout = useAuthStore((s) => s.logout)
  const error = useRoomsStore((s) => s.error)

  return (
    <div
      className="flex h-dvh flex-col bg-gray-950 text-white"
      style={{
        paddingTop: 'env(safe-area-inset-top)',
        paddingBottom: 'env(safe-area-inset-bottom)',
      }}
    >
      <header className="flex shrink-0 items-center gap-2 border-b border-gray-800 bg-gray-900 px-4 py-3">
        {showBack ? (
          <button onClick={onBack} className="text-indigo-400 hover:text-indigo-300 mr-1">
            ‹ 戻る
          </button>
        ) : (
          <span className="font-bold text-indigo-400">daberiba</span>
        )}
        {title && <span className="flex-1 truncate text-center font-medium">{title}</span>}
        <span className="ml-auto truncate text-sm text-gray-400 max-w-[40%]">{userId}</span>
        <button
          onClick={() => void logout()}
          className="ml-2 text-sm text-gray-500 hover:text-gray-300"
        >
          ログアウト
        </button>
      </header>

      {error && (
        <div className="shrink-0 bg-red-900/80 px-4 py-1 text-center text-sm text-red-200">
          {error}
        </div>
      )}

      <main className="min-h-0 flex-1 overflow-hidden">{children}</main>
    </div>
  )
}
