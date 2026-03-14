export type AppSurface = 'main' | 'hud'

declare global {
  interface Window {
    __RECORD_SCREEN_SURFACE__?: AppSurface
  }
}

export function getAppSurface(): AppSurface {
  if (typeof window === 'undefined') {
    return 'main'
  }

  return window.__RECORD_SCREEN_SURFACE__ === 'hud' ? 'hud' : 'main'
}
