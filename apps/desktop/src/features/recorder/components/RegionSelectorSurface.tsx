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

type ResizeHandle = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w'

type DragMode =
  | {
      kind: 'draw'
      start: DragPoint
    }
  | {
      kind: 'move'
      pointerOffsetX: number
      pointerOffsetY: number
      rect: DragRect
    }
  | {
      kind: 'resize'
      handle: ResizeHandle
      start: DragPoint
      rect: DragRect
    }

function normalizeRect(start: DragPoint, end: DragPoint): DragRect {
  const left = Math.min(start.x, end.x)
  const top = Math.min(start.y, end.y)
  const width = Math.abs(end.x - start.x)
  const height = Math.abs(end.y - start.y)

  return { left, top, width, height }
}

function clampRectToBounds(rect: DragRect, context: RegionSelectorSurfaceContext): DragRect {
  const maxLeft = Math.max(0, context.width - 64)
  const maxTop = Math.max(0, context.height - 64)
  const left = Math.min(Math.max(0, rect.left), maxLeft)
  const top = Math.min(Math.max(0, rect.top), maxTop)
  const width = Math.min(Math.max(64, rect.width), context.width - left)
  const height = Math.min(Math.max(64, rect.height), context.height - top)

  return {
    left,
    top,
    width,
    height,
  }
}

function pointInRect(point: DragPoint, rect: DragRect) {
  return (
    point.x >= rect.left &&
    point.x <= rect.left + rect.width &&
    point.y >= rect.top &&
    point.y <= rect.top + rect.height
  )
}

function detectResizeHandle(
  point: DragPoint,
  rect: DragRect,
): ResizeHandle | null {
  const handleRadius = 16
  const horizontalCenter = rect.left + rect.width / 2
  const verticalCenter = rect.top + rect.height / 2
  const anchors: Array<{ handle: ResizeHandle; x: number; y: number }> = [
    { handle: 'nw', x: rect.left, y: rect.top },
    { handle: 'n', x: horizontalCenter, y: rect.top },
    { handle: 'ne', x: rect.left + rect.width, y: rect.top },
    { handle: 'e', x: rect.left + rect.width, y: verticalCenter },
    { handle: 'se', x: rect.left + rect.width, y: rect.top + rect.height },
    { handle: 's', x: horizontalCenter, y: rect.top + rect.height },
    { handle: 'sw', x: rect.left, y: rect.top + rect.height },
    { handle: 'w', x: rect.left, y: verticalCenter },
  ]

  return (
    anchors.find(
      (anchor) =>
        Math.abs(point.x - anchor.x) <= handleRadius &&
        Math.abs(point.y - anchor.y) <= handleRadius,
    )?.handle ?? null
  )
}

function resizeRect(
  baseRect: DragRect,
  handle: ResizeHandle,
  start: DragPoint,
  current: DragPoint,
): DragRect {
  let left = baseRect.left
  let top = baseRect.top
  let right = baseRect.left + baseRect.width
  let bottom = baseRect.top + baseRect.height
  const deltaX = current.x - start.x
  const deltaY = current.y - start.y

  if (handle.includes('w')) {
    left += deltaX
  }
  if (handle.includes('e')) {
    right += deltaX
  }
  if (handle.includes('n')) {
    top += deltaY
  }
  if (handle.includes('s')) {
    bottom += deltaY
  }

  return normalizeRect({ x: left, y: top }, { x: right, y: bottom })
}

function moveRect(
  pointer: DragPoint,
  rect: DragRect,
  pointerOffsetX: number,
  pointerOffsetY: number,
): DragRect {
  return {
    left: pointer.x - pointerOffsetX,
    top: pointer.y - pointerOffsetY,
    width: rect.width,
    height: rect.height,
  }
}

