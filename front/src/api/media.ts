/**
 * Matrix メディア URL ヘルパー
 *
 * mxc://server/mediaId を HTTP URL に変換する。
 * Matrix v1.11 以降は /_matrix/client/v1/media/download が推奨だが、
 * 後方互換のため /_matrix/media/v3/download を使用する。
 */

/**
 * mxc:// URI を HTTP ダウンロード URL に変換する
 * @param mxc      "mxc://server/mediaId" 形式の URI
 * @param homeserver バックエンドのホームサーバー URL
 */
export function mxcToHttp(mxc: string, homeserver: string): string {
  // mxc://serverName/mediaId
  const match = /^mxc:\/\/([^/]+)\/(.+)$/.exec(mxc)
  if (!match) return mxc
  const [, serverName, mediaId] = match
  return `${homeserver}/_matrix/media/v3/download/${serverName}/${mediaId}`
}

/**
 * mxc:// URI をサムネイル URL に変換する
 * @param mxc       mxc URI
 * @param homeserver ホームサーバー URL
 * @param width     最大幅 px（デフォルト 800）
 * @param height    最大高さ px（デフォルト 600）
 */
export function mxcToThumbnail(mxc: string, homeserver: string, width = 800, height = 600): string {
  const match = /^mxc:\/\/([^/]+)\/(.+)$/.exec(mxc)
  if (!match) return mxc
  const [, serverName, mediaId] = match
  const params = new URLSearchParams({
    width: String(width),
    height: String(height),
    method: 'scale',
  })
  return `${homeserver}/_matrix/media/v3/thumbnail/${serverName}/${mediaId}?${params}`
}
