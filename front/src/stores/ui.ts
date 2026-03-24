import { create } from 'zustand'

interface Toast {
  id: number
  message: string
  type: 'info' | 'error' | 'success'
}

interface UiState {
  isOnline: boolean
  toasts: Toast[]
  /** オンライン/オフライン監視を開始する（アプリ起動時に1回だけ呼ぶ） */
  startNetworkWatch: () => () => void
  showToast: (message: string, type?: Toast['type'], durationMs?: number) => void
  dismissToast: (id: number) => void
}

let _toastId = 0

export const useUiStore = create<UiState>((set, get) => ({
  isOnline: navigator.onLine,
  toasts: [],

  startNetworkWatch() {
    const onOnline = () => {
      set({ isOnline: true })
      get().showToast('再接続しました', 'success', 3000)
    }
    const onOffline = () => {
      set({ isOnline: false })
      get().showToast('オフラインです', 'error', 0) // 0 = 自動非表示しない
    }
    window.addEventListener('online', onOnline)
    window.addEventListener('offline', onOffline)
    return () => {
      window.removeEventListener('online', onOnline)
      window.removeEventListener('offline', onOffline)
    }
  },

  showToast(message, type = 'info', durationMs = 4000) {
    const id = ++_toastId
    set((s) => ({ toasts: [...s.toasts, { id, message, type }] }))
    if (durationMs > 0) {
      setTimeout(() => get().dismissToast(id), durationMs)
    }
  },

  dismissToast(id) {
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }))
  },
}))