function toRegionCoordinates(
  rect: DragRect,
  context: RegionSelectorSurfaceContext,
) {
  const scaleFactor = context.scaleFactor || 1

  return {
    x: Math.max(0, Math.round(rect.left * scaleFactor)),
    y: Math.max(0, Math.round(rect.top * scaleFactor)),
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
    captureTargetId: 'full-desktop',
    initialRegion: undefined,
  }
  const [dragMode, setDragMode] = useState<DragMode | null>(null)
  const [selectionRect, setSelectionRect] = useState<DragRect | null>(
    context.initialRegion ?? null,
  )
  const [isSaving, setIsSaving] = useState(false)
  const [hoverHandle, setHoverHandle] = useState<ResizeHandle | 'move' | null>(null)

  useEffect(() => {
    setSelectionRect(context.initialRegion ?? null)
    setDragMode(null)
    setHoverHandle(null)
  }, [context])

  const selectionCursor = useMemo(() => {
    if (dragMode?.kind === 'move' || hoverHandle === 'move') {
      return 'move'
    }

    const handle = dragMode?.kind === 'resize' ? dragMode.handle : hoverHandle
    switch (handle) {
      case 'nw':
      case 'se':
        return 'nwse-resize'
      case 'ne':
      case 'sw':
        return 'nesw-resize'
      case 'n':
      case 's':
        return 'ns-resize'
      case 'e':
      case 'w':
        return 'ew-resize'
      default:
        return 'crosshair'
    }
  }, [dragMode, hoverHandle])

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
        context.captureTargetId,
        context.originX,
        context.originY,
        Math.max(1, Math.round((context.scaleFactor || 1) * 1000)),
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
      style={{ cursor: selectionCursor }}
      onPointerDown={(event) => {
        if (isSaving || event.button !== 0) {
          return
        }

        event.preventDefault()
        event.currentTarget.setPointerCapture(event.pointerId)
        const nextPoint = { x: event.clientX, y: event.clientY }
        if (selectionRect) {
          const nextHandle = detectResizeHandle(nextPoint, selectionRect)
          if (nextHandle) {
            setDragMode({
              kind: 'resize',
              handle: nextHandle,
              start: nextPoint,
              rect: selectionRect,
            })
            return
          }

          if (pointInRect(nextPoint, selectionRect)) {
            setDragMode({
              kind: 'move',
              pointerOffsetX: nextPoint.x - selectionRect.left,
              pointerOffsetY: nextPoint.y - selectionRect.top,
              rect: selectionRect,
            })
            return
          }
        }

        setDragMode({
          kind: 'draw',
          start: nextPoint,
        })
        setSelectionRect({
          left: nextPoint.x,
          top: nextPoint.y,
          width: 0,
          height: 0,
        })
      }}
      onPointerMove={(event) => {
        const nextPoint = { x: event.clientX, y: event.clientY }

        if (!dragMode || isSaving) {
          if (selectionRect) {
            const nextHandle = detectResizeHandle(nextPoint, selectionRect)
            if (nextHandle) {
              setHoverHandle(nextHandle)
              return
            }

            if (pointInRect(nextPoint, selectionRect)) {
              setHoverHandle('move')
              return
            }
          }

          setHoverHandle(null)
          return
        }

        if (dragMode.kind === 'draw') {
          setSelectionRect(
            clampRectToBounds(normalizeRect(dragMode.start, nextPoint), context),
          )
          return
        }

        if (dragMode.kind === 'move') {
          setSelectionRect(
            clampRectToBounds(
              moveRect(
                nextPoint,
                dragMode.rect,
                dragMode.pointerOffsetX,
                dragMode.pointerOffsetY,
              ),
              context,
            ),
          )
          return
        }

        setSelectionRect(
          clampRectToBounds(
            resizeRect(dragMode.rect, dragMode.handle, dragMode.start, nextPoint),
            context,
          ),
        )
      }}
      onPointerUp={(event) => {
        if (!dragMode || isSaving) {
          return
        }

        if (event.currentTarget.hasPointerCapture(event.pointerId)) {
          event.currentTarget.releasePointerCapture(event.pointerId)
        }
        let rect = selectionRect
        if (dragMode.kind === 'draw') {
          rect = clampRectToBounds(
            normalizeRect(dragMode.start, { x: event.clientX, y: event.clientY }),
            context,
          )
        }
        setDragMode(null)

        if (!rect || rect.width < 12 || rect.height < 12) {
          return
        }

        void applySelection(rect)
      }}
      onPointerCancel={(event) => {
        if (event.currentTarget.hasPointerCapture(event.pointerId)) {
          event.currentTarget.releasePointerCapture(event.pointerId)
        }
        setDragMode(null)
      }}
    >
      <div className="region-selector__copy">
        <strong>Select a region</strong>
        <p>
          Drag a new box, drag inside the current box to move it, or use the
          handles to resize. Release to save. Press Esc to cancel.
        </p>
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
          <span className="region-selector__handle region-selector__handle--nw" />
          <span className="region-selector__handle region-selector__handle--n" />
          <span className="region-selector__handle region-selector__handle--ne" />
          <span className="region-selector__handle region-selector__handle--e" />
          <span className="region-selector__handle region-selector__handle--se" />
          <span className="region-selector__handle region-selector__handle--s" />
          <span className="region-selector__handle region-selector__handle--sw" />
          <span className="region-selector__handle region-selector__handle--w" />
        </div>
      ) : null}
    </main>
  )
}
