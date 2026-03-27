import { create } from 'zustand'
import { MatrixClient } from 'matrix-js-sdk'
import { getClient, clearClient } from '../api/client'
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

export const useAuthStore = create<AuthState>((set, get) => ({
  client: null,
  userId: null,
  deviceId: null,

  hydrate() {
    const client = getClient()
    if (client) {
      const deviceId = localStorage.getItem('mx_device_id')
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
