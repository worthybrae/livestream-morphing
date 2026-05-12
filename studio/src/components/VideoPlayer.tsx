import { useEffect, useRef } from 'react'
import Hls from 'hls.js'

export function VideoPlayer() {
  const videoRef = useRef<HTMLVideoElement>(null)

  useEffect(() => {
    const video = videoRef.current
    if (!video) return

    const src = '/api/stream'

    if (Hls.isSupported()) {
      const hls = new Hls({
        liveSyncDurationCount: 2,
        liveMaxLatencyDurationCount: 4,
        enableWorker: true,
        manifestLoadingRetryDelay: 3000,
        manifestLoadingMaxRetry: 100,
        levelLoadingRetryDelay: 3000,
        levelLoadingMaxRetry: 100,
      })
      hls.loadSource(src)
      hls.attachMedia(video)
      hls.on(Hls.Events.MANIFEST_PARSED, () => {
        video.play().catch(() => {})
      })
      // Recover from fatal errors by reloading the source
      hls.on(Hls.Events.ERROR, (_event, data) => {
        if (data.fatal) {
          if (data.type === Hls.ErrorTypes.NETWORK_ERROR) {
            setTimeout(() => hls.loadSource(src), 3000)
          } else if (data.type === Hls.ErrorTypes.MEDIA_ERROR) {
            hls.recoverMediaError()
          }
        }
      })
      return () => hls.destroy()
    } else if (video.canPlayType('application/vnd.apple.mpegurl')) {
      video.src = src
      video.addEventListener('loadedmetadata', () => {
        video.play().catch(() => {})
      })
    }
  }, [])

  return (
    <div className="flex-1 bg-black flex items-center justify-center relative">
      <video
        ref={videoRef}
        className="w-full h-full object-contain"
        muted
        playsInline
      />
      <div className="absolute top-3 left-3.5 flex items-center gap-1.5">
        <div
          className="w-1.5 h-1.5 rounded-full animate-live-pulse"
          style={{ background: '#ef4444' }}
        />
        <span className="text-[10px] text-white/50 font-semibold tracking-wide">
          LIVE
        </span>
      </div>
    </div>
  )
}
