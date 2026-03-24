import { createClient, MatrixClient } from 'matrix-js-sdk'

// localStorage キー
export const STORAGE_KEY = {
  ACCESS_TOKEN: 'mx_access_token',
  USER_ID: 'mx_user_id',
  DEVICE_ID: 'mx_device_id',
  HOMESERVER: 'mx_homeserver',
} as const

let _client: MatrixClient | null = null

/** 保存済み認証情報から MatrixClient を生成して返す（未認証なら null） */
export function getClient(): MatrixClient | null {
  if (_client) return _client

  const accessToken = localStorage.getItem(STORAGE_KEY.ACCESS_TOKEN)
  const userId = localStorage.getItem(STORAGE_KEY.USER_ID)
  const deviceId = localStorage.getItem(STORAGE_KEY.DEVICE_ID)
  const homeserver = localStorage.getItem(STORAGE_KEY.HOMESERVER)

  if (!accessToken || !userId || !homeserver) return null

  _client = createClient({
    baseUrl: homeserver,
    accessToken,
    userId,
    deviceId: deviceId ?? undefined,
  })
  return _client
}

/** 認証情報を設定して MatrixClient を（再）生成する */
export function initClient(params: {
  homeserver: string
  accessToken: string
  userId: string
  deviceId: string
}): MatrixClient {
  localStorage.setItem(STORAGE_KEY.HOMESERVER, params.homeserver)
  localStorage.setItem(STORAGE_KEY.ACCESS_TOKEN, params.accessToken)
  localStorage.setItem(STORAGE_KEY.USER_ID, params.userId)
  localStorage.setItem(STORAGE_KEY.DEVICE_ID, params.deviceId)

  _client = createClient({
    baseUrl: params.homeserver,
    accessToken: params.accessToken,
    userId: params.userId,
    deviceId: params.deviceId,
  })
  return _client
}

/** 認証情報を削除してクライアントを破棄する */
export function clearClient(): void {
  Object.values(STORAGE_KEY).forEach((k) => localStorage.removeItem(k))
  _client = null
}
