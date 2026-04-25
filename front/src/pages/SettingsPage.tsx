/**
 * 設定ページ — デバイス管理・パスワード変更
 */
import { type FormEvent, useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuthStore } from '../stores/auth'
import { STORAGE_KEY } from '../api/client'
import {
  fetchDevices,
  renameDevice,
  deleteDevice,
  changePassword,
  type Device,
} from '../api/devices'
import ProfileModal from '../components/common/ProfileModal'

// ---------------------------------------------------------------------------
// デバイス一覧
// ---------------------------------------------------------------------------

function DeviceItem({
  device,
  isCurrent,
  onRename,
  onDelete,
}: {
  device: Device
  isCurrent: boolean
  onRename: (deviceId: string, name: string) => Promise<void>
  onDelete: (deviceId: string) => void
}) {
  const [editing, setEditing] = useState(false)
  const [nameInput, setNameInput] = useState(device.display_name ?? '')
  const [saving, setSaving] = useState(false)

  async function handleRename(e: FormEvent) {
    e.preventDefault()
    if (!nameInput.trim()) return
    setSaving(true)
    try {
      await onRename(device.device_id, nameInput.trim())
      setEditing(false)
    } finally {
      setSaving(false)
    }
  }

  const lastSeen = device.last_seen_ts
    ? new Date(device.last_seen_ts).toLocaleString('ja-JP', {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
      })
    : null

  return (
    <li className="px-4 py-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          {editing ? (
            <form onSubmit={(e) => void handleRename(e)} className="flex gap-2">
              <input
                value={nameInput}
                onChange={(e) => setNameInput(e.target.value)}
                autoFocus
                className="min-w-0 flex-1 rounded bg-gray-800 px-2 py-1 text-sm text-white focus:outline-none focus:ring-1 focus:ring-indigo-500"
              />
              <button
                type="submit"
                disabled={saving || !nameInput.trim()}
                className="rounded bg-indigo-600 px-2 py-1 text-xs text-white hover:bg-indigo-500 disabled:opacity-50"
              >
                保存
              </button>
              <button
                type="button"
                onClick={() => {
                  setEditing(false)
                  setNameInput(device.display_name ?? '')
                }}
                className="rounded bg-gray-700 px-2 py-1 text-xs text-gray-300 hover:bg-gray-600"
              >
                キャンセル
              </button>
            </form>
          ) : (
            <div className="flex items-center gap-2">
              <span className="truncate text-sm font-medium text-white">
                {device.display_name ?? device.device_id}
              </span>
              {isCurrent && (
                <span className="shrink-0 rounded-full bg-indigo-900 px-2 py-0.5 text-xs text-indigo-300">
                  このデバイス
                </span>
              )}
            </div>
          )}
          <p className="mt-0.5 truncate text-xs text-gray-500">{device.device_id}</p>
          {lastSeen && <p className="text-xs text-gray-600">最終ログイン: {lastSeen}</p>}
        </div>

        {!editing && (
          <div className="flex shrink-0 gap-1.5">
            <button
              onClick={() => setEditing(true)}
              className="rounded bg-gray-700 px-2 py-1 text-xs text-gray-300 hover:bg-gray-600"
            >
              名前変更
            </button>
            {!isCurrent && (
              <button
                onClick={() => onDelete(device.device_id)}
                className="rounded bg-red-900/60 px-2 py-1 text-xs text-red-300 hover:bg-red-800/60"
              >
                削除
              </button>
            )}
          </div>
        )}
      </div>
    </li>
  )
}

// ---------------------------------------------------------------------------
// 削除確認ダイアログ（パスワード入力付き）
// ---------------------------------------------------------------------------

