#!/usr/bin/env python3
"""Prepare a fixed camera view for line tracing.

From a short clip of the scene this writes, into the scene folder:
  plate.jpg   the scene with passing cars and people averaged away
  edges.png   the scene's contours: what a stroke drawn on the tracing page
              settles onto

With --daylight it also takes a clip of the same place in better light, even one
framed a little differently, lays it over the plate and writes plate-day.jpg: a
second view to trace from. Its contours join edges.png, so a stroke can settle
on a line that is there but cannot be seen at night.

Run again whenever the camera is re-aimed.
"""
import argparse
import pathlib

import cv2
import numpy as np


def still(clip):
    """The scene of a clip: whatever stays put across it. Movers drop out of the median."""
    cap = cv2.VideoCapture(str(clip))
    frames, n = [], 0
    while True:
        ok, frame = cap.read()
        if not ok:
            break
        if n%12==0:
            frames.append(frame)
        n += 1
    if not frames:
        raise RuntimeError(f'Cannot read {clip}')
    return np.median(np.stack(frames), axis=0).astype(np.uint8), len(frames)


def evened(image):
    """Grey with the contrast evened out, so a dark post against a dark hedge at
    night counts as an edge as much as a white line on the road does."""
    gray = cv2.cvtColor(cv2.bilateralFilter(image, 9, 40, 7), cv2.COLOR_BGR2GRAY)
    return cv2.createCLAHE(clipLimit=3., tileGridSize=(8,8)).apply(gray)


def contours(image, shortest):
    """The view as thin lines, one pixel wide, wherever it has an edge that runs
    on for a while. Speckle (fallen leaves, foliage) is too short to count."""
    edges = cv2.Canny(cv2.GaussianBlur(evened(image), (0,0), 1.4), 30, 85, L2gradient=True)
    count, labels, stats, _ = cv2.connectedComponentsWithStats(edges, connectivity=8)
    return np.isin(labels, np.flatnonzero(stats[:,cv2.CC_STAT_AREA]>=shortest)[1:])


def matched(other, plate):
    """How another view of the same place, in other light and perhaps nudged or
    zoomed, maps onto the plate, and on how many points the two agree. None when
    they cannot be matched."""
    finder = cv2.SIFT_create(8000)
    (ka, da), (kb, db) = finder.detectAndCompute(evened(other), None), finder.detectAndCompute(evened(plate), None)
    good = [m for m, n in cv2.BFMatcher().knnMatch(da, db, k=2) if m.distance<.8*n.distance]
    if len(good)<20:
        return None
    turn, agreed = cv2.findHomography(np.float32([ka[m.queryIdx].pt for m in good]),
                                      np.float32([kb[m.trainIdx].pt for m in good]), cv2.RANSAC, 5.)
    if turn is None or agreed.sum()<20:
        return None
    return turn, int(agreed.sum())


def laid_over(other, plate):
    """Another view of the same place warped to sit exactly over the plate. None
    when the two cannot be matched."""
    match = matched(other, plate)
    if match is None:
        return None
    turn, agreed = match
    size = (plate.shape[1], plate.shape[0])
    covered = cv2.warpPerspective(np.full(other.shape[:2], 255, np.uint8), turn, size)
    return cv2.warpPerspective(other, turn, size), covered>250, agreed


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('clip')
    ap.add_argument('--daylight', help='a clip of the same place in better light, to trace unseen lines from')
    ap.add_argument('--scene', type=pathlib.Path, default=pathlib.Path(__file__).resolve().parent.parent/'scene')
    args = ap.parse_args()
    args.scene.mkdir(parents=True, exist_ok=True)
    plate, count = still(args.clip)
    height, width = plate.shape[:2]
    cv2.imwrite(str(args.scene/'plate.jpg'), plate, [cv2.IMWRITE_JPEG_QUALITY, 92])
    shortest = round(45*width/1920)
    lines = contours(plate, shortest)
    note = ''
    (args.scene/'plate-day.jpg').unlink(missing_ok=True)
    if args.daylight:
        match = laid_over(still(args.daylight)[0], plate)
        if match is None:
            note = '; the daylight clip could not be matched to this view'
        else:
            day, covered, agreed = match
            cv2.imwrite(str(args.scene/'plate-day.jpg'), day, [cv2.IMWRITE_JPEG_QUALITY, 92])
            # Not the rim of the warped picture itself, which is no line of the scene.
            inside = cv2.erode(covered.astype(np.uint8), np.ones((15,15), np.uint8))>0
            lines |= contours(day, shortest) & inside
            note = f'; daylight view laid over it on {agreed} matching points'
    # Every contour pulls alike, so a stroke goes to the nearest one, not the loudest.
    # A slight blur gives each a slope for a stroke to slide down onto it.
    cv2.imwrite(str(args.scene/'edges.png'), cv2.GaussianBlur(lines.astype(np.float32)*255, (0,0), 1.6).clip(0, 255).astype(np.uint8)*2)
    print(f'{args.scene}: plate {width}x{height} from {count} frames, contours on {lines.mean()*100:.1f}% of it{note}')


if __name__=='__main__':
    main()
