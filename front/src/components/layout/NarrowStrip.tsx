import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuthStore } from '../../stores/auth'
import Avatar from '../common/Avatar'

export default function NarrowStrip() {
  const navigate = useNavigate()
  const userId = useAuthStore((s) => s.userId)
  const logout = useAuthStore((s) => s.logout)
  const [menuOpen, setMenuOpen] = useState(false)
  const menuRef = useRef<HTMLDivElement>(null)

  // メニュー外クリックで閉じる
  useEffect(() => {
    if (!menuOpen) return
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false)
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [menuOpen])

  return (
    <div
      className="relative flex w-[50px] shrink-0 flex-col items-center py-2"
      style={{ background: '#21262d', borderRight: '1px solid #2d3440' }}
    >
      {/* ユーザーアバター（クリックでメニュー） */}
      <button
        onClick={() => setMenuOpen((v) => !v)}
        className="mb-2 rounded-full transition-opacity hover:opacity-80"
        title={userId ?? ''}
      >
        <Avatar userId={userId ?? ''} displayName={userId ?? ''} size="sm" />
      </button>

      {/* ホームアイコン */}
      <button
        onClick={() => navigate('/')}
        className="flex h-9 w-9 items-center justify-center rounded-lg transition-colors hover:bg-white/10"
        style={{ color: '#8d99a6' }}
        title="ホーム"
      >
        <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.8}
            d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"
          />
        </svg>
      </button>

      {/* スペーサー */}
      <div className="flex-1" />

      {/* チャットアイコン */}
      <button
        className="mb-1 flex h-9 w-9 items-center justify-center rounded-lg transition-colors hover:bg-white/10"
        style={{ color: '#8d99a6' }}
        title="チャット"
      >
        <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.8}
            d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"
          />
        </svg>
      </button>

      {/* 設定アイコン */}
      <button
        onClick={() => navigate('/settings')}
        className="flex h-9 w-9 items-center justify-center rounded-lg transition-colors hover:bg-white/10"
        style={{ color: '#8d99a6' }}
        title="設定"
      >
        <svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.8}
            d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
          />
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={1.8}
            d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
          />
        </svg>
      </button>

      {/* ユーザーメニュー */}
      {menuOpen && (
        <div
          ref={menuRef}
          className="absolute left-[54px] top-0 z-50 w-64 rounded-xl py-1 shadow-2xl"
          style={{ background: '#21262d', border: '1px solid #2d3440' }}
        >
          {/* ユーザー情報 */}
          <div className="flex items-center gap-3 px-4 py-3">
            <Avatar userId={userId ?? ''} displayName={userId ?? ''} size="sm" />
            <div className="min-w-0 flex-1">
              <p className="truncate text-sm font-semibold" style={{ color: '#e9edf1' }}>
                {userId?.split(':')[0]?.replace('@', '') ?? ''}
              </p>
              <p className="truncate text-xs" style={{ color: '#8d99a6' }}>
                {userId}
              </p>
            </div>
          </div>

          <div className="my-1" style={{ borderTop: '1px solid #2d3440' }} />

          <button
            onClick={() => {
              setMenuOpen(false)
              navigate('/settings')
            }}
            className="flex w-full items-center gap-3 px-4 py-2.5 text-sm transition-colors hover:bg-white/5"
            style={{ color: '#e9edf1' }}
          >
            <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
              />
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
              />
            </svg>
            全ての設定
          </button>

          <div className="my-1" style={{ borderTop: '1px solid #2d3440' }} />

          <button
            onClick={() => {
              setMenuOpen(false)
              void logout()
            }}
            className="flex w-full items-center gap-3 px-4 py-2.5 text-sm transition-colors hover:bg-white/5"
            style={{ color: '#e53935' }}
          >
            <svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"
              />
            </svg>
            サインアウト
          </button>
        </div>
      )}
    </div>
  )
}
