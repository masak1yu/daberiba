import { useShallow } from 'zustand/react/shallow'
import { useUiStore } from '../../stores/ui'

const TYPE_CLASSES = {
  info: 'bg-gray-800 text-white',
  success: 'bg-green-700 text-white',
  error: 'bg-red-700 text-white',
}

export default function ToastStack() {
  const { toasts, dismissToast } = useUiStore(
    useShallow((s) => ({ toasts: s.toasts, dismissToast: s.dismissToast }))
  )

  if (toasts.length === 0) return null

  return (
    <div
      className="fixed inset-x-0 bottom-0 z-50 flex flex-col items-center gap-2 px-4 pb-6"
      style={{ paddingBottom: 'max(env(safe-area-inset-bottom), 1.5rem)' }}
    >
      {toasts.map((t) => (
        <button
          key={t.id}
          onClick={() => dismissToast(t.id)}
          className={`w-full max-w-sm rounded-xl px-4 py-3 text-sm font-medium shadow-lg ${TYPE_CLASSES[t.type]}`}
        >
          {t.message}
        </button>
      ))}
    </div>
  )
}
