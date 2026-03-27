/**
 * メンバーリストパネル — ルームの参加者を表示 + 招待フォーム
 */
import { type FormEvent, useEffect, useState } from 'react'
import { fetchMembers, inviteUser, type RoomMember } from '../../api/rooms'
import { STORAGE_KEY } from '../../api/client'
import Avatar from '../common/Avatar'

interface Props {
  roomId: string
  onClose: () => void
}

function MemberItem({ member }: { member: RoomMember }) {
  return (
    <li className="flex items-center gap-3 px-4 py-2">
      <Avatar userId={member.userId} displayName={member.displayName} avatarUrl={member.avatarUrl} />
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

  const [inviteInput, setInviteInput] = useState('')
  const [inviting, setInviting] = useState(false)
  const [inviteError, setInviteError] = useState<string | null>(null)
  const [inviteSuccess, setInviteSuccess] = useState(false)

  useEffect(() => {
    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return

    setLoading(true)
    fetchMembers(homeserver, token, roomId)
      .then((data) => {
        const sorted = [...data].sort((a, b) =>
          (a.displayName ?? a.userId).localeCompare(b.displayName ?? b.userId, 'ja')
        )
        setMembers(sorted)
      })
      .catch((err: unknown) => setError(err instanceof Error ? err.message : '取得失敗'))
      .finally(() => setLoading(false))
  }, [roomId])

  async function handleInvite(e: FormEvent) {
    e.preventDefault()
    const target = inviteInput.trim()
    if (!target || inviting) return

    const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)
    const token = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
    if (!homeserver || !token) return

    setInviting(true)
    setInviteError(null)
    setInviteSuccess(false)
    try {
      await inviteUser(homeserver, token, roomId, target)
      setInviteInput('')
      setInviteSuccess(true)
      setTimeout(() => setInviteSuccess(false), 3000)
    } catch (err) {
      setInviteError(err instanceof Error ? err.message : '招待失敗')
    } finally {
      setInviting(false)
    }
  }

  return (
    <>
      {/* バックドロップ */}
      <div className="fixed inset-0 z-40 bg-black/40" onClick={onClose} />

      {/* ドロワー本体（右から） */}
      <div
        className="fixed inset-y-0 right-0 z-50 flex w-72 max-w-full flex-col bg-gray-900 shadow-xl"
        style={{ paddingTop: 'env(safe-area-inset-top)', paddingBottom: 'env(safe-area-inset-bottom)' }}
      >
        <div className="flex items-center justify-between border-b border-gray-800 px-4 py-3">
          <h2 className="font-semibold">メンバー</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white text-xl leading-none">
            ×
          </button>
        </div>

        {/* 招待フォーム */}
        <form onSubmit={(e) => void handleInvite(e)} className="border-b border-gray-800 p-3">
          <p className="mb-1.5 text-xs text-gray-500">ユーザーを招待</p>
          <div className="flex gap-1.5">
            <input
              type="text"
              value={inviteInput}
              onChange={(e) => setInviteInput(e.target.value)}
              placeholder="@user:server"
              className="min-w-0 flex-1 rounded-lg bg-gray-800 px-3 py-1.5 text-sm text-white placeholder-gray-600 focus:outline-none focus:ring-1 focus:ring-indigo-500"
            />
            <button
              type="submit"
              disabled={!inviteInput.trim() || inviting}
              className="rounded-lg bg-indigo-600 px-3 py-1.5 text-sm text-white hover:bg-indigo-500 disabled:opacity-50"
            >
              {inviting ? '…' : '招待'}
            </button>
          </div>
          {inviteError && <p className="mt-1 text-xs text-red-400">{inviteError}</p>}
          {inviteSuccess && <p className="mt-1 text-xs text-green-400">招待しました</p>}
        </form>

        {loading && (
          <div className="flex flex-1 items-center justify-center">
            <div className="h-5 w-5 animate-spin rounded-full border-2 border-gray-500 border-t-transparent" />
          </div>
        )}

        {error && <p className="p-4 text-sm text-red-400">{error}</p>}

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
