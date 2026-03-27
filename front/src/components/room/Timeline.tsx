/**
 * タイムライン — メッセージを吹き出しで表示し、最新に自動スクロール
 *
 * - 末尾イベントが変わったときだけ最下部へ自動スクロール（過去ログ挿入時は除外）
 * - 上端センチネルの IntersectionObserver で過去ログを遡り読み込み
 * - 過去ログ挿入後はスクロール位置を復元して表示位置が飛ばないようにする
 * - リアクション（m.reaction）を絵文字バッジとして吹き出し下に表示
 * - バブルタップで絵文字ピッカーを表示してリアクションを送信できる
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

  // m.text / m.notice / その他 — 改行保持
  return <span className="whitespace-pre-wrap">{body}</span>
}

/** 絵文字ピッカー */
function EmojiPicker({
  onSelect,
  onClose,
}: {
  onSelect: (emoji: string) => void
  onClose: () => void
}) {
  return (
    <div className="absolute bottom-full mb-1 z-20 flex gap-1 rounded-xl bg-gray-800 p-1.5 shadow-lg">
      {EMOJI_LIST.map((emoji) => (
        <button
          key={emoji}
          onClick={() => {
            onSelect(emoji)
            onClose()
          }}
          className="rounded-lg p-1 text-lg hover:bg-gray-700 active:scale-90"
        >
          {emoji}
        </button>
      ))}
    </div>
  )
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
}: Props) {
  const containerRef = useRef<HTMLDivElement>(null)
  const bottomRef = useRef<HTMLDivElement>(null)
  const topSentinelRef = useRef<HTMLDivElement>(null)
  const [pickerEventId, setPickerEventId] = useState<string | null>(null)

  // スクロール位置復元用: onLoadMore 呼び出し前に保存しておく
  const savedScrollRef = useRef<{ height: number; top: number } | null>(null)

  // 末尾のイベントが変わったときだけ最下部へ自動スクロール
  const lastEventId = events.at(-1)?.event_id
  const prevLastEventIdRef = useRef<string | undefined>(lastEventId)
  useEffect(() => {
    if (lastEventId !== prevLastEventIdRef.current) {
      prevLastEventIdRef.current = lastEventId
      // 過去ログ読み込み後（savedScroll がある場合）は自動スクロールしない
      if (!savedScrollRef.current) {
        bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
      }
    }
  }, [lastEventId])

  // 過去ログ挿入後にスクロール位置を復元（DOM 更新直後に実行）
  const firstEventId = events[0]?.event_id
  useLayoutEffect(() => {
    const el = containerRef.current
    const saved = savedScrollRef.current
    if (el && saved) {
      el.scrollTop = saved.top + (el.scrollHeight - saved.height)
      savedScrollRef.current = null
    }
  }, [firstEventId])

  // 上端センチネルが見えたら過去ログを読み込む
  useEffect(() => {
    const sentinel = topSentinelRef.current
    if (!sentinel || !onLoadMore) return

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting && hasMore && !historyLoading) {
          // スクロール位置を保存してから読み込み開始
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

  // ピッカー外タップで閉じる
  useEffect(() => {
    if (!pickerEventId) return
    const handler = () => setPickerEventId(null)
    document.addEventListener('click', handler, { capture: true })
    return () => document.removeEventListener('click', handler, { capture: true })
  }, [pickerEventId])

  if (events.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-gray-500">
        メッセージはありません
      </div>
    )
  }

  return (
    <div ref={containerRef} className="flex h-full flex-col overflow-y-auto">
      {/* 上端センチネル: IntersectionObserver のターゲット */}
      <div ref={topSentinelRef} />

      {/* 過去ログ読み込み中インジケーター */}
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
          const showPicker = pickerEventId === ev.event_id

          return (
            <div
              key={ev.event_id}
              className={`flex items-end gap-2 ${isMine ? 'justify-end' : 'justify-start'}`}
            >
              {/* 他ユーザーのアバター（左端に配置） */}
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

                {/* バブル + ピッカー */}
                <div className="relative">
                  {showPicker && onReact && ev.event_id && (
                    <EmojiPicker
                      onSelect={(emoji) => onReact(ev.event_id!, emoji)}
                      onClose={() => setPickerEventId(null)}
                    />
                  )}
                  <div
                    role="button"
                    tabIndex={0}
                    onClick={(e) => {
                      e.stopPropagation()
                      if (onReact && ev.event_id) {
                        setPickerEventId(showPicker ? null : ev.event_id)
                      }
                    }}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' && onReact && ev.event_id) {
                        setPickerEventId(showPicker ? null : ev.event_id)
                      }
                    }}
                    className={`break-words rounded-2xl px-3 py-2 text-sm cursor-default ${
                      isMine
                        ? 'rounded-br-sm bg-indigo-600 text-white'
                        : 'rounded-bl-sm bg-gray-800 text-gray-100'
                    }`}
                  >
                    <MessageContent content={ev.content} />
                  </div>
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
  )
}
