/**
 * タイムライン — メッセージを吹き出しで表示し、最新に自動スクロール
 *
 * - 末尾イベントが変わったときだけ最下部へ自動スクロール（過去ログ挿入時は除外）
 * - 上端センチネルの IntersectionObserver で過去ログを遡り読み込み
 * - 過去ログ挿入後はスクロール位置を復元して表示位置が飛ばないようにする
 * - リアクション（m.reaction）を絵文字バッジとして吹き出し下に表示
 */
import { useEffect, useLayoutEffect, useRef } from 'react'
import type { MatrixEvent } from '../../api/sync'
import type { MemberNames, Reactions } from '../../stores/rooms'

interface Props {
  events: MatrixEvent[]
  myUserId: string | null
  reactions?: Reactions
  memberNames?: MemberNames
  hasMore?: boolean
  historyLoading?: boolean
  onLoadMore?: () => void
}

export default function Timeline({ events, myUserId, reactions, memberNames, hasMore, historyLoading, onLoadMore }: Props) {
  const containerRef = useRef<HTMLDivElement>(null)
  const bottomRef = useRef<HTMLDivElement>(null)
  const topSentinelRef = useRef<HTMLDivElement>(null)

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
          const body = String((ev.content as { body?: string }).body ?? '')
          const time = new Date(ev.origin_server_ts ?? 0).toLocaleTimeString('ja-JP', {
            hour: '2-digit',
            minute: '2-digit',
          })
          const eventReactions = ev.event_id ? (reactions?.[ev.event_id] ?? {}) : {}
          const reactionEntries = Object.entries(eventReactions)
          const senderName = (ev.sender && memberNames?.[ev.sender]) ?? ev.sender ?? ''

          return (
            <div key={ev.event_id} className={`flex ${isMine ? 'justify-end' : 'justify-start'}`}>
              <div className={`flex max-w-[75%] flex-col ${isMine ? 'items-end' : 'items-start'}`}>
                {!isMine && <span className="mb-0.5 text-xs text-gray-500">{senderName}</span>}
                <div
                  className={`break-words rounded-2xl px-3 py-2 text-sm ${
                    isMine
                      ? 'rounded-br-sm bg-indigo-600 text-white'
                      : 'rounded-bl-sm bg-gray-800 text-gray-100'
                  }`}
                >
                  {body}
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