function DeleteDeviceDialog({
  deviceId,
  onConfirm,
  onCancel,
}: {
  deviceId: string
  onConfirm: (password: string) => Promise<void>
  onCancel: () => void
}) {
  const [password, setPassword] = useState('')
  const [deleting, setDeleting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  async function handleSubmit(e: FormEvent) {
    e.preventDefault()
    if (!password) return
    setDeleting(true)
    setError(null)
    try {
      await onConfirm(password)
    } catch (err) {
      setError(err instanceof Error ? err.message : '削除に失敗しました')
      setDeleting(false)
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 px-4"
      onClick={(e) => {
        if (e.target === e.currentTarget) onCancel()
      }}
    >
      <div className="w-full max-w-sm rounded-2xl bg-gray-900 p-6 shadow-xl">
        <h3 className="mb-2 font-bold">デバイスを削除</h3>
        <p className="mb-4 text-sm text-gray-400">
          <span className="font-mono text-xs text-gray-500">{deviceId}</span>{' '}
          を削除するには現在のパスワードを入力してください。
        </p>
        <form onSubmit={(e) => void handleSubmit(e)} className="flex flex-col gap-3">
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="現在のパスワード"
            autoFocus
            className="rounded-lg bg-gray-800 px-4 py-2 text-white placeholder-gray-600 focus:outline-none focus:ring-2 focus:ring-red-500"
          />
          {error && <p className="text-sm text-red-400">{error}</p>}
          <div className="flex gap-2">
            <button
              type="button"
              onClick={onCancel}
              className="flex-1 rounded-lg bg-gray-700 py-2 text-sm text-gray-300 hover:bg-gray-600"
            >
              キャンセル
            </button>
            <button
              type="submit"
              disabled={deleting || !password}
              className="flex-1 rounded-lg bg-red-700 py-2 text-sm text-white hover:bg-red-600 disabled:opacity-50"
            >
              {deleting ? '削除中…' : '削除'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// パスワード変更フォーム
// ---------------------------------------------------------------------------

function PasswordSection({ userId }: { userId: string }) {
  const [current, setCurrent] = useState('')
  const [next, setNext] = useState('')
  const [confirm, setConfirm] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [saved, setSaved] = useState(false)

  async function handleSubmit(e: FormEvent) {
    e.preventDefault()
    if (next !== confirm) {
      setError('新しいパスワードが一致しません')
      return
    }
    if (next.length < 8) {
      setError('パスワードは 8 文字以上にしてください')
      return
    }

    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return

    setSaving(true)
    setError(null)
    setSaved(false)
    try {
      await changePassword(homeserver, token, userId, current, next)
      setSaved(true)
      setCurrent('')
      setNext('')
      setConfirm('')
      setTimeout(() => setSaved(false), 3000)
    } catch (err) {
      setError(err instanceof Error ? err.message : '変更に失敗しました')
    } finally {
      setSaving(false)
    }
  }

  return (
    <section>
      <h2 className="mb-3 px-4 text-xs font-semibold uppercase tracking-wider text-gray-500">
        パスワード変更
      </h2>
      <div className="rounded-xl bg-gray-900 px-4 py-4 mx-4">
        <form onSubmit={(e) => void handleSubmit(e)} className="flex flex-col gap-3">
          <input
            type="password"
            value={current}
            onChange={(e) => setCurrent(e.target.value)}
            placeholder="現在のパスワード"
            className="rounded-lg bg-gray-800 px-4 py-2 text-white placeholder-gray-600 focus:outline-none focus:ring-2 focus:ring-indigo-500"
          />
          <input
            type="password"
            value={next}
            onChange={(e) => setNext(e.target.value)}
            placeholder="新しいパスワード"
            className="rounded-lg bg-gray-800 px-4 py-2 text-white placeholder-gray-600 focus:outline-none focus:ring-2 focus:ring-indigo-500"
          />
          <input
            type="password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            placeholder="新しいパスワード（確認）"
            className="rounded-lg bg-gray-800 px-4 py-2 text-white placeholder-gray-600 focus:outline-none focus:ring-2 focus:ring-indigo-500"
          />
          {error && <p className="text-sm text-red-400">{error}</p>}
          {saved && <p className="text-sm text-green-400">パスワードを変更しました</p>}
          <button
            type="submit"
            disabled={saving || !current || !next || !confirm}
            className="rounded-lg bg-indigo-600 py-2 text-sm text-white hover:bg-indigo-500 disabled:opacity-50"
          >
            {saving ? '変更中…' : '変更する'}
          </button>
        </form>
      </div>
    </section>
  )
}

// ---------------------------------------------------------------------------
// SettingsPage
// ---------------------------------------------------------------------------

export default function SettingsPage() {
  const navigate = useNavigate()
  const userId = useAuthStore((s) => s.userId)
  const deviceId = useAuthStore((s) => s.deviceId)

  const [devices, setDevices] = useState<Device[]>([])
  const [loadingDevices, setLoadingDevices] = useState(true)
  const [devicesError, setDevicesError] = useState<string | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null)
  const [showProfile, setShowProfile] = useState(false)

  useEffect(() => {
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return

    fetchDevices(homeserver, token)
      .then(setDevices)
      .catch((err: unknown) =>
        setDevicesError(err instanceof Error ? err.message : '取得に失敗しました')
      )
      .finally(() => setLoadingDevices(false))
  }, [])

  async function handleRename(devId: string, name: string) {
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return
    await renameDevice(homeserver, token, devId, name)
    setDevices((prev) =>
      prev.map((d) => (d.device_id === devId ? { ...d, display_name: name } : d))
    )
  }

  async function handleDeleteConfirm(password: string) {
    if (!deleteTarget) return
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token || !userId) return
    await deleteDevice(homeserver, token, userId, deleteTarget, password)
    setDevices((prev) => prev.filter((d) => d.device_id !== deleteTarget))
    setDeleteTarget(null)
  }

  return (
    <>
      <div className="flex h-full flex-col" style={{ background: '#15191e', color: '#e9edf1' }}>
        {/* ヘッダー */}
        <div
          className="flex shrink-0 items-center gap-3 px-4 py-3"
          style={{ borderBottom: '1px solid #2d3440' }}
        >
          <button
            onClick={() => navigate(-1)}
            className="rounded p-1.5 transition-colors hover:bg-white/10"
            style={{ color: '#8d99a6' }}
          >
            <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M15 19l-7-7 7-7"
              />
            </svg>
          </button>
          <h1 className="text-base font-semibold" style={{ color: '#e9edf1' }}>
            設定
          </h1>
        </div>

        <div className="h-full overflow-y-auto py-6 flex flex-col gap-6">
          {/* プロフィール */}
          {userId && (
            <section>
              <h2
                className="mb-3 px-4 text-xs font-semibold uppercase tracking-wider"
                style={{ color: '#636e7d' }}
              >
                プロフィール
              </h2>
              <div
                className="mx-4 rounded-xl"
                style={{ background: '#21262d', border: '1px solid #2d3440' }}
              >
                <button
                  onClick={() => setShowProfile(true)}
                  className="flex w-full items-center gap-3 rounded-xl px-4 py-3 text-left transition-colors hover:bg-white/5"
                >
                  <div
                    className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full text-sm font-bold select-none"
                    style={{ background: '#0dbd8b', color: 'white' }}
                  >
                    {userId.charAt(1).toUpperCase()}
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium" style={{ color: '#e9edf1' }}>
                      表示名・アバターを変更
                    </p>
                    <p className="truncate text-xs" style={{ color: '#636e7d' }}>
                      {userId}
                    </p>
                  </div>
                  <span style={{ color: '#636e7d' }}>›</span>
                </button>
              </div>
            </section>
          )}

          {/* デバイス管理 */}
          <section>
            <h2
              className="mb-3 px-4 text-xs font-semibold uppercase tracking-wider"
              style={{ color: '#636e7d' }}
            >
              セッション管理
            </h2>
            <ul
              className="mx-4 rounded-xl divide-y"
              style={
                {
                  background: '#21262d',
                  border: '1px solid #2d3440',
                  borderColor: '#2d3440',
                  '--tw-divide-opacity': '1',
                  '--tw-divide-color': '#2d3440',
                } as React.CSSProperties
              }
            >
              {loadingDevices && (
                <li className="flex justify-center py-6">
                  <div
                    className="h-5 w-5 animate-spin rounded-full border-2"
                    style={{ borderColor: '#2d3440', borderTopColor: '#0dbd8b' }}
                  />
                </li>
              )}
              {devicesError && (
                <li className="px-4 py-3 text-sm" style={{ color: '#f87171' }}>
                  {devicesError}
                </li>
              )}
              {devices.map((d) => (
                <DeviceItem
                  key={d.device_id}
                  device={d}
                  isCurrent={d.device_id === deviceId}
                  onRename={handleRename}
                  onDelete={setDeleteTarget}
                />
              ))}
            </ul>
          </section>

          {/* パスワード変更 */}
          {userId && <PasswordSection userId={userId} />}
        </div>
      </div>

      {showProfile && userId && (
        <ProfileModal userId={userId} onClose={() => setShowProfile(false)} />
      )}

      {deleteTarget && (
        <DeleteDeviceDialog
          deviceId={deleteTarget}
          onConfirm={handleDeleteConfirm}
          onCancel={() => setDeleteTarget(null)}
        />
      )}
    </>
  )
}
