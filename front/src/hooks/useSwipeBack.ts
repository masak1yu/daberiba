import { useEffect, useRef } from 'react'

/**
 * 画面左端（edgePx 以内）から右方向へ threshold px スワイプすると
 * onSwipeBack を呼ぶ。モバイルの「スワイプで戻る」ジェスチャーを模倣する。
 */
export function useSwipeBack(onSwipeBack: () => void, edgePx = 30, threshold = 60) {
  const startX = useRef<number | null>(null)
  const startY = useRef<number | null>(null)

  useEffect(() => {
    function onTouchStart(e: TouchEvent) {
      const t = e.touches[0]
      startX.current = t.clientX < edgePx ? t.clientX : null
      startY.current = t.clientX < edgePx ? t.clientY : null
    }

    function onTouchEnd(e: TouchEvent) {
      if (startX.current === null || startY.current === null) return
      const t = e.changedTouches[0]
      const dx = t.clientX - startX.current
      const dy = Math.abs(t.clientY - (startY.current ?? 0))
      // 水平方向優位のスワイプのみ反応
      if (dx > threshold && dy < dx * 0.6) {
        onSwipeBack()
      }
      startX.current = null
    }

    document.addEventListener('touchstart', onTouchStart, { passive: true })
    document.addEventListener('touchend', onTouchEnd, { passive: true })
    return () => {
      document.removeEventListener('touchstart', onTouchStart)
      document.removeEventListener('touchend', onTouchEnd)
    }
  }, [onSwipeBack, edgePx, threshold])
}
