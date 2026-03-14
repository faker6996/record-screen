export type AppSurface = 'main' | 'hud' | 'region-selector' | 'target-preview'

export interface RegionSelectorSurfaceContext {
  originX: number
  originY: number
  width: number
  height: number
  scaleFactor: number
  captureTargetId: string
}

declare global {
  interface Window {
    __RECORD_SCREEN_SURFACE__?: AppSurface
    __RECORD_SCREEN_SELECTOR_CONTEXT__?: RegionSelectorSurfaceContext
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
