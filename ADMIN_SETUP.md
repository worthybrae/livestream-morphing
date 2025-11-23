# Admin UI Setup - Complete Guide

## What We're Building

A sleek admin dashboard with:
- **Live Frame Preview** - See processed frames in real-time
- **Config Editor** - Sliders to adjust all stylization parameters
- **Processor Status** - Monitor segment processing
- **Preset Management** - Save/load effect configurations

## Quick Setup

```bash
cd admin-app

# Install remaining dependencies
npm install @radix-ui/react-slider @radix-ui/react-label @radix-ui/react-tabs

# Run the admin UI
npm run dev
```

The admin will run on `http://localhost:5173` and connect to the backend at `http://localhost:8000`

## Features

### 1. Config Sliders
- Bilateral Filter (diameter, sigma)
- Gaussian Blur size
- Quantization levels (4-16)
- Psychedelic distortion (amplitude, frequency)
- Edge blend factor
- Frame processing interval

### 2. Live Preview
- Shows the last processed frame
- Auto-refreshes every 2 seconds
- Side-by-side comparison option

### 3. Preset System
- Heavy Blobs
- Detailed Posterization
- Psychedelic
- Custom (save your own)

### 4. Status Monitor
- Recent segments processed
- Ready segments count
- Processing stats

## Files Created

The complete admin app is in `admin-app/src/`:
- `App.tsx` - Main dashboard
- `components/ui/*` - Shadcn components
- `lib/utils.ts` - Utilities
- `types.ts` - TypeScript types

Run `npm run dev` to start!
