/**
 * プロフィール編集モーダル — 表示名の変更
 */
import { type FormEvent, useEffect, useState } from 'react'
import { STORAGE_KEY } from '../../api/client'

interface Props {
  userId: string
  onClose: () => void
}

async function fetchDisplayName(homeserver: string, token: string, userId: string): Promise<string> {
  const res = await fetch(
    `${homeserver}/_matrix/client/v3/profile/${encodeURIComponent(userId)}/displayname`,
    { headers: { Authorization: `Bearer ${token}` } }
  )
  if (!res.ok) return ''
  const data = (await res.json()) as { displayname?: string }
  return data.displayname ?? ''
}

async function putDisplayName(homeserver: string, token: string, userId: string, name: string): Promise<void> {
  const res = await fetch(
    `${homeserver}/_matrix/client/v3/profile/${encodeURIComponent(userId)}/displayname`,
    {
      method: 'PUT',
      headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
      body: JSON.stringify({ displayname: name }),
    }
  )
  if (!res.ok) {
    const err = (await res.json().catch(() => ({}))) as { error?: string }
    throw new Error(err.error ?? `PUT displayname failed: ${res.status}`)
  }
}

export default function ProfileModal({ userId, onClose }: Props) {
  const [displayName, setDisplayName] = useState('')
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [saved, setSaved] = useState(false)

  useEffect(() => {
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) { setLoading(false); return }

    fetchDisplayName(homeserver, token, userId)
      .then((name) => setDisplayName(name))
      .catch(() => {/* 取得失敗は空欄のまま */})
      .finally(() => setLoading(false))
  }, [userId])

  async function handleSave(e: FormEvent) {
    e.preventDefault()
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return

    setSaving(true)
    setError(null)
    setSaved(false)
    try {
      await putDisplayName(homeserver, token, userId, displayName.trim())
      setSaved(true)
      setTimeout(() => setSaved(false), 3000)
    } catch (err) {
      setError(err instanceof Error ? err.message : '保存に失敗しました')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 px-4"
      onClick={(e) => { if (e.target === e.currentTarget) onClose() }}
    >
      <div className="w-full max-w-sm rounded-2xl bg-gray-900 p-6 shadow-xl">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-bold">プロフィール</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white text-xl leading-none">×</button>
        </div>

        <p className="mb-4 truncate text-sm text-gray-400">{userId}</p>

        <form onSubmit={(e) => void handleSave(e)} className="flex flex-col gap-3">
          <div>
            <label className="mb-1 block text-xs text-gray-500">表示名</label>
            <input
              type="text"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              disabled={loading}
              maxLength={100}
              placeholder="表示名を入力"
              className="w-full rounded-lg bg-gray-800 px-4 py-2 text-white placeholder-gray-600 focus:outline-none focus:ring-2 focus:ring-indigo-500 disabled:opacity-50"
            />
          </div>

          {error && <p className="text-sm text-red-400">{error}</p>}
          {saved && <p className="text-sm text-green-400">保存しました</p>}

          <button
            type="submit"
            disabled={saving || loading || !displayName.trim()}
            className="rounded-lg bg-indigo-600 py-2 text-sm text-white hover:bg-indigo-500 disabled:opacity-50"
          >
            {saving ? '保存中…' : '保存'}
          </button>
        </form>
      </div>
    </div>
  )
}
