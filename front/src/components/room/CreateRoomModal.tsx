/**
 * 新規ルーム作成モーダル
 */
import { type FormEvent, useRef, useState } from 'react'
import { createRoom } from '../../api/rooms'
import { STORAGE_KEY } from '../../api/client'

interface Props {
  onCreated: (roomId: string) => void
  onClose: () => void
}

export default function CreateRoomModal({ onCreated, onClose }: Props) {
  const [name, setName] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  async function handleSubmit(e: FormEvent) {
    e.preventDefault()
    const trimmed = name.trim()
    if (!trimmed || loading) return

    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return

    setLoading(true)
    setError(null)
    try {
      const { room_id } = await createRoom(homeserver, token, trimmed)
      onCreated(room_id)
    } catch (err) {
      setError(err instanceof Error ? err.message : '作成に失敗しました')
    } finally {
      setLoading(false)
    }
  }

  return (
    // バックドロップ
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 px-4"
      onClick={(e) => { if (e.target === e.currentTarget) onClose() }}
    >
      <div className="w-full max-w-sm rounded-2xl bg-gray-900 p-6 shadow-xl">
        <h2 className="mb-4 text-lg font-bold">新しいルームを作成</h2>

        <form onSubmit={(e) => void handleSubmit(e)} className="flex flex-col gap-3">
          <input
            ref={inputRef}
            autoFocus
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="ルーム名"
            maxLength={80}
            className="rounded-lg bg-gray-800 px-4 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
          />

          {error && <p className="text-sm text-red-400">{error}</p>}

          <div className="flex gap-2 pt-1">
            <button
              type="button"
              onClick={onClose}
              className="flex-1 rounded-lg border border-gray-700 py-2 text-sm text-gray-400 hover:bg-gray-800"
            >
              キャンセル
            </button>
            <button
              type="submit"
              disabled={!name.trim() || loading}
              className="flex-1 rounded-lg bg-indigo-600 py-2 text-sm text-white hover:bg-indigo-500 disabled:opacity-50"
            >
              {loading ? '作成中…' : '作成'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
