/**
 * アプリ共通シェル — iOS SafeArea 対応ヘッダー + コンテンツ領域 + トースト
 */
import type { ReactNode } from 'react'
import { useAuthStore } from '../../stores/auth'
import { useRoomsStore } from '../../stores/rooms'
import ToastStack from '../common/ToastStack'

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
    // h-dvh: iOS で動的ビューポート（アドレスバー表示/非表示を考慮）
    <div className="flex h-dvh flex-col bg-gray-950 text-white">
      <header
        className="flex shrink-0 items-center gap-2 border-b border-gray-800 bg-gray-900 px-4 py-3"
        style={{ paddingTop: 'max(env(safe-area-inset-top), 0.75rem)' }}
      >
        {showBack ? (
          <button onClick={onBack} className="mr-1 text-indigo-400 hover:text-indigo-300">
            ‹ 戻る
          </button>
        ) : (
          <span className="font-bold text-indigo-400">daberiba</span>
        )}
        {title && <span className="flex-1 truncate text-center font-medium">{title}</span>}
        <span className="ml-auto max-w-[40%] truncate text-sm text-gray-400">{userId}</span>
        <button
          onClick={() => void logout()}
          className="ml-2 text-sm text-gray-500 hover:text-gray-300"
        >
          ログアウト
        </button>
      </header>

      {/* sync エラーバー */}
      {error && (
        <div className="shrink-0 bg-red-900/80 px-4 py-1 text-center text-sm text-red-200">
          {error}
        </div>
      )}

      {/* コンテンツ — 下部 SafeArea はコンテンツ側で個別対応 */}
      <main className="min-h-0 flex-1 overflow-hidden">{children}</main>

      {/* オフライン/オンライン トースト */}
      <ToastStack />
    </div>
  )
}
