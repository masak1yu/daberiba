import { type FormEvent, useEffect, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { fetchLoginFlows, login, loginWithToken, type IdentityProvider } from '../api/auth'
import { useAuthStore } from '../stores/auth'

export default function LoginPage() {
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const setClient = useAuthStore((s) => s.setClient)

  const [homeserver, setHomeserver] = useState(
    () => searchParams.get('homeserver') ?? 'http://localhost:8448'
  )
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [providers, setProviders] = useState<IdentityProvider[]>([])

  // SSO コールバック: URL に loginToken がある場合は自動ログイン
  useEffect(() => {
    const loginToken = searchParams.get('loginToken')
    const hs = searchParams.get('homeserver') ?? homeserver
    if (!loginToken) return
    setLoading(true)
    loginWithToken({ homeserver: hs, loginToken })
      .then((result) => {
        setClient(result.client, result.userId, result.deviceId)
        navigate('/', { replace: true })
      })
      .catch((err: unknown) => {
        setError(err instanceof Error ? err.message : 'SSOログインに失敗しました')
        setLoading(false)
      })
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  // ホームサーバーが変わるたびにログインフローを取得
  useEffect(() => {
    if (!homeserver) return
    fetchLoginFlows(homeserver)
      .then((flows) => setProviders(flows.identity_providers ?? []))
      .catch(() => setProviders([]))
  }, [homeserver])

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

  function handleSsoLogin(providerId: string) {
    // コールバック後に戻る URL（homeserver をクエリに含める）
    const callbackUrl = `${window.location.origin}/login?homeserver=${encodeURIComponent(homeserver)}`
    const redirectTo = `${homeserver}/_matrix/client/v3/login/sso/redirect/${providerId}?redirectUrl=${encodeURIComponent(callbackUrl)}`
    window.location.href = redirectTo
  }

  return (
    <div className="flex min-h-full flex-col items-center justify-center bg-gray-950 px-4">
      <div className="w-full max-w-sm space-y-6">
        <h1 className="text-center text-2xl font-bold text-white">daberiba</h1>

        {/* SSO ボタン */}
        {providers.length > 0 && (
          <div className="space-y-2">
            {providers.map((p) => (
              <button
                key={p.id}
                type="button"
                onClick={() => handleSsoLogin(p.id)}
                disabled={loading}
                className="flex w-full items-center justify-center gap-3 rounded-lg py-2.5 text-sm font-medium transition-opacity disabled:opacity-50"
                style={providerButtonStyle(p.id)}
              >
                <ProviderIcon id={p.id} />
                {providerLabel(p.id, p.name)}
              </button>
            ))}
            <div className="relative flex items-center py-1">
              <div className="flex-grow border-t border-gray-700" />
              <span className="mx-3 shrink-0 text-xs text-gray-500">または</span>
              <div className="flex-grow border-t border-gray-700" />
            </div>
          </div>
        )}

        {/* パスワードログインフォーム */}
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

function providerButtonStyle(id: string): React.CSSProperties {
  switch (id) {
    case 'google':
      return { background: '#ffffff', color: '#3c4043' }
    case 'github':
      return { background: '#24292e', color: '#ffffff' }
    case 'apple':
      return { background: '#000000', color: '#ffffff' }
    default:
      return { background: '#374151', color: '#ffffff' }
  }
}

function providerLabel(id: string, name: string): string {
  switch (id) {
    case 'apple':
      return `Sign in with ${name}`
    default:
      return `${name} でログイン`
  }
}

function ProviderIcon({ id }: { id: string }) {
  switch (id) {
    case 'google':
      return (
        <svg viewBox="0 0 24 24" className="h-5 w-5" aria-hidden="true">
          <path
            d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
            fill="#4285F4"
          />
          <path
            d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
            fill="#34A853"
          />
          <path
            d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
            fill="#FBBC05"
          />
          <path
            d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
            fill="#EA4335"
          />
        </svg>
      )
    case 'github':
      return (
        <svg viewBox="0 0 24 24" className="h-5 w-5 fill-white" aria-hidden="true">
          <path d="M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12" />
        </svg>
      )
    case 'apple':
      return (
        <svg viewBox="0 0 24 24" className="h-5 w-5 fill-white" aria-hidden="true">
          <path d="M12.152 6.896c-.948 0-2.415-1.078-3.96-1.04-2.04.027-3.91 1.183-4.961 3.014-2.117 3.675-.54 9.103 1.519 12.09 1.013 1.454 2.208 3.09 3.792 3.029 1.52-.065 2.09-.987 3.935-.987 1.831 0 2.35.987 3.96.948 1.637-.026 2.676-1.48 3.676-2.948 1.156-1.688 1.636-3.325 1.662-3.415-.039-.013-3.182-1.221-3.22-4.857-.026-3.04 2.48-4.494 2.597-4.559-1.429-2.09-3.623-2.324-4.39-2.376-2-.156-3.675 1.09-4.61 1.09zM15.53 3.83c.843-1.012 1.4-2.427 1.245-3.83-1.207.052-2.662.805-3.532 1.818-.78.896-1.454 2.338-1.273 3.714 1.338.104 2.715-.688 3.559-1.701" />
        </svg>
      )
    default:
      return null
  }
}
