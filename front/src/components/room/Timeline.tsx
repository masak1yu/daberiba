/**
 * タイムライン — Element 風の2カラムレイアウト
 *
 * 左カラム（52px）: グループ先頭はアバター、それ以外は時刻
 * 右カラム（flex）: グループ先頭は送信者名、その後はメッセージ本文
 */
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import type { MatrixEvent } from '../../api/sync'
import type { MemberAvatars, MemberNames, Reactions } from '../../stores/rooms'
import { mxcToHttp, mxcToThumbnail } from '../../api/media'
import { STORAGE_KEY } from '../../api/client'
import Avatar from '../common/Avatar'

const EMOJI_LIST = ['👍', '❤️', '😂', '😮', '😢', '🙏', '🎉', '🔥']
const GROUP_TIMEOUT_MS = 5 * 60 * 1000 // 5分以上経過で新グループ

/** Matrix user ID から表示名を取得（@localpart:server → localpart） */
function displayName(sender: string, memberNames?: MemberNames): string {
  if (memberNames?.[sender]) return memberNames[sender]!
  const m = sender.match(/^@?([^:]+)/)
  return m ? m[1] : sender
}

function DateSeparator({ ts }: { ts: number }) {
  const date = new Date(ts)
  const now = new Date()
  const yesterday = new Date(now)
  yesterday.setDate(now.getDate() - 1)

  let label: string
  if (date.toDateString() === now.toDateString()) {
    label = '今日'
  } else if (date.toDateString() === yesterday.toDateString()) {
    label = '昨日'
  } else {
    label = date.toLocaleDateString('ja-JP', { year: 'numeric', month: 'long', day: 'numeric' })
  }

  return (
    <div className="flex items-center gap-3 px-4 py-3">
      <div className="flex-1" style={{ height: '1px', background: '#2d3440' }} />
      <span
        className="shrink-0 rounded-full px-3 py-0.5 text-xs"
        style={{ background: '#21262d', color: '#636e7d', border: '1px solid #2d3440' }}
      >
        {label}
      </span>
      <div className="flex-1" style={{ height: '1px', background: '#2d3440' }} />
    </div>
  )
}

interface Props {
  events: MatrixEvent[]
  myUserId: string | null
  reactions?: Reactions
  memberNames?: MemberNames
  memberAvatars?: MemberAvatars
  hasMore?: boolean
  historyLoading?: boolean
  onLoadMore?: () => void
  onReact?: (eventId: string, emoji: string) => void
  onDelete?: (eventId: string) => void
  onEdit?: (eventId: string, currentBody: string) => void
}

function MessageContent({ content }: { content: Record<string, unknown> }) {
  const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER) ?? ''
  const msgtype = String(content.msgtype ?? '')
  const body = String(content.body ?? '')

  if (msgtype === 'm.image') {
    const mxc = String(content.url ?? '')
    const src = mxc.startsWith('mxc://') ? mxcToThumbnail(mxc, homeserver) : mxc
    return (
      <a href={mxcToHttp(mxc, homeserver)} target="_blank" rel="noopener noreferrer">
        <img
          src={src}
          alt={body}
          className="max-h-72 max-w-sm rounded-lg object-cover"
          loading="lazy"
        />
      </a>
    )
  }

  if (msgtype === 'm.file' || msgtype === 'm.audio' || msgtype === 'm.video') {
    const mxc = String(content.url ?? '')
    const href = mxc.startsWith('mxc://') ? mxcToHttp(mxc, homeserver) : mxc
    const icon = msgtype === 'm.audio' ? '🎵' : msgtype === 'm.video' ? '🎬' : '📎'
    return (
      <a
        href={href}
        target="_blank"
        rel="noopener noreferrer"
        className="inline-flex items-center gap-1.5 underline underline-offset-2"
        style={{ color: '#0dbd8b' }}
      >
        {icon} <span className="break-all">{body}</span>
      </a>
    )
  }

  return <span className="whitespace-pre-wrap break-words">{body}</span>
}

