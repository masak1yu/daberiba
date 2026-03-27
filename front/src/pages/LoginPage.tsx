import { type FormEvent, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { login } from '../api/auth'
import { useAuthStore } from '../stores/auth'

export default function LoginPage() {
  const navigate = useNavigate()
  const setClient = useAuthStore((s) => s.setClient)

  const [homeserver, setHomeserver] = useState('http://localhost:8448')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  async function handleSubmit(e: FormEvent) {
    e.preventDefault()
    setError(null)
    setLoading(true)
    try {
      const result = await login({ homeserver, username, password })
      setClient(result.client, result.userId, result.deviceId)
      navigate('/', { replace: true })
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'ログインに失敗しました')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="flex min-h-full flex-col items-center justify-center bg-gray-950 px-4">
      <div className="w-full max-w-sm space-y-6">
        <h1 className="text-center text-2xl font-bold text-white">daberiba</h1>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-1">
            <label className="block text-sm text-gray-400">ホームサーバー</label>
            <input
              type="url"
              value={homeserver}
              onChange={(e) => setHomeserver(e.target.value)}
              required
              className="w-full rounded-lg bg-gray-800 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
          </div>

          <div className="space-y-1">
            <label className="block text-sm text-gray-400">ユーザー名</label>
            <input
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              required
              autoComplete="username"
              className="w-full rounded-lg bg-gray-800 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
          </div>

          <div className="space-y-1">
            <label className="block text-sm text-gray-400">パスワード</label>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              autoComplete="current-password"
              className="w-full rounded-lg bg-gray-800 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
          </div>

          {error && <p className="text-sm text-red-400">{error}</p>}

          <button
            type="submit"
            disabled={loading}
            className="w-full rounded-lg bg-indigo-600 py-2 font-semibold text-white hover:bg-indigo-500 disabled:opacity-50"
          >
            {loading ? 'ログイン中…' : 'ログイン'}
          </button>
        </form>
      </div>
    </div>
  )
}
