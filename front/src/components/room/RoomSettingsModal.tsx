/**
 * ルーム設定モーダル — 名前・トピックの変更
 */
import { type FormEvent, useEffect, useState } from 'react'
import { STORAGE_KEY } from '../../api/client'
import { setRoomName, setRoomTopic } from '../../api/roomState'

interface Props {
  roomId: string
  currentName?: string
  currentTopic?: string
  onClose: () => void
}

export default function RoomSettingsModal({ roomId, currentName, currentTopic, onClose }: Props) {
  const [name, setName] = useState(currentName ?? '')
  const [topic, setTopic] = useState(currentTopic ?? '')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [saved, setSaved] = useState(false)

  useEffect(() => {
    setName(currentName ?? '')
    setTopic(currentTopic ?? '')
  }, [currentName, currentTopic])

  async function handleSave(e: FormEvent) {
    e.preventDefault()
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return

    setSaving(true)
    setError(null)
    setSaved(false)
    try {
      const ops: Promise<void>[] = []
      if (name.trim() !== (currentName ?? '')) {
        ops.push(setRoomName(homeserver, token, roomId, name.trim()))
      }
      if (topic !== (currentTopic ?? '')) {
        ops.push(setRoomTopic(homeserver, token, roomId, topic))
      }
      await Promise.all(ops)
      setSaved(true)
      setTimeout(() => setSaved(false), 2000)
    } catch (err) {
      setError(err instanceof Error ? err.message : '保存に失敗しました')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 px-4"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose()
      }}
    >
      <div className="w-full max-w-sm rounded-2xl bg-gray-900 p-6 shadow-xl">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="font-bold">ルーム設定</h2>
          <button onClick={onClose} className="text-xl leading-none text-gray-400 hover:text-white">
            ×
          </button>
        </div>

        <form onSubmit={(e) => void handleSave(e)} className="flex flex-col gap-3">
          <div>
            <label className="mb-1 block text-xs text-gray-500">ルーム名</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              maxLength={255}
              placeholder="ルーム名"
              className="w-full rounded-lg bg-gray-800 px-4 py-2 text-white placeholder-gray-600 focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
          </div>
          <div>
            <label className="mb-1 block text-xs text-gray-500">トピック</label>
            <textarea
              value={topic}
              onChange={(e) => setTopic(e.target.value)}
              rows={3}
              placeholder="ルームのトピック（任意）"
              className="w-full resize-none rounded-lg bg-gray-800 px-4 py-2 text-white placeholder-gray-600 focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
          </div>

          {error && <p className="text-sm text-red-400">{error}</p>}
          {saved && <p className="text-sm text-green-400">保存しました</p>}

          <button
            type="submit"
            disabled={saving}
            className="rounded-lg bg-indigo-600 py-2 text-sm text-white hover:bg-indigo-500 disabled:opacity-50"
          >
            {saving ? '保存中…' : '保存'}
          </button>
        </form>
      </div>
    </div>
  )
}
