/**
 * プロフィール API — avatar_url の取得・更新 + メディアアップロード
 */

/** GET /_matrix/client/v3/profile/{userId}/avatar_url */
export async function fetchAvatarUrl(
  homeserver: string,
  token: string,
  userId: string
): Promise<string | null> {
  const res = await fetch(
    `${homeserver}/_matrix/client/v3/profile/${encodeURIComponent(userId)}/avatar_url`,
    { headers: { Authorization: `Bearer ${token}` } }
  )
  if (!res.ok) return null
  const data = (await res.json()) as { avatar_url?: string }
  return data.avatar_url ?? null
}

/** PUT /_matrix/client/v3/profile/{userId}/avatar_url */
export async function putAvatarUrl(
  homeserver: string,
  token: string,
  userId: string,
  avatarUrl: string
): Promise<void> {
  const res = await fetch(
    `${homeserver}/_matrix/client/v3/profile/${encodeURIComponent(userId)}/avatar_url`,
    {
      method: 'PUT',
      headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
      body: JSON.stringify({ avatar_url: avatarUrl }),
    }
  )
  if (!res.ok) {
    const err = (await res.json().catch(() => ({}))) as { error?: string }
    throw new Error(err.error ?? `PUT avatar_url failed: ${res.status}`)
  }
}

/**
 * POST /_matrix/media/v3/upload — ファイルをアップロードして mxc:// URI を返す
 */
export async function uploadMedia(homeserver: string, token: string, file: File): Promise<string> {
  const params = new URLSearchParams({ filename: file.name })
  const res = await fetch(`${homeserver}/_matrix/media/v3/upload?${params}`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': file.type || 'application/octet-stream',
    },
    body: file,
  })
  if (!res.ok) {
    const err = (await res.json().catch(() => ({}))) as { error?: string }
    throw new Error(err.error ?? `upload failed: ${res.status}`)
  }
  const data = (await res.json()) as { content_uri: string }
  return data.content_uri
}
