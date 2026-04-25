import { create } from 'zustand'
import { MatrixClient } from 'matrix-js-sdk'
import { getClient, clearClient, STORAGE_KEY } from '../api/client'
import { logout as apiLogout } from '../api/auth'

interface AuthState {
  client: MatrixClient | null
  userId: string | null
  deviceId: string | null
  /** localStorage から認証情報を読み込み、クライアントを復元する */
  hydrate: () => void
  /** ログイン後に呼ぶ。client を store にセットする */
  setClient: (client: MatrixClient, userId: string, deviceId: string) => void
  /** ログアウト */
  logout: () => Promise<void>
}

// ストア生成時に localStorage から同期的に読み込む（リロード後の再認証防止）
const _initialClient = getClient()
const _initialDeviceId = _initialClient ? localStorage.getItem(STORAGE_KEY.DEVICE_ID) : null

export const useAuthStore = create<AuthState>((set, get) => ({
  client: _initialClient,
  userId: _initialClient?.getUserId() ?? null,
  deviceId: _initialDeviceId,

  hydrate() {
    // 初期化済みなら何もしない
    if (get().client) return
    const client = getClient()
    if (client) {
      const deviceId = localStorage.getItem(STORAGE_KEY.DEVICE_ID)
      set({ client, userId: client.getUserId(), deviceId })
    }
  },

  setClient(client, userId, deviceId) {
    set({ client, userId, deviceId })
  },

  async logout() {
    const { client } = get()
    if (client) {
      await apiLogout(client)
    } else {
      clearClient()
    }
    set({ client: null, userId: null })
  },
}))
