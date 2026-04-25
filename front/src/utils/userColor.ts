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
