import { useEffect, useRef, useState } from 'react'
import Hls from 'hls.js'

interface VideoPreviewProps {
  frameUrl: string | null
}

interface ProcessorStatus {
  running: boolean
  recent_segments: number[]
  ready_segments: number[]
  total_processed: number
  total_ready: number
  avg_processing_time: number
  avg_download_time: number
  avg_total_time: number
}

export function VideoPreview({ frameUrl }: VideoPreviewProps) {
  const videoRef = useRef<HTMLVideoElement>(null)
  const hlsRef = useRef<Hls | null>(null)
  const [showRaw, setShowRaw] = useState(false)
  const [reloadProgress, setReloadProgress] = useState(0)
  const [isReloading, setIsReloading] = useState(false)
  const [status, setStatus] = useState<ProcessorStatus | null>(null)
  const [currentSegment, setCurrentSegment] = useState<number | null>(null)
  const [firstNewSegment, setFirstNewSegment] = useState<number | null>(null)
  const [hasPlayedNewSegment, setHasPlayedNewSegment] = useState(false)

  useEffect(() => {
    const video = videoRef.current
    if (!video) return

    // Use raw Abbey Road stream (proxied through backend) or processed stream
    const streamUrl = showRaw
      ? 'http://localhost:8000/api/raw'
      : 'http://localhost:8000/api/stream'

    const initializeVideo = () => {
      // Clean up existing HLS instance
      if (hlsRef.current) {
        hlsRef.current.destroy()
        hlsRef.current = null
      }

      // Check if HLS is natively supported (Safari)
      if (video.canPlayType('application/vnd.apple.mpegurl')) {
        video.src = streamUrl
        video.load()
        video.play().catch(err => console.log('Auto-play prevented:', err))
      }
      // Use hls.js for browsers that don't support HLS natively (Chrome, Firefox)
      else if (Hls.isSupported()) {
        const hls = new Hls({
          enableWorker: true,
          lowLatencyMode: false, // Disable low latency for smoother playback
          backBufferLength: 90,
          maxBufferLength: 60, // Buffer up to 60 seconds
          maxMaxBufferLength: 120, // Max buffer size
          maxBufferSize: 60 * 1000 * 1000, // 60MB max buffer
          maxBufferHole: 0.5, // Jump over small gaps
          highBufferWatchdogPeriod: 3, // More aggressive buffer management
          nudgeMaxRetry: 10, // More retries for gaps
        })

        hls.loadSource(streamUrl)
        hls.attachMedia(video)

        hls.on(Hls.Events.MANIFEST_PARSED, () => {
          video.play().catch(err => console.log('Auto-play prevented:', err))
        })

        hls.on(Hls.Events.ERROR, (event, data) => {
          console.error('HLS error:', data)
        })

        // Track current segment being played
        hls.on(Hls.Events.FRAG_CHANGED, (event, data) => {
          if (data.frag.url) {
            console.log('Fragment changed, URL:', data.frag.url)

            // For processed stream: extract segment ID from URL like "http://localhost:8000/api/segments/248123.ts"
            // For raw stream: extract from URL like "https://XXX.akamaized.net/.../248123.ts"
            const processedMatch = data.frag.url.match(/\/segments\/(\d+)\.ts/)
            const rawMatch = data.frag.url.match(/\/(\d+)\.ts/)

            const segId = processedMatch
              ? parseInt(processedMatch[1])
              : rawMatch
                ? parseInt(rawMatch[1])
                : null

            if (segId) {
              console.log('Setting current segment to:', segId)
              setCurrentSegment(segId)

              // Check if this is the first new segment after code update (only for processed stream)
              if (!showRaw && firstNewSegment && segId >= firstNewSegment && !hasPlayedNewSegment) {
                console.log(`✅ First new segment ${segId} is now playing!`)
                setHasPlayedNewSegment(true)
                setReloadProgress(100)

                // Clear the indicator after a moment
                setTimeout(() => {
                  setIsReloading(false)
                  setReloadProgress(0)
                  setFirstNewSegment(null)
                }, 2000)
              }
            }
          }
        })

        hlsRef.current = hls
      }
    }

    // Initialize video on mount
    initializeVideo()

    // Listen for code updates to track first new segment
    const handleCodeUpdate = (event: any) => {
      console.log('🔄 Code updated', event.detail)
      const firstNew = event.detail?.firstNewSegment

      if (firstNew) {
        setFirstNewSegment(firstNew)
        setHasPlayedNewSegment(false)
        setIsReloading(true)
        setReloadProgress(0)

        // Progress will complete when the first new segment plays
        console.log(`⏳ Waiting for segment ${firstNew} to see new effects`)
      }
    }

    window.addEventListener('code-updated', handleCodeUpdate)

    // Cleanup
    return () => {
      if (hlsRef.current) {
        hlsRef.current.destroy()
      }
      window.removeEventListener('code-updated', handleCodeUpdate)
    }
  }, [showRaw])

  // Poll processor status for timeline
  useEffect(() => {
    const fetchStatus = async () => {
      try {
        const res = await fetch('http://localhost:8000/api/admin/status')
        const data = await res.json()
        setStatus(data)
      } catch (err) {
        console.error('Failed to fetch status:', err)
      }
    }

    fetchStatus()
    const interval = setInterval(fetchStatus, 1000) // Poll every second for smoother updates

    return () => clearInterval(interval)
  }, [])


  return (
    <div className="w-1/2 bg-black flex items-center justify-center relative">
      {/* Toggle buttons - matching header padding */}
      <div className="absolute top-4 z-10 flex gap-2" style={{ left: '2rem' }}>
        <button
          onClick={() => setShowRaw(false)}
          className="text-sm font-medium rounded border-2 transition-all"
          style={{
            paddingLeft: '1.25rem',
            paddingRight: '1.25rem',
            paddingTop: '0.625rem',
            paddingBottom: '0.625rem',
            backgroundColor: !showRaw ? 'rgba(255, 255, 255, 0.95)' : 'rgba(0, 0, 0, 0.3)',
            color: !showRaw ? '#000' : '#fff',
            borderColor: '#fff'
          }}
        >
          🎨 Processed
        </button>
        <button
          onClick={() => setShowRaw(true)}
          className="text-sm font-medium rounded border-2 transition-all"
          style={{
            paddingLeft: '1.25rem',
            paddingRight: '1.25rem',
            paddingTop: '0.625rem',
            paddingBottom: '0.625rem',
            backgroundColor: showRaw ? 'rgba(255, 255, 255, 0.95)' : 'rgba(0, 0, 0, 0.3)',
            color: showRaw ? '#000' : '#fff',
            borderColor: '#fff'
          }}
        >
          📹 Raw Input
        </button>
      </div>

      {/* Reload progress indicator */}
      {isReloading && (
        <div className="absolute top-4 right-0 z-10 bg-black/80 text-white px-4 py-2 rounded-l text-xs" style={{ right: '2rem' }}>
          <div className="flex items-center gap-2">
            <span>Processing new segments...</span>
            <div className="w-24 h-1.5 bg-zinc-700 rounded-full overflow-hidden">
              <div
                className="h-full bg-white transition-all duration-100"
                style={{ width: `${reloadProgress}%` }}
              />
            </div>
            <span className="text-zinc-400 tabular-nums">{Math.floor(reloadProgress)}%</span>
          </div>
        </div>
      )}

      <video
        ref={videoRef}
        className="max-w-full max-h-full object-contain"
        autoPlay
        loop
        muted
        playsInline
        style={{ width: '100%', height: '100%' }}
      >
        Your browser does not support the video tag.
      </video>

      {!frameUrl && (
        <div className="absolute inset-0 flex items-center justify-center text-center text-zinc-600 pointer-events-none">
          <div>
            <svg className="w-12 h-12 mx-auto mb-3 text-zinc-700" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1} d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z" />
            </svg>
            <p className="text-sm">Loading stream...</p>
          </div>
        </div>
      )}

      {/* Segment Timeline */}
      {status && status.ready_segments.length > 0 && (
        <div className="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black via-black/95 to-black/80 text-white border-t border-zinc-800/50" style={{ paddingLeft: '2rem', paddingRight: '2rem', paddingTop: '1.25rem', paddingBottom: '1.25rem' }}>
          {/* Performance Metrics */}
          <div className="flex items-center gap-6" style={{ marginBottom: '1.5rem' }}>
            <div className="flex items-center gap-4">
              <div className="text-base text-zinc-400 font-semibold">Performance</div>
              <div className="flex items-center gap-2">
                <span className="text-sm text-zinc-400">Download:</span>
                <span className="font-mono text-white font-bold text-base">{status.avg_download_time}s</span>
              </div>
              <div className="w-px h-5 bg-zinc-700"></div>
              <div className="flex items-center gap-2">
                <span className="text-sm text-zinc-400">Process:</span>
                <span className="font-mono text-white font-bold text-base">{status.avg_processing_time}s</span>
              </div>
              <div className="w-px h-5 bg-zinc-700"></div>
              <div className="flex items-center gap-2">
                <span className="text-sm text-zinc-400">Total:</span>
                <span className={`font-mono font-bold text-lg ${status.avg_total_time > 6 ? 'text-red-400' : 'text-green-400'}`}>
                  {status.avg_total_time}s
                </span>
                {status.avg_total_time > 6 && (
                  <span className="text-red-400 text-sm font-semibold">⚠ Lagging</span>
                )}
              </div>
              <div className="w-px h-5 bg-zinc-700"></div>
              <div className="flex items-center gap-2">
                <span className="font-bold text-white text-lg">{status.total_ready}</span>
                <span className="text-sm text-zinc-400">segments ready</span>
              </div>
            </div>
          </div>

          {/* Timeline */}
          <div className="flex items-center gap-4">
            <div className="text-base text-zinc-400 font-semibold" style={{ width: '100px' }}>Segments</div>
            <div className="flex-1 flex items-center gap-3 overflow-x-auto pb-2">
              {status.ready_segments.slice(0, 10).map((segmentId, idx) => {
                const isPlaying = currentSegment === segmentId
                const hasPlayed = currentSegment && segmentId < currentSegment
                const hasNewCode = firstNewSegment && segmentId >= firstNewSegment

                return (
                  <div
                    key={segmentId}
                    className="relative flex-shrink-0"
                    style={{ width: '110px' }}
                  >
                    {/* Segment box */}
                    <div
                      className="rounded-lg text-center transition-all relative overflow-hidden"
                      style={{
                        paddingLeft: '1rem',
                        paddingRight: '1rem',
                        paddingTop: '1rem',
                        paddingBottom: '1rem',
                        backgroundColor: isPlaying
                          ? 'rgba(34, 197, 94, 0.15)'
                          : hasPlayed
                          ? 'rgba(100, 116, 139, 0.1)'
                          : 'rgba(63, 63, 70, 0.2)',
                        borderWidth: '3px',
                        borderStyle: 'solid',
                        borderColor: hasPlayed
                          ? 'rgb(71, 85, 105)'
                          : 'rgb(63, 63, 70)',
                        color: isPlaying
                          ? 'rgb(34, 197, 94)'
                          : hasPlayed
                          ? 'rgb(148, 163, 184)'
                          : 'rgb(161, 161, 170)'
                      }}
                    >
                      <div className="font-mono font-semibold relative z-10 flex items-center justify-center gap-1" style={{ fontSize: '14px' }}>
                        {isPlaying && '▶ '}
                        {String(segmentId).slice(-4)}
                        {hasNewCode && <span style={{ fontSize: '12px' }}>✨</span>}
                      </div>

                      {/* Progress bar at the bottom */}
                      {isPlaying && (
                        <div
                          className="absolute bottom-0 left-0 right-0 h-1 bg-green-500"
                          style={{
                            animation: 'progress 6s linear infinite',
                            transformOrigin: 'left'
                          }}
                        />
                      )}

                      {/* Animated border for playing segment */}
                      {isPlaying && (
                        <div
                          className="absolute rounded-lg"
                          style={{
                            top: '-3px',
                            left: '-3px',
                            right: '-3px',
                            bottom: '-3px',
                            borderWidth: '3px',
                            borderStyle: 'solid',
                            borderColor: 'rgb(34, 197, 94)',
                            animation: 'glow 1.5s ease-in-out infinite'
                          }}
                        />
                      )}
                    </div>
                  </div>
                )
              })}
            </div>
          </div>
        </div>
      )}

      <style>{`
        @keyframes glow {
          0% {
            box-shadow: 0 0 5px rgba(34, 197, 94, 0.5), 0 0 10px rgba(34, 197, 94, 0.3);
            opacity: 1;
          }
          50% {
            box-shadow: 0 0 15px rgba(34, 197, 94, 0.8), 0 0 25px rgba(34, 197, 94, 0.5);
            opacity: 0.9;
          }
          100% {
            box-shadow: 0 0 5px rgba(34, 197, 94, 0.5), 0 0 10px rgba(34, 197, 94, 0.3);
            opacity: 1;
          }
        }

        @keyframes progress {
          0% {
            width: 0%;
          }
          100% {
            width: 100%;
          }
        }
      `}</style>
    </div>
  )
}
