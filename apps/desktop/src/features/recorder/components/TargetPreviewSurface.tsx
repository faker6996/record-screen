import { useEffect, useState } from 'react'

import type { TargetPreviewSurfaceContext } from '../../../app/surfaces'

const PREVIEW_EVENT = 'record-screen:target-preview'

function readPreviewContext(): TargetPreviewSurfaceContext {
  return (
    window.__RECORD_SCREEN_TARGET_PREVIEW_CONTEXT__ ?? {
      title: '',
      detail: '',
      sequence: 0,
      style: 'badge',
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
      {context.style === 'region-outline' ? (
        <div className="target-preview__region" key={context.sequence}>
          <div className="target-preview__region-chip">
            <span className="target-preview__region-title">{context.title}</span>
            {context.detail ? (
              <span className="target-preview__region-detail">{context.detail}</span>
            ) : null}
          </div>
          <div className="target-preview__region-frame" />
          <span className="target-preview__handle target-preview__handle--nw" />
          <span className="target-preview__handle target-preview__handle--n" />
          <span className="target-preview__handle target-preview__handle--ne" />
          <span className="target-preview__handle target-preview__handle--e" />
          <span className="target-preview__handle target-preview__handle--se" />
          <span className="target-preview__handle target-preview__handle--s" />
          <span className="target-preview__handle target-preview__handle--sw" />
          <span className="target-preview__handle target-preview__handle--w" />
        </div>
      ) : (
        <>
          <div className="target-preview__veil" />
          <div className="target-preview__guides" aria-hidden="true">
            <span className="target-preview__guide target-preview__guide--horizontal" />
            <span className="target-preview__guide target-preview__guide--vertical" />
          </div>
          <div className="target-preview__badge" key={context.sequence}>
            <span className="target-preview__eyebrow">Selected display</span>
            <span className="target-preview__title">{context.title}</span>
          </div>
        </>
      )}
    </main>
  )
}
