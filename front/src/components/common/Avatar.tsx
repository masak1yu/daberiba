/**
 * アバターコンポーネント — mxc:// 画像またはイニシャルフォールバック
 */
import { useState } from 'react'
import { mxcToThumbnail } from '../../api/media'
import { STORAGE_KEY } from '../../api/client'

interface Props {
  userId: string
  displayName?: string
  avatarUrl?: string
  size?: 'sm' | 'md' | 'lg'
}

const SIZE_CLASS = {
  sm: 'h-7 w-7 text-xs',
  md: 'h-9 w-9 text-sm',
  lg: 'h-16 w-16 text-xl',
}

function getInitial(userId: string, displayName?: string): string {
  const label = displayName ?? userId
  const ch = label.startsWith('@') ? label.charAt(1) : label.charAt(0)
  return ch.toUpperCase()
}

export default function Avatar({ userId, displayName, avatarUrl, size = 'md' }: Props) {
  const [imgError, setImgError] = useState(false)
  const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER) ?? ''
  const cls = `${SIZE_CLASS[size]} shrink-0 rounded-full`

  const src = avatarUrl?.startsWith('mxc://')
    ? mxcToThumbnail(avatarUrl, homeserver, 96, 96)
    : avatarUrl

  if (src && !imgError) {
    return (
      <img
        src={src}
        alt={displayName ?? userId}
        className={`${cls} object-cover`}
        onError={() => setImgError(true)}
      />
    )
  }

  return (
    <div className={`${cls} flex items-center justify-center bg-indigo-700 font-bold select-none`}>
      {getInitial(userId, displayName)}
    </div>
  )
}