export default function Timeline({
  events,
  myUserId,
  reactions,
  memberNames,
  memberAvatars,
  hasMore,
  historyLoading,
  onLoadMore,
  onReact,
  onDelete,
  onEdit,
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null)
  const bottomRef = useRef<HTMLDivElement>(null)
  const topSentinelRef = useRef<HTMLDivElement>(null)
  const savedScrollRef = useRef<{ height: number; top: number } | null>(null)

  const [activeEventId, setActiveEventId] = useState<string | null>(null)
  const [editingEventId, setEditingEventId] = useState<string | null>(null)
  const [editInput, setEditInput] = useState('')
  const [showScrollBtn, setShowScrollBtn] = useState(false)

  // 新着で最下部へ自動スクロール
  const lastEventId = events.at(-1)?.event_id
  const prevLastEventIdRef = useRef<string | undefined>(lastEventId)
  useEffect(() => {
    if (lastEventId !== prevLastEventIdRef.current) {
      prevLastEventIdRef.current = lastEventId
      if (!savedScrollRef.current) {
        bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
      }
    }
  }, [lastEventId])

  // 過去ログ挿入後スクロール位置復元
  const firstEventId = events[0]?.event_id
  useLayoutEffect(() => {
    const el = containerRef.current
    const saved = savedScrollRef.current
    if (el && saved) {
      el.scrollTop = saved.top + (el.scrollHeight - saved.height)
      savedScrollRef.current = null
    }
  }, [firstEventId])

  // 上端センチネル
  useEffect(() => {
    const sentinel = topSentinelRef.current
    if (!sentinel || !onLoadMore) return
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting && hasMore && !historyLoading) {
          const el = containerRef.current
          if (el) savedScrollRef.current = { height: el.scrollHeight, top: el.scrollTop }
          onLoadMore()
        }
      },
      { root: containerRef.current, threshold: 0 }
    )
    observer.observe(sentinel)
    return () => observer.disconnect()
  }, [onLoadMore, hasMore, historyLoading])

  // スクロール↓ボタン制御
  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const onScroll = () => {
      setShowScrollBtn(el.scrollHeight - el.scrollTop - el.clientHeight > 200)
    }
    el.addEventListener('scroll', onScroll, { passive: true })
    return () => el.removeEventListener('scroll', onScroll)
  }, [])

  // メニュー外クリックで閉じる
  useEffect(() => {
    if (!activeEventId) return
    const handler = () => setActiveEventId(null)
    document.addEventListener('click', handler, { capture: true })
    return () => document.removeEventListener('click', handler, { capture: true })
  }, [activeEventId])

  if (events.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm" style={{ color: '#636e7d' }}>
        まだメッセージはありません
      </div>
    )
  }

  return (
    <div className="relative h-full">
      <div ref={containerRef} className="h-full overflow-y-auto py-4">
        <div ref={topSentinelRef} />

        {historyLoading && (
          <div className="flex justify-center py-3">
            <div
              className="h-4 w-4 animate-spin rounded-full border-2"
              style={{ borderColor: '#2d3440', borderTopColor: '#0dbd8b' }}
            />
          </div>
        )}

        {events.map((ev, idx) => {
          const prevEv = events[idx - 1]
          const currTs = ev.origin_server_ts ?? 0
          const prevTs = prevEv?.origin_server_ts ?? 0
          const isGroupStart = prevEv?.sender !== ev.sender || currTs - prevTs > GROUP_TIMEOUT_MS
          const currDate = new Date(currTs)
          const prevDate = prevEv ? new Date(prevTs) : null
          const showDateSep = !prevDate || prevDate.toDateString() !== currDate.toDateString()
          const time = new Date(currTs).toLocaleTimeString('ja-JP', {
            hour: '2-digit',
            minute: '2-digit',
          })
          const eventReactions = ev.event_id ? (reactions?.[ev.event_id] ?? {}) : {}
          const reactionEntries = Object.entries(eventReactions)
          const senderName = displayName(ev.sender ?? '', memberNames)
          const senderAvatar = ev.sender ? memberAvatars?.[ev.sender] : undefined
          const isActive = activeEventId === ev.event_id
          const isEditing = editingEventId === ev.event_id
          const isMine = ev.sender === myUserId
          const body = String((ev.content as { body?: string }).body ?? '')

          return (
            <div key={ev.event_id}>
              {showDateSep && <DateSeparator ts={ev.origin_server_ts ?? 0} />}
              <div
                className="group relative"
                style={{ marginTop: !showDateSep && isGroupStart ? '12px' : '0' }}
              >
                <div
                  className="flex px-4 py-0.5 transition-colors"
                  style={{ background: isActive ? '#2d3440' : 'transparent' }}
                  onMouseEnter={(e) => {
                    if (!isActive) e.currentTarget.style.background = '#1e242c'
                  }}
                  onMouseLeave={(e) => {
                    if (!isActive) e.currentTarget.style.background = 'transparent'
                  }}
                >
                  {/* 左カラム: アバター or 時刻 */}
                  <div className="w-10 shrink-0 mr-3 flex items-start justify-center pt-0.5">
                    {isGroupStart ? (
                      <Avatar
                        userId={ev.sender ?? ''}
                        displayName={senderName}
                        avatarUrl={senderAvatar}
                        size="sm"
                      />
                    ) : (
                      <span
                        className="text-[10px] leading-5 opacity-0 group-hover:opacity-100 transition-opacity select-none"
                        style={{ color: '#636e7d' }}
                      >
                        {time}
                      </span>
                    )}
                  </div>

                  {/* 右カラム */}
                  <div className="min-w-0 flex-1">
                    {/* グループ先頭: ユーザー名 + 時刻 */}
                    {isGroupStart && (
                      <div className="flex items-baseline gap-2 mb-0.5">
                        <span
                          className="text-sm font-bold leading-tight"
                          style={{ color: '#e9edf1' }}
                        >
                          {senderName}
                        </span>
                        <span className="text-[11px]" style={{ color: '#636e7d' }}>
                          {time}
                        </span>
                      </div>
                    )}

                    {/* メッセージ本文 */}
                    {isEditing ? (
                      <div className="flex gap-2 py-1">
                        <input
                          value={editInput}
                          onChange={(e) => setEditInput(e.target.value)}
                          autoFocus
                          onKeyDown={(e) => {
                            if (e.key === 'Enter' && !e.shiftKey) {
                              e.preventDefault()
                              if (editInput.trim() && onEdit) onEdit(ev.event_id!, editInput.trim())
                              setEditingEventId(null)
                            }
                            if (e.key === 'Escape') setEditingEventId(null)
                          }}
                          className="min-w-0 flex-1 rounded-lg px-3 py-1.5 text-sm focus:outline-none"
                          style={{
                            background: '#2d3440',
                            color: '#e9edf1',
                            border: '1px solid #0dbd8b',
                          }}
                        />
                        <button
                          onClick={() => {
                            if (editInput.trim() && onEdit) onEdit(ev.event_id!, editInput.trim())
                            setEditingEventId(null)
                          }}
                          disabled={!editInput.trim()}
                          className="rounded-lg px-3 py-1.5 text-xs font-medium disabled:opacity-50"
                          style={{ background: '#0dbd8b', color: 'white' }}
                        >
                          保存
                        </button>
                        <button
                          onClick={() => setEditingEventId(null)}
                          className="rounded-lg px-3 py-1.5 text-xs"
                          style={{ background: '#2d3440', color: '#8d99a6' }}
                        >
                          取消
                        </button>
                      </div>
                    ) : (
                      <div className="text-sm leading-relaxed" style={{ color: '#d1d5db' }}>
                        <MessageContent content={ev.content} />
                      </div>
                    )}

                    {/* リアクションバッジ */}
                    {reactionEntries.length > 0 && (
                      <div className="mt-1 flex flex-wrap gap-1">
                        {reactionEntries.map(([emoji, senders]) => {
                          const reacted = senders.includes(myUserId ?? '')
                          return (
                            <button
                              key={emoji}
                              onClick={() => ev.event_id && onReact?.(ev.event_id, emoji)}
                              className="flex items-center gap-0.5 rounded-full px-2 py-0.5 text-xs transition-colors"
                              style={{
                                background: reacted ? 'rgba(13,189,139,0.15)' : '#2d3440',
                                border: `1px solid ${reacted ? '#0dbd8b' : '#363c48'}`,
                                color: reacted ? '#0dbd8b' : '#8d99a6',
                              }}
                            >
                              {emoji}
                              <span className="font-medium">{senders.length}</span>
                            </button>
                          )
                        })}
                      </div>
                    )}
                  </div>

                  {/* ホバーアクションメニュー（右端に浮かぶ） */}
                  <div
                    className="absolute right-4 -top-3 z-10 hidden rounded-lg p-0.5 group-hover:flex"
                    style={{
                      background: '#21262d',
                      border: '1px solid #2d3440',
                      boxShadow: '0 2px 8px rgba(0,0,0,0.5)',
                    }}
                    onClick={(e) => e.stopPropagation()}
                  >
                    {/* リアクション */}
                    {onReact && ev.event_id && (
                      <div className="relative">
                        <button
                          onClick={() => setActiveEventId(isActive ? null : (ev.event_id ?? null))}
                          className="rounded p-1.5 transition-colors hover:bg-white/10"
                          style={{ color: '#8d99a6' }}
                          title="リアクション"
                        >
                          <svg
                            className="h-3.5 w-3.5"
                            fill="none"
                            viewBox="0 0 24 24"
                            stroke="currentColor"
                          >
                            <path
                              strokeLinecap="round"
                              strokeLinejoin="round"
                              strokeWidth={2}
                              d="M14.828 14.828a4 4 0 01-5.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                            />
                          </svg>
                        </button>
                        {isActive && (
                          <div
                            className="absolute bottom-full right-0 mb-1 flex gap-0.5 rounded-xl p-2"
                            style={{
                              background: '#21262d',
                              border: '1px solid #2d3440',
                              boxShadow: '0 4px 16px rgba(0,0,0,0.6)',
                            }}
                            onClick={(e) => e.stopPropagation()}
                          >
                            {EMOJI_LIST.map((emoji) => (
                              <button
                                key={emoji}
                                onClick={() => {
                                  onReact(ev.event_id!, emoji)
                                  setActiveEventId(null)
                                }}
                                className="rounded-lg p-1.5 text-lg transition-colors hover:bg-white/10"
                              >
                                {emoji}
                              </button>
                            ))}
                          </div>
                        )}
                      </div>
                    )}

                    {/* 編集（自分のみ） */}
                    {isMine && onEdit && ev.event_id && (
                      <button
                        onClick={() => {
                          setEditingEventId(ev.event_id!)
                          setEditInput(body)
                        }}
                        className="rounded p-1.5 transition-colors hover:bg-white/10"
                        style={{ color: '#8d99a6' }}
                        title="編集"
                      >
                        <svg
                          className="h-3.5 w-3.5"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke="currentColor"
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            strokeWidth={2}
                            d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"
                          />
                        </svg>
                      </button>
                    )}

                    {/* 削除（自分のみ） */}
                    {isMine && onDelete && ev.event_id && (
                      <button
                        onClick={() => onDelete(ev.event_id!)}
                        className="rounded p-1.5 transition-colors hover:bg-white/10"
                        style={{ color: '#e53935' }}
                        title="削除"
                      >
                        <svg
                          className="h-3.5 w-3.5"
                          fill="none"
                          viewBox="0 0 24 24"
                          stroke="currentColor"
                        >
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            strokeWidth={2}
                            d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                          />
                        </svg>
                      </button>
                    )}
                  </div>
                </div>
              </div>
            </div>
          )
        })}

        <div ref={bottomRef} />
      </div>

      {showScrollBtn && (
        <button
          onClick={() => bottomRef.current?.scrollIntoView({ behavior: 'smooth' })}
          className="absolute bottom-4 right-4 z-10 flex h-8 w-8 items-center justify-center rounded-full shadow-lg"
          style={{ background: '#0dbd8b', color: 'white' }}
          title="最新メッセージへ"
        >
          ↓
        </button>
      )}
    </div>
  )
}
