import { useAuthStore } from '../stores/auth'

// Phase 3 でルーム一覧・sync を実装する
export default function HomePage() {
  const { userId, logout } = useAuthStore((s) => ({
    userId: s.userId,
    logout: s.logout,
  }))

  return (
    <div className="flex min-h-full flex-col bg-gray-950 text-white">
      <header className="flex items-center justify-between border-b border-gray-800 px-4 py-3">
        <span className="font-semibold">daberiba</span>
        <div className="flex items-center gap-3">
          <span className="text-sm text-gray-400">{userId}</span>
          <button
            onClick={() => logout()}
            className="rounded bg-gray-800 px-3 py-1 text-sm hover:bg-gray-700"
          >
            ログアウト
          </button>
        </div>
      </header>
      <main className="flex flex-1 items-center justify-center text-gray-500">
        <p>ルーム一覧は Phase 3 で実装します</p>
      </main>
    </div>
  )
}
