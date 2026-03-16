import { useEffect, useState } from 'react'

import type { TargetPreviewSurfaceContext } from '../../../app/surfaces'

const PREVIEW_EVENT = 'record-screen:target-preview'

function readPreviewContext(): TargetPreviewSurfaceContext {
  return (
    window.__RECORD_SCREEN_TARGET_PREVIEW_CONTEXT__ ?? {
      title: '',
      sequence: 0,
    }
  )
}

export function TargetPreviewSurface() {
  const [context, setContext] = useState<TargetPreviewSurfaceContext>(() =>
    readPreviewContext(),
  )

  useEffect(() => {
    const sync = () => setContext(readPreviewContext())
    window.addEventListener(PREVIEW_EVENT, sync)
    return () => window.removeEventListener(PREVIEW_EVENT, sync)
  }, [])

  return (
    <main className="target-preview" aria-hidden="true">
      <div className="target-preview__veil" />
      <div className="target-preview__badge" key={context.sequence}>
        <span className="target-preview__title">{context.title}</span>
      </div>
    </main>
  )
}
