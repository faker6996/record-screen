export type AppSurface = 'main' | 'hud' | 'region-selector' | 'target-preview'

export interface RegionSelectorSurfaceContext {
  originX: number
  originY: number
  width: number
  height: number
  scaleFactor: number
  captureTargetId: string
  initialRegion?: {
    left: number
    top: number
    width: number
    height: number
  }
}

export interface TargetPreviewSurfaceContext {
  title: string
  detail?: string
  sequence: number
  style: 'badge' | 'region-outline'
}

declare global {
  interface Window {
    __RECORD_SCREEN_SURFACE__?: AppSurface
    __RECORD_SCREEN_SELECTOR_CONTEXT__?: RegionSelectorSurfaceContext
    __RECORD_SCREEN_TARGET_PREVIEW_CONTEXT__?: TargetPreviewSurfaceContext
  }
}

export function getAppSurface(): AppSurface {
  if (typeof window === 'undefined') {
    return 'main'
  }

  if (window.__RECORD_SCREEN_SURFACE__ === 'hud') {
    return 'hud'
  }

  if (window.__RECORD_SCREEN_SURFACE__ === 'region-selector') {
    return 'region-selector'
  }

  if (window.__RECORD_SCREEN_SURFACE__ === 'target-preview') {
    return 'target-preview'
  }

  return 'main'
}
