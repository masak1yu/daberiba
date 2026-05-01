import { createClient, type MatrixClient } from 'matrix-js-sdk'
import { clearClient, initClient } from './client'

export interface LoginResult {
  client: MatrixClient
  userId: string
  deviceId: string
  accessToken: string
  homeserver: string
}

export interface IdentityProvider {
  id: string
  name: string
}

export interface LoginFlows {
  flows: { type: string }[]
  identity_providers?: IdentityProvider[]
}

/** GET /login でサポートされるログインフロー一覧を取得する。 */
export async function fetchLoginFlows(homeserver: string): Promise<LoginFlows> {
  const res = await fetch(`${homeserver}/_matrix/client/v3/login`)
  if (!res.ok) throw new Error(`Failed to fetch login flows: ${res.status}`)
  return res.json() as Promise<LoginFlows>
}

/** パスワードログイン。成功時に MatrixClient を初期化して認証情報を返す。 */
export async function login(params: {
  homeserver: string
  username: string
  password: string
}): Promise<LoginResult> {
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

/** SSO コールバック後に受け取った loginToken を accessToken に交換する。 */
export async function loginWithToken(params: {
  homeserver: string
  loginToken: string
}): Promise<LoginResult> {
  const res = await fetch(`${params.homeserver}/_matrix/client/v3/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ type: 'm.login.token', token: params.loginToken }),
  })
  if (!res.ok) {
    const body = (await res.json().catch(() => ({}))) as { error?: string }
    throw new Error(body.error ?? `Login failed: ${res.status}`)
  }
  const data = (await res.json()) as {
    access_token: string
    user_id: string
    device_id: string
  }
  const client = initClient({
    homeserver: params.homeserver,
    accessToken: data.access_token,
    userId: data.user_id,
    deviceId: data.device_id,
  })
  return {
    client,
    userId: data.user_id,
    deviceId: data.device_id,
    accessToken: data.access_token,
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
