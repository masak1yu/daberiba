import { create } from 'zustand'
import { MatrixClient } from 'matrix-js-sdk'
import { getClient, clearClient } from '../api/client'
import { logout as apiLogout } from '../api/auth'

interface AuthState {
  client: MatrixClient | null
  userId: string | null
  /** localStorage から認証情報を読み込み、クライアントを復元する */
  hydrate: () => void
  /** ログイン後に呼ぶ。client を store にセットする */
  setClient: (client: MatrixClient, userId: string) => void
  /** ログアウト */
  logout: () => Promise<void>
}

export const useAuthStore = create<AuthState>((set, get) => ({
  client: null,
  userId: null,

  hydrate() {
    const client = getClient()
    if (client) {
      set({ client, userId: client.getUserId() })
    }
  },

  setClient(client, userId) {
    set({ client, userId })
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
