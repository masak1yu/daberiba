/**
 * アバターコンポーネント — mxc:// 画像またはイニシャルフォールバック
 * userId からハッシュで色を決定（Element 風）
 */
import { useState } from 'react'
import { mxcToThumbnail } from '../../api/media'
import { STORAGE_KEY } from '../../api/client'

interface Props {
  userId: string
  displayName?: string
  avatarUrl?: string
  size?: 'xs' | 'sm' | 'md' | 'lg'
  className?: string
}

const SIZE_PX: Record<string, number> = {
  xs: 20,
  sm: 28,
  md: 36,
  lg: 64,
}

// ユーザー ID から決定論的な色を生成（Element の色パレットに近い）
export function userColor(userId: string): string {
  let hash = 0
  for (let i = 0; i < userId.length; i++) {
    hash = userId.charCodeAt(i) + ((hash << 5) - hash)
  }
  const palette = [
    '#76cfa5',
    '#e95f55',
    '#9c64a6',
    '#4a90e2',
    '#f4a623',
    '#2dc2c5',
    '#e064f7',
    '#74d12c',
    '#c8a48c',
    '#ac3ba8',
  ]
  return palette[Math.abs(hash) % palette.length]!
}

function getInitial(userId: string, displayName?: string): string {
  const label = displayName ?? userId
  const ch = label.startsWith('@') ? label.charAt(1) : label.charAt(0)
  return ch.toUpperCase()
}

export default function Avatar({ userId, displayName, avatarUrl, size = 'md', className = '' }: Props) {
  const [imgError, setImgError] = useState(false)
  const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER) ?? ''
  const px = SIZE_PX[size] ?? 36
  const fontSize = px <= 20 ? 9 : px <= 28 ? 11 : px <= 36 ? 13 : 22

  const src = avatarUrl?.startsWith('mxc://')
    ? mxcToThumbnail(avatarUrl, homeserver, 96, 96)
    : avatarUrl

  const style = {
    width: px,
    height: px,
    minWidth: px,
    fontSize,
    borderRadius: '50%',
  }

  if (src && !imgError) {
    return (
      <img
        src={src}
        alt={displayName ?? userId}
        className={`object-cover select-none ${className}`}
        style={style}
        onError={() => setImgError(true)}
      />
    )
  }

  return (
    <div
      className={`flex items-center justify-center font-bold select-none ${className}`}
      style={{ ...style, background: userColor(userId), color: 'white' }}
    >
      {getInitial(userId, displayName)}
    </div>
  )
}
