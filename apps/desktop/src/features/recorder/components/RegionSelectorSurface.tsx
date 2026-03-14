import { useEffect, useMemo, useState } from 'react'
import { desktopClient } from '../../../services/desktop-client'
import type { RegionSelectorSurfaceContext } from '../../../app/surfaces'

interface DragPoint {
  x: number
  y: number
}

interface DragRect {
  left: number
  top: number
  width: number
  height: number
}

function normalizeRect(start: DragPoint, end: DragPoint): DragRect {
  const left = Math.min(start.x, end.x)
  const top = Math.min(start.y, end.y)
  const width = Math.abs(end.x - start.x)
  const height = Math.abs(end.y - start.y)

  return { left, top, width, height }
}

function toRegionCoordinates(
  rect: DragRect,
  context: RegionSelectorSurfaceContext,
) {
  const scaleFactor = context.scaleFactor || 1

  return {
    x: Math.round(context.originX + rect.left * scaleFactor),
    y: Math.round(context.originY + rect.top * scaleFactor),
    width: Math.max(64, Math.round(rect.width * scaleFactor)),
    height: Math.max(64, Math.round(rect.height * scaleFactor)),
  }
}

export function RegionSelectorSurface() {
  const context = window.__RECORD_SCREEN_SELECTOR_CONTEXT__ ?? {
    originX: 0,
    originY: 0,
    width: window.innerWidth,
    height: window.innerHeight,
    scaleFactor: 1,
  }
  const [dragStart, setDragStart] = useState<DragPoint | null>(null)
  const [dragCurrent, setDragCurrent] = useState<DragPoint | null>(null)
  const [isSaving, setIsSaving] = useState(false)

  const selectionRect = useMemo(() => {
    if (!dragStart || !dragCurrent) {
      return null
    }

    return normalizeRect(dragStart, dragCurrent)
  }, [dragCurrent, dragStart])

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key !== 'Escape') {
        return
      }

      void desktopClient.hideRegionSelector()
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => {
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [])

  async function applySelection(rect: DragRect) {
    const physicalRect = toRegionCoordinates(rect, context)
    setIsSaving(true)
    try {
      await desktopClient.updateCustomRegion(
        physicalRect.x,
        physicalRect.y,
        physicalRect.width,
        physicalRect.height,
      )
      await desktopClient.updateCaptureTarget('region:custom')
      await desktopClient.hideRegionSelector()
      await desktopClient.focusLauncher()
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <main
      className="region-selector"
      onPointerDown={(event) => {
        if (isSaving || event.button !== 0) {
          return
        }

        const nextPoint = { x: event.clientX, y: event.clientY }
        setDragStart(nextPoint)
        setDragCurrent(nextPoint)
      }}
      onPointerMove={(event) => {
        if (!dragStart || isSaving) {
          return
        }

        setDragCurrent({ x: event.clientX, y: event.clientY })
      }}
      onPointerUp={(event) => {
        if (!dragStart || isSaving) {
          return
        }

        const rect = normalizeRect(dragStart, { x: event.clientX, y: event.clientY })
        setDragStart(null)
        setDragCurrent(null)

        if (rect.width < 12 || rect.height < 12) {
          return
        }

        void applySelection(rect)
      }}
    >
      <div className="region-selector__copy">
        <strong>Select a region</strong>
        <p>Drag to draw a capture box. Release to save. Press Esc to cancel.</p>
      </div>

      {selectionRect ? (
        <div
          className="region-selector__selection"
          style={{
            left: selectionRect.left,
            top: selectionRect.top,
            width: selectionRect.width,
            height: selectionRect.height,
          }}
        >
          <div className="region-selector__selection-label">
            {Math.round(selectionRect.width)} x {Math.round(selectionRect.height)}
          </div>
        </div>
      ) : null}
    </main>
  )
}
