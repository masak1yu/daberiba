/**
 * タイムライン — メッセージを吹き出しで表示し、最新に自動スクロール
 *
 * - 末尾イベントが変わったときだけ最下部へ自動スクロール（過去ログ挿入時は除外）
 * - 上端センチネルの IntersectionObserver で過去ログを遡り読み込み
 * - 過去ログ挿入後はスクロール位置を復元して表示位置が飛ばないようにする
 * - リアクション（m.reaction）を絵文字バッジとして吹き出し下に表示
 * - バブルタップで絵文字ピッカー / 編集・削除メニューを表示
 * - スクロール中は ↓ ボタンを表示して最下部に戻れる
 * - m.image はサムネイル表示、m.file はダウンロードリンク表示
 */
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import type { MatrixEvent } from '../../api/sync'
import type { MemberAvatars, MemberNames, Reactions } from '../../stores/rooms'
import { mxcToHttp, mxcToThumbnail } from '../../api/media'
import { STORAGE_KEY } from '../../api/client'
import Avatar from '../common/Avatar'

const EMOJI_LIST = ['👍', '❤️', '😂', '😮', '😢', '🙏', '🎉', '🔥']

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

/** msgtype ごとのバブル内コンテンツ */
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
          className="max-h-60 max-w-full rounded-lg object-cover"
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
        className="flex items-center gap-1.5 underline underline-offset-2"
      >
        <span>{icon}</span>
        <span className="break-all">{body}</span>
      </a>
    )
  }

  return <span className="whitespace-pre-wrap">{body}</span>
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

  // アクティブなバブルのイベント ID（ピッカー / メニュー表示用）
  const [activeEventId, setActiveEventId] = useState<string | null>(null)
  // インライン編集中のイベント ID と入力値
  const [editingEventId, setEditingEventId] = useState<string | null>(null)
  const [editInput, setEditInput] = useState('')
  // スクロール位置が最下部から離れているか
  const [showScrollBtn, setShowScrollBtn] = useState(false)

  // スクロール位置復元用
  const savedScrollRef = useRef<{ height: number; top: number } | null>(null)

  // 末尾のイベントが変わったときだけ最下部へ自動スクロール
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

  // 過去ログ挿入後にスクロール位置を復元
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

  // スクロール位置を監視して ↓ ボタン表示を制御
  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const onScroll = () => {
      const distFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight
      setShowScrollBtn(distFromBottom > 200)
    }
    el.addEventListener('scroll', onScroll, { passive: true })
    return () => el.removeEventListener('scroll', onScroll)
  }, [])

  // ピッカー外タップで閉じる
  useEffect(() => {
    if (!activeEventId) return
    const handler = () => setActiveEventId(null)
    document.addEventListener('click', handler, { capture: true })
    return () => document.removeEventListener('click', handler, { capture: true })
  }, [activeEventId])

  function scrollToBottom() {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }

  function startEdit(eventId: string, body: string) {
    setEditingEventId(eventId)
    setEditInput(body)
    setActiveEventId(null)
  }

  function submitEdit(eventId: string) {
    if (editInput.trim() && onEdit) {
      onEdit(eventId, editInput.trim())
    }
    setEditingEventId(null)
    setEditInput('')
  }

  if (events.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-gray-500">
        メッセージはありません
      </div>
    )
  }

  return (
    <div className="relative h-full">
      <div ref={containerRef} className="flex h-full flex-col overflow-y-auto">
        <div ref={topSentinelRef} />

        {historyLoading && (
          <div className="flex justify-center py-2">
            <div className="h-4 w-4 animate-spin rounded-full border-2 border-gray-500 border-t-transparent" />
          </div>
        )}

        <div className="flex flex-col gap-2 p-4">
          {events.map((ev) => {
            const isMine = ev.sender === myUserId
            const time = new Date(ev.origin_server_ts ?? 0).toLocaleTimeString('ja-JP', {
              hour: '2-digit',
              minute: '2-digit',
            })
            const eventReactions = ev.event_id ? (reactions?.[ev.event_id] ?? {}) : {}
            const reactionEntries = Object.entries(eventReactions)
            const senderName = (ev.sender && memberNames?.[ev.sender]) ?? ev.sender ?? ''
            const senderAvatar = ev.sender ? memberAvatars?.[ev.sender] : undefined
            const isActive = activeEventId === ev.event_id
            const isEditing = editingEventId === ev.event_id
            const body = String((ev.content as { body?: string }).body ?? '')

            return (
              <div
                key={ev.event_id}
                className={`flex items-end gap-2 ${isMine ? 'justify-end' : 'justify-start'}`}
              >
                {!isMine && (
                  <Avatar
                    userId={ev.sender ?? ''}
                    displayName={senderName}
                    avatarUrl={senderAvatar}
                    size="sm"
                  />
                )}

                <div
                  className={`flex min-w-0 max-w-[75%] flex-col ${isMine ? 'items-end' : 'items-start'}`}
                >
                  {!isMine && <span className="mb-0.5 text-xs text-gray-500">{senderName}</span>}

                  <div className="relative">
                    {/* アクションメニュー（ピッカー + 編集/削除） */}
                    {isActive && (
                      <div
                        className={`absolute bottom-full mb-1 z-20 flex flex-col gap-1 rounded-xl bg-gray-800 p-1.5 shadow-lg min-w-max ${isMine ? 'right-0' : 'left-0'}`}
                        onClick={(e) => e.stopPropagation()}
                      >
                        {/* 絵文字ピッカー */}
                        {onReact && ev.event_id && (
                          <div className="flex gap-0.5">
                            {EMOJI_LIST.map((emoji) => (
                              <button
                                key={emoji}
                                onClick={() => {
                                  onReact(ev.event_id!, emoji)
                                  setActiveEventId(null)
                                }}
                                className="rounded-lg p-1 text-base hover:bg-gray-700"
                              >
                                {emoji}
                              </button>
                            ))}
                          </div>
                        )}
                        {/* 編集・削除（自分のメッセージのみ） */}
                        {isMine && ev.event_id && (
                          <div className="flex gap-1 border-t border-gray-700 pt-1">
                            {onEdit && (
                              <button
                                onClick={() => startEdit(ev.event_id!, body)}
                                className="flex-1 rounded-lg px-2 py-1 text-xs text-gray-300 hover:bg-gray-700"
                              >
                                編集
                              </button>
                            )}
                            {onDelete && (
                              <button
                                onClick={() => {
                                  onDelete(ev.event_id!)
                                  setActiveEventId(null)
                                }}
                                className="flex-1 rounded-lg px-2 py-1 text-xs text-red-400 hover:bg-gray-700"
                              >
                                削除
                              </button>
                            )}
                          </div>
                        )}
                      </div>
                    )}

                    {/* バブル */}
                    {isEditing ? (
                      <div className="flex gap-1.5">
                        <input
                          value={editInput}
                          onChange={(e) => setEditInput(e.target.value)}
                          autoFocus
                          onKeyDown={(e) => {
                            if (e.key === 'Enter' && !e.shiftKey) {
                              e.preventDefault()
                              submitEdit(ev.event_id!)
                            }
                            if (e.key === 'Escape') {
                              setEditingEventId(null)
                            }
                          }}
                          className="min-w-0 flex-1 rounded-lg bg-gray-700 px-3 py-2 text-sm text-white focus:outline-none focus:ring-1 focus:ring-indigo-500"
                        />
                        <button
                          onClick={() => submitEdit(ev.event_id!)}
                          disabled={!editInput.trim()}
                          className="rounded-lg bg-indigo-600 px-2 py-1 text-xs text-white hover:bg-indigo-500 disabled:opacity-50"
                        >
                          保存
                        </button>
                        <button
                          onClick={() => setEditingEventId(null)}
                          className="rounded-lg bg-gray-700 px-2 py-1 text-xs text-gray-300 hover:bg-gray-600"
                        >
                          取消
                        </button>
                      </div>
                    ) : (
                      <div
                        role="button"
                        tabIndex={0}
                        onClick={(e) => {
                          e.stopPropagation()
                          setActiveEventId(isActive ? null : (ev.event_id ?? null))
                        }}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') {
                            setActiveEventId(isActive ? null : (ev.event_id ?? null))
                          }
                        }}
                        className={`break-words rounded-2xl px-3 py-2 text-sm cursor-default select-text ${
                          isMine
                            ? 'rounded-br-sm bg-indigo-600 text-white'
                            : 'rounded-bl-sm bg-gray-800 text-gray-100'
                        }`}
                      >
                        <MessageContent content={ev.content} />
                      </div>
                    )}
                  </div>

                  {/* リアクションバッジ */}
                  {reactionEntries.length > 0 && (
                    <div className="mt-1 flex flex-wrap gap-1">
                      {reactionEntries.map(([emoji, senders]) => {
                        const reacted = senders.includes(myUserId ?? '')
                        return (
                          <span
                            key={emoji}
                            className={`flex items-center gap-0.5 rounded-full border px-2 py-0.5 text-xs ${
                              reacted
                                ? 'border-indigo-500 bg-indigo-900/60 text-indigo-200'
                                : 'border-gray-700 bg-gray-800 text-gray-300'
                            }`}
                          >
                            {emoji}
                            <span className="font-medium">{senders.length}</span>
                          </span>
                        )
                      })}
                    </div>
                  )}

                  <span className="mt-0.5 text-xs text-gray-600">{time}</span>
                </div>
              </div>
            )
          })}
        </div>

        <div ref={bottomRef} />
      </div>

      {/* スクロールtoBottomボタン */}
      {showScrollBtn && (
        <button
          onClick={scrollToBottom}
          className="absolute bottom-4 right-4 z-10 flex h-9 w-9 items-center justify-center rounded-full bg-indigo-600 text-white shadow-lg hover:bg-indigo-500"
          title="最新メッセージへ"
        >
          ↓
        </button>
      )}
    </div>
  )
}
