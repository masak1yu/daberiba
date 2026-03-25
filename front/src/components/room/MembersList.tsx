/**
 * メンバーリストパネル — ルームの参加者を表示するスライドインドロワー
 */
import { useEffect, useState } from 'react'
import { fetchMembers, type RoomMember } from '../../api/rooms'
import { STORAGE_KEY } from '../../api/client'

interface Props {
  roomId: string
  onClose: () => void
}

function MemberItem({ member }: { member: RoomMember }) {
  const label = member.displayName ?? member.userId
  const initial = label.startsWith('@') ? label.charAt(1).toUpperCase() : label.charAt(0).toUpperCase()

  return (
    <li className="flex items-center gap-3 px-4 py-2">
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-indigo-700 text-sm font-bold select-none">
        {initial}
      </div>
      <div className="min-w-0">
        {member.displayName && (
          <p className="truncate text-sm font-medium text-white">{member.displayName}</p>
        )}
        <p className="truncate text-xs text-gray-400">{member.userId}</p>
      </div>
    </li>
  )
}

export default function MembersList({ roomId, onClose }: Props) {
  const [members, setMembers] = useState<RoomMember[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return

    setLoading(true)
    fetchMembers(homeserver, token, roomId)
      .then((data) => {
        // displayName 優先、なければ userId でソート
        const sorted = [...data].sort((a, b) =>
          (a.displayName ?? a.userId).localeCompare(b.displayName ?? b.userId, 'ja')
        )
        setMembers(sorted)
      })
      .catch((err: unknown) => setError(err instanceof Error ? err.message : '取得失敗'))
      .finally(() => setLoading(false))
  }, [roomId])

  return (
    <>
      {/* バックドロップ */}
      <div className="fixed inset-0 z-40 bg-black/40" onClick={onClose} />

      {/* ドロワー本体（右から） */}
      <div className="fixed inset-y-0 right-0 z-50 flex w-72 max-w-full flex-col bg-gray-900 shadow-xl"
        style={{ paddingTop: 'env(safe-area-inset-top)', paddingBottom: 'env(safe-area-inset-bottom)' }}
      >
        <div className="flex items-center justify-between border-b border-gray-800 px-4 py-3">
          <h2 className="font-semibold">メンバー</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white text-xl leading-none">
            ×
          </button>
        </div>

        {loading && (
          <div className="flex flex-1 items-center justify-center">
            <div className="h-5 w-5 animate-spin rounded-full border-2 border-gray-500 border-t-transparent" />
          </div>
        )}

        {error && (
          <p className="p-4 text-sm text-red-400">{error}</p>
        )}

        {!loading && !error && (
          <>
            <p className="px-4 pt-3 pb-1 text-xs text-gray-500">{members.length} 人参加中</p>
            <ul className="flex-1 overflow-y-auto divide-y divide-gray-800/50">
              {members.map((m) => (
                <MemberItem key={m.userId} member={m} />
              ))}
            </ul>
          </>
        )}
      </div>
    </>
  )
}
