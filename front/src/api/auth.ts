import { createClient, type MatrixClient } from 'matrix-js-sdk'
import { clearClient, initClient } from './client'

export interface LoginResult {
  client: MatrixClient
  userId: string
  deviceId: string
  accessToken: string
  homeserver: string
}

/**
 * パスワードログイン。成功時に MatrixClient を初期化して認証情報を返す。
 * homeserver は http(s):// スキームを含む URL（例: https://matrix.example.com）
 */
export async function login(params: {
  homeserver: string
  username: string
  password: string
}): Promise<LoginResult> {
  // 認証前の一時クライアント（アクセストークン不要）
  const tempClient = createClient({ baseUrl: params.homeserver })

  const res = await tempClient.login('m.login.password', {
    identifier: { type: 'm.id.user', user: params.username },
    password: params.password,
  })

  const client = initClient({
    homeserver: params.homeserver,
    accessToken: res.access_token,
    userId: res.user_id,
    deviceId: res.device_id,
  })

  return {
    client,
    userId: res.user_id,
    deviceId: res.device_id,
    accessToken: res.access_token,
    homeserver: params.homeserver,
  }
}

/** ログアウト。サーバー側のセッションを失効させてから認証情報を削除する。 */
export async function logout(client: ReturnType<typeof initClient>): Promise<void> {
  try {
    await client.logout(true)
  } catch {
    // サーバーエラーでもローカルは必ずクリアする
  } finally {
    clearClient()
  }
}
