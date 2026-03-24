/**
 * タイムライン — メッセージを吹き出しで表示し、最新に自動スクロール
 */
import { useEffect, useRef } from 'react'
import type { MatrixEvent } from '../../api/sync'

interface Props {
  events: MatrixEvent[]
  myUserId: string | null
}

export default function Timeline({ events, myUserId }: Props) {
  const bottomRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [events.length])

  if (events.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-gray-500">
        メッセージはありません
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-2 p-4">
      {events.map((ev) => {
        const isMine = ev.sender === myUserId
        const body = String((ev.content as { body?: string }).body ?? '')
        const time = new Date(ev.origin_server_ts ?? 0).toLocaleTimeString('ja-JP', {
          hour: '2-digit',
          minute: '2-digit',
        })

        return (
          <div key={ev.event_id} className={`flex ${isMine ? 'justify-end' : 'justify-start'}`}>
            <div className={`flex max-w-[75%] flex-col ${isMine ? 'items-end' : 'items-start'}`}>
              {!isMine && <span className="mb-0.5 text-xs text-gray-500">{ev.sender}</span>}
              <div
                className={`break-words rounded-2xl px-3 py-2 text-sm ${
                  isMine
                    ? 'rounded-br-sm bg-indigo-600 text-white'
                    : 'rounded-bl-sm bg-gray-800 text-gray-100'
                }`}
              >
                {body}
              </div>
              <span className="mt-0.5 text-xs text-gray-600">{time}</span>
            </div>
          </div>
        )
      })}
      <div ref={bottomRef} />
    </div>
  )
}
