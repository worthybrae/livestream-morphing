#!/usr/bin/env python3
"""Recorded feasibility study. Persistent marks + low-resolution optical flow.

No production endpoint/configuration is changed. OpenCV estimates motion; a
small native helper reuses each mark's brush mask and paints an opaque canvas.
"""
import argparse
import json
import pathlib
import struct
import subprocess
import time

import cv2
import numpy as np
from motion import transport


def sample(image, points):
    if not len(points):
        return np.empty((0, 1 if image.ndim==2 else image.shape[2]), np.float32)
    if len(points)<=16384:
        return cv2.remap(image, points[:, 0:1].astype(np.float32), points[:, 1:2].astype(np.float32),
                         cv2.INTER_LINEAR, borderMode=cv2.BORDER_REPLICATE).reshape(len(points), -1)
    # OpenCV uses signed 16-bit destination coordinates internally.
    return np.concatenate([cv2.remap(image, part[:, 0:1].astype(np.float32),
                     part[:, 1:2].astype(np.float32), cv2.INTER_LINEAR,
                     borderMode=cv2.BORDER_REPLICATE).reshape(len(part), -1)
                     for part in np.array_split(points, max(1,(len(points)+16383)//16384))])


def palette(hour):
    smooth = lambda x: np.clip(x, 0, 1)**2*(3-2*np.clip(x, 0, 1))
    night = 1-smooth((hour-4.5)/3.5)+smooth((hour-18)/3)
    stops = [(0, [-13,-5,27]), (4, [-13,-5,27]), (6.5, [32,3,12]),
             (10, [15,12,-9]), (15, [15,12,-9]), (18.5, [37,1,-16]),
             (21, [8,-14,30]), (24, [-13,-5,27])]
    for (a, ca), (b, cb) in zip(stops, stops[1:]):
        if hour < b:
            t = smooth((hour-a)/(b-a))
            tint = np.array(ca)*(1-t)+np.array(cb)*t
            break
    return night,tint


def grade(source, hour):
    """Same day/night color anchors as the approved fine knife edition."""
    night,tint = palette(hour)
    rgb = source.astype(np.float32)
    gray = rgb[:,:,0]*.299+rgb[:,:,1]*.587+rgb[:,:,2]*.114
    peak = rgb.max(axis=2)
    excess = np.maximum(peak-150, 0)
    compressed = peak-excess+excess/(1+excess/95)
    gain = 1+night*(compressed/np.maximum(peak, 1)-1)
    weight = 1-night*np.clip((peak-160)/95, 0, 1)*.85
    result = ((gray[..., None]+(rgb-gray[..., None])*(1.4-night*.98)) *
              gain[..., None]*(1.04-night*.42)+
              tint*weight[..., None]*(.75-night*.68))
    return np.clip(result, 0, 255).astype(np.uint8)


def grade_reference(source, hour):
    """Native color transform for the adaptive pass's full-resolution target.

    Same palette math, within one 8-bit level of grade's float64 path.
    """
    night,tint = palette(hour)
    rgb=source.astype(np.float32)
    saturation=1.4-night*.98
    matrix=np.eye(3,dtype=np.float32)*saturation+(1-saturation)*np.array([[.299,.587,.114]],np.float32)
    result=cv2.transform(rgb,matrix)*(1.04-night*.42)
    if night>0:
        r,g,b=cv2.split(rgb);peak=cv2.max(cv2.max(r,g),b)
        excess=np.maximum(peak-150,0)
        compressed=peak-excess+excess/(1+excess/95)
        gain=1+night*(compressed/np.maximum(peak,1)-1)
        result*=gain[...,None]
        weight=1-night*np.clip((peak-160)/95,0,1)*.85
        result+=weight[...,None]*(tint*(.75-night*.68)).astype(np.float32)
    else:
        result+=(tint*.75).astype(np.float32)
    return np.clip(result,0,255).astype(np.uint8)


# A line drawn with the point of the brush is a darker accent of the colour it crosses.
LINE_TONE = .86
# A long contour line is laid in bolder than the short drawn ones.
CONTOUR_TONE = .6
# Coat codes beyond the approved coats. adaptive.py owns 4, 5 and 6.
PULL, DRAWN, CONTOUR, RIDGE = 1.5, 8, 9, 10
LONG = (PULL, CONTOUR, RIDGE)
# Ways of working that lay long strokes on the still scene.
LINE_HANDS = ('gesture', 'contour', 'border')
BEND = .4


def bends(seeds):
    """Bow and S-curve of a long stroke, carried in the high bits of its seed.

    The native rasteriser unpacks the same fourteen bits (material.rs `bends`).
    """
    code = np.floor(seeds*16384)
    return (code//128-64)/63*BEND, (code%128-64)/63*BEND


def along(marks, u):
    """Points on long strokes at chord position u, from -1 (start) to 1 (end)."""
    bow, snake = bends(marks[:,6])
    cs, sn = np.cos(marks[:,5]), np.sin(marks[:,5])
    reach, offset = marks[:,3]*u, marks[:,3]*(1-u*u)*(bow+snake*u)
    return marks[:,1:3]+np.stack([cs*reach-sn*offset, sn*reach+cs*offset], axis=1)


def trace(start, heading, alive, step, count, settle=None, turn=.9):
    """Walk both ways from each start along a direction field.

    heading(points) gives unit tangents of either sign, alive(points) says which
    walks may go on, settle(points, direction) may pull a walker back onto its
    edge. Returns the walked points [n, 2*count+1, 2] and which were reached.
    """
    n = len(start)
    path = np.repeat(start[:,None,:], 2*count+1, axis=1).astype(np.float32)
    valid = np.zeros((n, 2*count+1), bool); valid[:,count] = True
    first = heading(start)
    for sign in (1, -1):
        at, direction, going = start.astype(np.float32).copy(), first*sign, np.ones(n, bool)
        for k in range(1, count+1):
            at = at+direction*step
            if settle is not None:
                at = settle(at, direction)
            ahead = heading(at)
            ahead = ahead*np.where((ahead*direction).sum(axis=1, keepdims=True)<0, -1, 1)
            going &= alive(at) & ((ahead*direction).sum(axis=1)>turn) & ((ahead*first).sum(axis=1)*sign>.3)
            if not going.any():
                break
            direction = np.where(going[:,None], ahead, direction)
            path[going, count+sign*k], valid[going, count+sign*k] = at[going], True
    return path, valid


def fit(path, valid, flip, tolerance, most=BEND):
    """Chord, bow and S-curve that follow each walked path, shortened until they do.

    A path that needs more bend than `most` counts as not followed.

    Rows are [x, y, half-length, angle, bow, snake, miss]; miss is how far, in
    pixels, the walked points sit from the fitted stroke.
    """
    count = path.shape[1]; middle = count//2
    index = np.arange(count); rows = np.arange(len(path))
    result = None
    for share in (1., .6, .35):
        part = valid & (np.abs(index-middle)<=max(2, round(middle*share)))
        a = np.where(part, index, count).min(axis=1); b = np.where(part, index, -1).max(axis=1)
        A, B = path[rows,a], path[rows,b]
        A, B = np.where(flip[:,None], B, A), np.where(flip[:,None], A, B)
        centre, half = (A+B)/2, np.maximum(np.linalg.norm(B-A, axis=1)/2, 1e-3)
        tangent = (B-A)/(2*half[:,None])
        offset = path-centre[:,None,:]
        u = np.clip((offset*tangent[:,None,:]).sum(axis=2)/half[:,None], -1, 1)
        d = (offset[:,:,1]*tangent[:,None,0]-offset[:,:,0]*tangent[:,None,1])/half[:,None]
        f1, f2 = (1-u*u)*part, u*(1-u*u)*part
        s11, s12, s22 = (f1*f1).sum(1)+1e-4, (f1*f2).sum(1), (f2*f2).sum(1)+1e-4
        b1, b2 = (f1*d).sum(1), (f2*d).sum(1)
        det = s11*s22-s12*s12
        bow = np.clip((b1*s22-b2*s12)/det, -BEND, BEND)
        snake = np.clip((b2*s11-b1*s12)/det, -BEND, BEND)
        miss = np.sqrt((((d-(1-u*u)*(bow[:,None]+snake[:,None]*u))*part)**2).sum(1)/np.maximum(part.sum(1),1))*half
        miss = np.where((np.abs(bow)>most) | (np.abs(snake)>most), np.inf, miss)
        found = np.stack([centre[:,0], centre[:,1], half, np.arctan2(tangent[:,1], tangent[:,0]), bow, snake, miss], axis=1)
        result = found if result is None else np.where((result[:,6]>tolerance)[:,None], found, result)
    return result


def evenly(run, step):
    """A run of points re-stepped evenly along its length."""
    run = np.asarray(run, np.float32)
    gone = np.concatenate([[0], np.cumsum(np.linalg.norm(np.diff(run, axis=0), axis=1))])
    at = np.linspace(0, gone[-1], max(2, int(round(gone[-1]/step))+1))
    return np.stack([np.interp(at, gone, run[:,0]), np.interp(at, gone, run[:,1])], axis=1).astype(np.float32)


def followed(run, tolerance, most=.3):
    """Strokes that follow a run of evenly stepped points from end to end: one
    where a single bowed stroke can, else one for each half, and so on down.
    Rows as from fit()."""
    found = fit(run[None], np.ones((1, len(run)), bool), np.zeros(1, bool), np.inf, most)[0]
    if found[6]<=tolerance or len(run)<6:
        return [found]
    half = len(run)//2
    return followed(run[:half+1], tolerance, most)+followed(run[half:], tolerance, most)


def cut(run, closed, longest, corner=.6):
    """A run of evenly stepped points cut into stretches one stroke might follow:
    at its corners, and wherever a stretch would grow longer than `longest` points."""
    n = len(run)
    if n<5:
        return [run]
    before, after = (np.roll(run, 2, axis=0), np.roll(run, -2, axis=0)) if closed else (
        np.concatenate([run[:1], run[:1], run[:-2]]), np.concatenate([run[2:], run[-1:], run[-1:]]))
    a, b = run-before, after-run
    turn = np.abs(np.arctan2(a[:,0]*b[:,1]-a[:,1]*b[:,0], (a*b).sum(axis=1)))
    if not closed:
        turn[[0,1,-2,-1]] = 0
    else:
        # Begin at the sharpest corner, so that no stretch straddles it.
        first = int(np.argmax(turn))
        run, turn = np.roll(run, -first, axis=0), np.roll(turn, -first)
        run, turn = np.concatenate([run, run[:1]]), np.append(turn, 0)
    sharp = (turn>corner) & (turn>=np.roll(turn, 1)) & (turn>np.roll(turn, -1))
    breaks = [0]+[int(i) for i in np.flatnonzero(sharp) if 2<=i<len(run)-2]+[len(run)-1]
    stretches = []
    for begun, ended in zip(breaks[:-1], breaks[1:]):
        pieces = max(1, int(np.ceil((ended-begun)/longest)))
        marks = np.round(np.linspace(begun, ended, pieces+1)).astype(int)
        stretches += [run[i:j+1] for i, j in zip(marks[:-1], marks[1:]) if j>i]
    return stretches


def spine(ring, within):
    """The middle line of a slim closed shape, tip to tip, evenly stepped, and half
    the shape's breadth at each of its points. `within` gives the distance to the
    shape's edge."""
    apart = np.linalg.norm(ring[:,None]-ring[None], axis=2)
    i, j = sorted(np.unravel_index(int(apart.argmax()), apart.shape))
    one, other = ring[i:j+1], np.concatenate([ring[j:], ring[:i+1]])[::-1]
    count = max(len(one), len(other))
    def stepped(side):
        gone = np.concatenate([[0], np.cumsum(np.linalg.norm(np.diff(side, axis=0), axis=1))])
        at = np.linspace(0, gone[-1], count)
        return np.stack([np.interp(at, gone, side[:,0]), np.interp(at, gone, side[:,1])], axis=1)
    middle = evenly((stepped(one)+stepped(other))/2, 6)
    breadth = within(middle)
    # Not out into the tips, where a stroke as broad as the shape would overshoot.
    body = np.flatnonzero(breadth>=.5*np.median(breadth))
    return middle[body[0]:body[-1]+1], breadth[body[0]:body[-1]+1]


def picked_strokes(rings, size, scale, slim=18., longest=110.):
    """Strokes for shapes picked out by hand on the still scene.

    A slim shape (a road marking, a kerb line, a rail) becomes a line down its
    middle, as broad as the shape. Any other becomes a chain of strokes round its
    outline, all running the same way round it. Returns rows as from fit(); for
    each, the breadth of its shape (of the stroke itself, for a slim one), whether
    it is an outline stroke, and which shape it belongs to; a picture of which
    shape lies where (0: none, else its number plus one); and which shapes are slim.
    """
    width, height = size
    rows, breadths, borders, owner = [], [], [], []
    where, slender = np.zeros((height, width), np.int32), np.zeros(len(rings), bool)
    most = max(3, int(round(longest*scale/6)))
    kept = []
    for index, ring in enumerate(rings):
        ring = np.asarray(ring, np.float32)
        if len(ring)>1 and np.allclose(ring[0], ring[-1]):
            ring = ring[:-1]
        if len(ring)<3:
            continue
        x0, y0 = np.floor(ring.min(axis=0)).astype(int)-2
        x1, y1 = np.ceil(ring.max(axis=0)).astype(int)+3
        patch = np.zeros((y1-y0, x1-x0), np.uint8)
        cv2.fillPoly(patch, [np.round(ring-[x0,y0]).astype(np.int32)], 1)
        depth = cv2.distanceTransform(patch, cv2.DIST_L2, 3)
        def within(points, x0=x0, y0=y0, depth=depth):
            at = np.clip(np.round(points-[x0,y0]).astype(int), 0, [depth.shape[1]-1, depth.shape[0]-1])
            return depth[at[:,1], at[:,0]]
        middle, half = spine(evenly(np.concatenate([ring, ring[:1]]), 3)[:-1], within)
        # As broad as it mostly is: a long kerb line is slim even where it swells at a
        # corner. A pole with a crown of leaves is not: most of it is slim, the rest far from it.
        thick = 2*float(np.median(half))
        kept.append((thick, thick<slim*scale and 2*np.percentile(half, 90)<2.2*thick+2*scale, index, ring, middle, half))
    # The broadest first, so that a slim shape lying on a broad one is not lost under it.
    for thick, line, index, ring, middle, half in sorted(kept, key=lambda shape: -shape[0]):
        cv2.fillPoly(where, [np.round(ring).astype(np.int32)], index+1)
        if line:
            slender[index] = True
            for stretch in cut(middle, False, most):
                for found in followed(stretch, 1.5*scale):
                    near = np.linalg.norm(middle-found[:2], axis=1)<=found[2]+3
                    rows.append(found); breadths.append(max(1.2*scale, float(half[near].mean()) if near.any() else 0)); borders.append(False); owner.append(index)
        else:
            around = evenly(np.concatenate([ring, ring[:1]]), 6)[:-1]
            after = np.roll(around, -1, axis=0)
            if (around[:,0]*after[:,1]-after[:,0]*around[:,1]).sum()>0:
                around = around[::-1]  # the same way round every shape, so one side of a stroke is always the shape's
            for stretch in cut(around, True, most):
                for found in followed(stretch, 2.2*scale):
                    rows.append(found); breadths.append(thick); borders.append(True); owner.append(index)
    rows = np.array(rows, np.float32).reshape(-1, 7)
    return rows, np.array(breadths, np.float32), np.array(borders, bool), np.array(owner, int), where, slender


class Piles:
    """A painter's mixed piles: the few tones the whole picture is built from.

    Tone is mixed in a handful of steps taken from the first frame. Hue and
    strength snap to a coarse grid, so a muted green coat stays green while the
    smooth photographic shading inside each passage is gone. A live version
    would have to remix the tone steps slowly as the light changes.
    """
    def __init__(self, image, count):
        small = cv2.resize(image, (384, round(384*image.shape[0]/image.shape[1])), interpolation=cv2.INTER_AREA)
        tone = cv2.cvtColor(small.astype(np.float32)/255, cv2.COLOR_RGB2Lab)[:,:,0].ravel()
        rng = np.random.default_rng(11)
        # Half the samples as they come, half spread evenly over the tonal range:
        # the big passages get a step of their own, and so do the rare darks and lights.
        bands = [tone[(tone>=band*10) & (tone<band*10+10)] for band in range(10)]
        even = np.concatenate([band[rng.choice(len(band), min(1500, len(band)), replace=False)] for band in bands if len(band)>=20])
        samples = np.concatenate([even, rng.choice(tone, len(even))]).astype(np.float32)[:,None]
        cv2.setRNGSeed(7)
        steps = np.sort(cv2.kmeans(samples, count, None, (cv2.TERM_CRITERIA_EPS+cv2.TERM_CRITERIA_MAX_ITER, 30, .1),
                                   3, cv2.KMEANS_PP_CENTERS)[2].ravel())
        # A staircase with short ramps: flat on each step, blended only where two are a toss-up.
        middle, gap = (steps[1:]+steps[:-1])/2, np.diff(steps)*.14
        self.middle = middle
        self.ramp_from = np.sort(np.concatenate([middle-gap, middle+gap]))
        self.ramp_to = np.repeat(steps, 2)[1:-1]
        self.grid = 60/count
        levels = (np.arange(32, dtype=np.float32)+.5)*8
        cube = np.stack(np.meshgrid(levels, levels, levels, indexing='ij'), axis=3).reshape(-1,3)
        self.table = np.clip(self.mix(cube), 0, 255).astype(np.uint8)

    @staticmethod
    def lean(seeds, colours):
        """Neighbouring strokes of one pile lean a little warm or cool, light or dark.
        Less so in the dark, where the same lean would shout."""
        warm, value = ((seeds*37.31)%1-.5)[:,None]*2, ((seeds*71.93)%1-.5)[:,None]*2
        return (warm*np.array([7,1,-7], np.float32)+value*5)*np.clip(colours.max(axis=1)/90, .25, 1)[:,None]

    def mix(self, colours, seeds=None):
        """Each colour as the pile a painter would reach for."""
        if not len(colours):
            return np.empty((0,3), np.float32)
        lab = cv2.cvtColor(colours.astype(np.float32)[None]/255, cv2.COLOR_RGB2Lab)[0]
        lab[:,0] = np.interp(lab[:,0], self.ramp_from, self.ramp_to)
        cell = lab[:,1:]/self.grid
        near = np.round(cell)
        blend = np.clip((np.abs(cell-near)-.36)/.14, 0, 1)
        snapped = (near+np.sign(cell-near)*blend*blend*(3-2*blend)*.5)*self.grid
        # A grey has no hue worth rounding: snapping it splits one quiet passage,
        # a road at night, into warm patches and cool ones.
        strong = np.clip((np.linalg.norm(lab[:,1:], axis=1)-10)/15, 0, 1)[:,None]
        lab[:,1:] += (snapped-lab[:,1:])*strong
        mixed = cv2.cvtColor(lab[None], cv2.COLOR_Lab2RGB)[0]*255
        return mixed if seeds is None else mixed+self.lean(seeds, mixed)

    def repaint(self, image):
        """A whole image in pile colours, for the finishing pass to measure against."""
        image = image.astype(np.uint8, copy=False)
        index = (image[:,:,0]>>3).astype(np.uint16)<<10 | (image[:,:,1]>>3).astype(np.uint16)<<5 | image[:,:,2]>>3
        return self.table.take(index, axis=0)


class Painting:
    def __init__(self, width, height, flow_size, mode, detail='original', detail_hour=None, limit=45000, hand='approved', piles=0, drift=1., stir=0., ripple=1., border_tone='dark', pulse=False,
                 border_weight=1., border_depth=1., ripple_everywhere=0., stand_out=20, picked=None, slim=18., swell=.85, keen=1., reserve=0, rate=15., pace=1):
        self.w, self.h = width, height
        self.fw, self.fh = flow_size
        self.factor = np.array([self.fw/width, self.fh/height], np.float32)
        self.mode = mode
        self.detail = detail
        self.detail_hour = detail_hour
        self.limit = limit
        self.hand = hand
        self.pile_count, self.piles = piles, None
        self.unrest = None
        self.turn = 0
        self.drift, self.stir = drift, stir
        self.stirred = None
        self.towed = None
        self.ripple, self.border_tone = ripple, border_tone
        self.border_weight, self.border_depth, self.ripple_everywhere = border_weight, border_depth, ripple_everywhere
        # Lines that stay put and swell and thin, instead of swaying and pulling.
        self.pulse = pulse or hand=='border'
        self.thing_mask = self.held = self.votes = None
        self.raggedness = 60  # outline length squared over area: 16 for a square, far more for foliage
        self.shapes = None
        self.stand_out = stand_out  # how far a thing's colour must be from its surroundings
        self.thing_counts = []
        # Shapes picked out by hand on the still scene (closed runs of points, in
        # painting pixels). With these, lines are laid on them and nowhere else.
        self.picked, self.slim = picked, slim
        self.swell = swell  # the most a pulsing line may swell and thin, as a share of its width
        self.keen = keen  # how readily a faint edge of a thing is given a line
        # Room kept free of ordinary marks for long strokes, so that a population
        # running at its limit, as the live one does, can still take a line.
        self.reserve = reserve
        self.rate = rate  # updates a second: what turns the count of updates into time
        # How many updates pass between looks at the still scene: its edges, its
        # things, where a new line could go. 1, the study's pace, looks at every
        # update. A scene that stands still need not be looked at so often; what is
        # crossing a line is still asked at every update.
        self.pace = max(1, int(pace))
        self.doubt = np.empty(0, bool)  # for each stroke, whether the last look found its footing gone
        self.solid = self.off_outline = None
        self.picked_ids, self.picked_owner = (0, 0), None
        self.where = self.line_mask = self.present = self.give = None
        self.showing = []  # picked lines up at each update
        self.rippling = []  # share of the picture whose paint is shifted by a pixel or more, at each update
        if picked is not None and hand not in LINE_HANDS:
            raise ValueError('Picked shapes need a hand that draws lines')
        self.shown = []  # drawn lines clearly visible at each update
        self.laid = [0, 0]  # long strokes laid on the first frame, and since
        self.scale = width/1920
        self.steps = np.array([32., 17., 6.])*self.scale
        if detail != 'original':
            self.steps = np.append(self.steps, (2.8 if detail=='people' else 5.5)*self.scale)
        self.strokes = np.empty((0, 12), np.float32)
        self.failures = np.empty(0, np.float32)
        self.age = np.empty(0, np.int32)
        self.next_id = 1
        self.candidates = []
        self.variants = []
        self.births, self.retired = 0, 0
        self.ground = None
        self.previous_gray = None
        rng = np.random.default_rng(8341)
        for step in self.steps:
            x, y = np.meshgrid(np.arange(step*.3, width, step),
                               np.arange(step*.3, height, step))
            pts = np.stack([x.ravel(), y.ravel()], axis=1)
            pts += rng.uniform(-.32, .32, pts.shape)*step
            pts = np.clip(pts, [1,1], [width-2,height-2]).astype(np.float32)
            variants = rng.random((len(pts), 4)).astype(np.float32)
            order = rng.permutation(len(pts))
            self.candidates.append(pts[order])
            self.variants.append(variants[order])
        if hand in ('lines', 'full')+LINE_HANDS:
            # Sites where a thin drawn line may start. A separate generator keeps
            # the approved coats' candidates exactly as they were.
            rng = np.random.default_rng(5519)
            step = 10*self.scale
            x, y = np.meshgrid(np.arange(step*.3, width, step), np.arange(step*.3, height, step))
            pts = np.stack([x.ravel(), y.ravel()], axis=1)+rng.uniform(-.4, .4, (x.size, 2))*step
            order = rng.permutation(len(pts))
            self.line_candidates = np.clip(pts, [1,1], [width-2,height-2]).astype(np.float32)[order]
            self.line_variants = rng.random((len(pts), 4)).astype(np.float32)[order]
        if hand in LINE_HANDS:
            # Sites where a long pull may start, coarser than the line sites.
            step = 24*self.scale
            x, y = np.meshgrid(np.arange(step*.3, width, step), np.arange(step*.3, height, step))
            pts = np.stack([x.ravel(), y.ravel()], axis=1)+rng.uniform(-.4, .4, (x.size, 2))*step
            order = rng.permutation(len(pts))
            self.pull_candidates = np.clip(pts, [1,1], [width-2,height-2]).astype(np.float32)[order]
            self.pull_variants = rng.random((len(pts), 4)).astype(np.float32)[order]

    def things(self, image):
        """The distinct, compact things in the picture, as a mask on the analysis image
        and the distance from their outlines. A thing is a patch of one mixed colour
        that is neither thin nor vast and stands out from what surrounds it: a lamp
        post, a pillar, a sign, a parked car, someone standing. Not the road, a line
        painted on it, or a shadow lying across it."""
        if self.shapes is None:
            self.shapes = self.piles or Piles(image, 6)
        unit = image.shape[1]/960
        # Hard steps of tone and broad bands of hue: here the soft blends between piles
        # would only splinter every passage into slivers.
        lab = cv2.cvtColor(cv2.medianBlur(image, 2*round(3*unit)+1).astype(np.float32)/255, cv2.COLOR_RGB2Lab)
        hue = np.round(lab[:,:,1:]/18).astype(np.int32)+8
        key = np.searchsorted(self.shapes.middle, lab[:,:,0]).astype(np.int32)<<16 | hue[:,:,0]<<8 | hue[:,:,1]
        seam = np.zeros(key.shape, bool)
        seam[:,:-1] |= key[:,1:]!=key[:,:-1]
        seam[:-1] |= key[1:]!=key[:-1]
        open_ = (~seam).astype(np.uint8)
        count, labels, stats, _ = cv2.connectedComponentsWithStats(open_, connectivity=4)
        area = stats[:,cv2.CC_STAT_AREA]
        # Enough of the patch must lie well inside it: that rules out lines and slivers.
        body = np.bincount(labels.ravel(), weights=(cv2.distanceTransform(open_, cv2.DIST_L2, 3)>=5*unit).ravel(), minlength=count)
        pad = round(10*unit)
        ring = np.ones((2*pad+1,2*pad+1), np.uint8)
        chosen = []
        for i in np.flatnonzero((area>120*unit*unit) & (area<.06*key.size) & (body>=20*unit*unit))[1:]:
            x, y, w, h = stats[i,:4]
            if area[i]<.42*w*h:
                continue  # ragged or strung out: foliage, a kerb, a shadow's edge
            x0, y0, x1, y1 = max(0,x-pad), max(0,y-pad), min(key.shape[1],x+w+pad), min(key.shape[0],y+h+pad)
            inside = (labels[y0:y1,x0:x1]==i).astype(np.uint8)
            if self.hand=='border':
                rim = int((inside-cv2.erode(inside, np.ones((3,3), np.uint8))).sum())
                if rim*rim>self.raggedness*area[i] or area[i]<300*unit*unit:
                    continue  # a border only goes round a clean, sizeable shape
            around = cv2.dilate(inside, ring)-inside
            patch = image[y0:y1,x0:x1]
            if np.abs(patch[inside>0].mean(axis=0)-patch[around>0].mean(axis=0)).max()>self.stand_out+(6 if self.hand=='border' else 0):
                chosen.append(i)
        self.thing_counts.append(len(chosen))
        solid = cv2.dilate(np.isin(labels, chosen).astype(np.uint8), np.ones((3,3), np.uint8))
        outline = cv2.morphologyEx(solid, cv2.MORPH_GRADIENT, np.ones((3,3), np.uint8))
        return solid.astype(np.float32), cv2.distanceTransform(1-outline, cv2.DIST_L2, 3)

    def outlines(self, solid, analysis_scale):
        """Each thing's outline cut into stretches one stroke can follow: evenly
        stepped, lightly smoothed, and broken wherever it turns a corner."""
        reach, stretches = 8, []
        chains, _ = cv2.findContours(solid.astype(np.uint8), cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
        for chain in chains:
            ring = chain[::4,0,:].astype(np.float32)/analysis_scale
            if len(ring)<8:
                continue
            ring = (np.roll(ring, 1, axis=0)+2*ring+np.roll(ring, -1, axis=0))/4
            step = np.roll(ring, -1, axis=0)-ring
            heading = np.arctan2(step[:,1], step[:,0])
            turn = np.abs((heading-np.roll(heading, 1)+np.pi)%(2*np.pi)-np.pi)
            # Begin at the sharpest corner, so that no stretch straddles it.
            order = (np.arange(len(ring)+1)+int(np.argmax(turn)))%len(ring)
            begun = 0
            for k in range(1, len(order)):
                if k-begun>=2*reach or turn[order[k]]>.5 or k==len(order)-1:
                    if k-begun>=3:
                        stretches.append(ring[order[begun:k+1]])
                    begun = k
        path = np.zeros((len(stretches), 2*reach+1, 2), np.float32)
        valid = np.zeros(path.shape[:2], bool)
        for i, stretch in enumerate(stretches):
            a = reach-len(stretch)//2
            path[i,a:a+len(stretch)], valid[i,a:a+len(stretch)] = stretch, True
        return path, valid

    def swim(self):
        """The paint inside each still thing drifts in slow ripples while its border
        holds: the thing distorts without leaving its place. Returns the shift in
        pixels on the motion grid, or None when nothing moves."""
        if not (self.drift and self.ripple) or self.thing_mask is None:
            return None
        # Nothing at the border itself, most well inside; not on anything in motion.
        hold = cv2.GaussianBlur(cv2.erode(self.thing_mask, np.ones((5,5), np.uint8)), (0,0), 1.5)
        if self.picked is not None:
            # A slim line has no inside to ripple: the line and the paint beside it
            # sway a little instead. Nothing ripples while its shape is hidden.
            hold = np.maximum(hold, self.line_mask)*self.present
        # Passers-by stir the things, not the ground between them: that part of the
        # picture, when it is allowed to swim at all, only ever swims gently.
        lively = np.full(hold.shape, self.drift, np.float32)
        if self.stir and self.stirred is not None:
            lively *= 1+self.stir*np.clip(self.stirred/6, 0, 1)*hold
        hold = np.maximum(hold, self.ripple_everywhere)
        if self.unrest is not None:
            hold = hold*np.clip(1-self.unrest/6, 0, 1)
        # Eased in and out, so a thing never jumps when something passes it.
        self.held = hold if self.held is None else self.held*.75+hold*.25
        seconds = self.turn/self.rate
        y, x = np.mgrid[0:self.fh,0:self.fw].astype(np.float32)
        wave = np.stack([np.sin(2*np.pi*seconds/5+y*.16+x*.05), np.cos(2*np.pi*seconds/6.3+x*.14-y*.04)], axis=2)
        # However it is driven, no patch of paint travels far enough to tear.
        far = np.minimum(self.held*lively*2.2*self.scale*self.ripple, 26*self.scale)
        if self.picked is not None:
            # Nor so far that a narrow thing is bent out of its own shape.
            far = np.minimum(far, self.give)
        self.rippling.append(float((far>=self.scale).mean()))
        return wave*far[:,:,None]

    def lively(self, lines):
        """Phase and reach of each drawn line's motion: one slow wave crossing the
        picture, livelier where people and cars have just passed."""
        phase = 2*np.pi*self.turn/self.rate/7+(lines[:,1]*.004+lines[:,2]*.006)/self.scale
        reach = np.full(len(lines), self.drift, np.float32)
        if self.stir and self.stirred is not None:
            reach *= 1+self.stir*np.clip(sample(self.stirred, lines[:,1:3]*self.factor)[:,0]/6, 0, 1)
        return phase, reach

    def tow(self, marks):
        """Where each drawn line sways and flexes to, spread into the paint around it:
        a line does not slide over the thing it outlines, it takes the thing along.
        Returns the shift in pixels on the motion grid, or None when nothing moves."""
        lines = marks[marks[:,11]>=CONTOUR]
        if not self.drift or not len(lines):
            return None
        phase, reach = self.lively(lines)
        sway = 3*self.scale*reach*np.sin(phase)
        bow, snake = .035*reach*np.sin(phase+1.3), .03*reach*np.sin(phase+2.6)
        across = np.stack([-np.sin(lines[:,5]), np.cos(lines[:,5])], axis=1)
        total = np.zeros((self.fh,self.fw,2), np.float32)
        weight = np.zeros((self.fh,self.fw), np.float32)
        # A fading line lets go of its surroundings as gently as it fades.
        share = lines[:,10]*lines[:,3]*self.factor[0]/8
        for u in np.linspace(-1, 1, 17):
            at = np.clip(np.round(along(lines, u)*self.factor), 0, [self.fw-1,self.fh-1]).astype(int)
            push = sway+lines[:,3]*(1-u*u)*(bow+snake*u)
            np.add.at(weight, (at[:,1],at[:,0]), share)
            np.add.at(total, (at[:,1],at[:,0]), across*(push*share)[:,None])
        spread = 7.
        weight, total = cv2.GaussianBlur(weight, (0,0), spread), cv2.GaussianBlur(total, (0,0), spread)
        return total/(weight[:,:,None]+.25/(spread*2.5066))

    def adrift(self, marks):
        """The weight of each drawn line travels along it. The rasteriser keeps one
        brush mask per identity, so such a line needs a fresh identity each update;
        its coat carries the phase of the travelling weight."""
        lines = marks[:,11]>=CONTOUR
        if not self.drift or not lines.any():
            return marks
        marks = marks.copy()
        # Above every other identity, the live finishing marks' 2,000,000 to 8,000,000 included.
        marks[lines,0] = 9_000_000+2*marks[lines,0]+self.turn%2
        if self.pulse:
            # The line stays where it is and swells and thins instead. For a border, the
            # upper half of the coat's fraction tells the rasteriser to draw it as one.
            phase, reach = self.lively(marks[lines])
            marks[lines,4] *= 1+np.clip(.3*reach, 0, self.swell)*np.sin(phase)
            marks[lines,11] += .5*(self.hand=='border')
        else:
            marks[lines,11] += (self.turn/self.rate/5%1)/2
        return marks

    def mixed(self, colours, seeds):
        return colours if self.piles is None else self.piles.mix(colours, seeds)

    def fields(self, gray, brush_scale):
        """Edge strength, edge firmness, and the two direction fields long strokes follow."""
        def tensor(blur, spread):
            soft = cv2.GaussianBlur(gray, (0,0), blur)
            gx = cv2.Sobel(soft, cv2.CV_32F, 1, 0, ksize=3)*.125
            gy = cv2.Sobel(soft, cv2.CV_32F, 0, 1, ksize=3)*.125
            return cv2.GaussianBlur(np.stack([gx*gx, gy*gy, gx*gy], axis=2), (0,0), spread)
        def way(t):
            # The tangent as a doubled angle: opposite directions average, not cancel.
            across, lengthwise = t[:,:,0]-t[:,:,1], 2*t[:,:,2]
            size = np.sqrt(across**2+lengthwise**2)
            return np.stack([-across, -lengthwise], axis=2)/(size[:,:,None]+1e-3), size/(t[:,:,0]+t[:,:,1]+1)
        edge = tensor(1.1*brush_scale, 3.5*brush_scale)
        edge_way, firm = way(edge)
        # Broad passages take their lie from the big shapes around them. Only the
        # long pulls of the gesture hand follow it, and it is the costliest field.
        form_way = way(tensor(3*brush_scale, 22*brush_scale))[0] if self.hand=='gesture' else None
        # Thin things lighter or darker than what surrounds them: road markings, poles.
        lift = cv2.GaussianBlur(gray, (0,0), 2*brush_scale)-cv2.GaussianBlur(gray, (0,0), 8*brush_scale)
        return np.sqrt(edge[:,:,0]+edge[:,:,1]), firm, edge_way, form_way, lift

    def transport_only(self, forward, backward, previous_gray, current_gray):
        changed = np.zeros(len(self.strokes), bool)
        self._transport_trusted = np.zeros(len(self.strokes), bool)
        if not len(self.strokes):
            return changed
        pts = self.strokes[:, 1:3]*self.factor
        moved, trusted = transport(pts, forward, backward, previous_gray, current_gray)
        displacement = (moved-pts)/self.factor
        displacement[np.linalg.norm(displacement, axis=1)<.22] = 0
        # Long strokes belong to the still scene: they stay where they were laid.
        displacement[np.isin(self.strokes[:,11], LONG)] = 0
        if self.mode == 'transported':
            changed = trusted & np.any(displacement != 0, axis=1)
            self.strokes[trusted, 1:3] += displacement[trusted]
        self._transport_trusted = trusted
        return changed

    def update(self, image, forward, backward, detail_image=None, attention=None,
               motion_grays=None):
        if self.detail=='people' and attention is None:
            raise ValueError('Person-guided brushwork requires an attention field')
        first = not len(self.strokes)
        rgb = image.astype(np.float32)
        analysis_scale = image.shape[1]/self.w
        if self.pile_count and self.piles is None:
            self.piles = Piles(image, self.pile_count)
        brush_scale = self.scale*analysis_scale
        def pigment(image, points, image_scale=analysis_scale, tint=False):
            values = sample(image, points*image_scale)
            if tint and self.detail_hour is not None:
                # Grade the few sampled pigments, not millions of source pixels.
                values = grade(values[:,None,:], self.detail_hour%24).reshape(-1,3).astype(np.float32)
            return values
        fine = cv2.GaussianBlur(rgb, (0,0), .7*brush_scale)
        medium = cv2.GaussianBlur(rgb, (0,0), 2.5*brush_scale)
        broad = cv2.GaussianBlur(rgb, (0,0), 6*brush_scale)
        feature = None
        gray = cv2.cvtColor(image, cv2.COLOR_RGB2GRAY).astype(np.float32)
        current_gray=cv2.resize(gray,(self.fw,self.fh),interpolation=cv2.INTER_AREA)
        dx = dy = None
        sources = [(blurred, analysis_scale) for blurred in [broad, medium, fine]]
        if self.detail != 'original':
            # Finishing marks read the full source, while the three approved
            # coats keep exactly the same analysis and seeded brush geometry.
            if detail_image is None:
                raise ValueError('Detail finishing requires a source detail image')
            detail_scale = detail_image.shape[1]/self.w
            radius = self.scale*detail_scale
            detail_rgb = detail_image.astype(np.float32)
            accent = cv2.GaussianBlur(detail_rgb, (0,0), .6*radius)
            detail_gray = cv2.cvtColor(detail_image, cv2.COLOR_RGB2GRAY).astype(np.float32)
            local = cv2.GaussianBlur(detail_gray, (0,0), .9*radius)
            ax = cv2.Sobel(local, cv2.CV_32F, 1, 0, ksize=3)*.125
            ay = cv2.Sobel(local, cv2.CV_32F, 0, 1, ksize=3)*.125
            if self.detail!='people':
                surround = cv2.GaussianBlur(detail_rgb, (0,0), 4.*radius)
                contrast_channels = cv2.split(cv2.absdiff(accent,surround))
                accent_feature = cv2.max(cv2.max(contrast_channels[0],contrast_channels[1]),contrast_channels[2])
                tensor = cv2.GaussianBlur(np.stack([ax*ax, ay*ay, ax*ay], axis=2), (0,0), 2.*radius)
                coherence = np.sqrt((tensor[:,:,0]-tensor[:,:,1])**2+4*tensor[:,:,2]**2)/(tensor[:,:,0]+tensor[:,:,1]+1)
            sources.append((accent, detail_scale))
        if self.hand in ('lines', 'full'):
            soft = cv2.GaussianBlur(gray, (0,0), 1.1*brush_scale)
            lx = cv2.Sobel(soft, cv2.CV_32F, 1, 0, ksize=3)*.125
            ly = cv2.Sobel(soft, cv2.CV_32F, 0, 1, ksize=3)*.125
            tensor = cv2.GaussianBlur(np.stack([lx*lx, ly*ly, lx*ly], axis=2), (0,0), 3.5*brush_scale)
            firm = np.sqrt((tensor[:,:,0]-tensor[:,:,1])**2+4*tensor[:,:,2]**2)/(tensor[:,:,0]+tensor[:,:,1]+1)
        def line_pigment(points, angles, seeds, tone=LINE_TONE, touches=.25):
            # A drawn line takes the darker side of its edge as an accent;
            # of the short ones, about one in four is a light touch from the brighter side.
            side = np.stack([-np.sin(angles), np.cos(angles)], axis=1)*(2.2*self.scale)
            a, b = pigment(fine, points+side), pigment(fine, points-side)
            swap = a.sum(axis=1)>b.sum(axis=1)
            dark, light = np.where(swap[:,None], b, a), np.where(swap[:,None], a, b)
            highlight = ((seeds*91.7)%1<touches)[:,None]
            paint = self.mixed(np.where(highlight, light, dark), seeds)
            return np.where(highlight, np.minimum(paint*1.05, 255), paint*tone)
        def border_pigment(points, angles, seeds):
            # A border takes the colour of the thing it bounds, deepened and
            # strengthened. Not a black line laid on top.
            if self.border_tone=='dark' or not len(points):
                return line_pigment(points, angles, seeds, CONTOUR_TONE, 0)
            inside = points-np.stack([-np.sin(angles), np.cos(angles)], axis=1)*(8*self.scale)
            lab = cv2.cvtColor(np.clip(self.mixed(pigment(medium, inside), None), 0, 255).astype(np.float32)[None]/255, cv2.COLOR_RGB2Lab)[0]
            lab[:,0] = np.maximum(lab[:,0]-30*self.border_depth, 6)
            lab[:,1:] *= 1+.4*self.border_depth
            return cv2.cvtColor(lab[None], cv2.COLOR_Lab2RGB)[0]*255
        def restated(marks):
            # A picked line is put back in its thing's own colour, pushed a little
            # further from what lies either side of it.
            slope = marks[:,5]+np.arctan(bends(marks[:,6])[1])
            beside = np.stack([-np.sin(slope), np.cos(slope)], axis=1)*(marks[:,4]+4*self.scale)[:,None]
            own = pigment(fine, marks[:,1:3])
            around = (pigment(medium, marks[:,1:3]+beside)+pigment(medium, marks[:,1:3]-beside))/2
            return np.clip(own+(own-around)*.35*self.border_depth, 0, 255)
        looking = first or self.turn%self.pace==0
        if self.hand in LINE_HANDS:
            if looking:
                strength, firm, edge_way, form_way, lift = self.fields(gray, brush_scale)
            def heading(way):
                def at(points):
                    doubled = pigment(way, points)
                    angle = .5*np.arctan2(doubled[:,1], doubled[:,0])
                    return np.stack([np.cos(angle), np.sin(angle)], axis=1)
                return at
            if self.previous_gray is not None:
                # Where the scene has lately moved, a long stroke has nothing still to sit on.
                moved = cv2.GaussianBlur(cv2.absdiff(current_gray, self.previous_gray), (0,0), 1.5)
                self.unrest = moved if self.unrest is None else np.maximum(self.unrest*.85, moved)
                # A wider, slower sense of where things are passing: lines nearby get livelier.
                wide = cv2.GaussianBlur(self.unrest, (0,0), 12)
                self.stirred = wide if self.stirred is None else self.stirred*.8+wide*.2
            def showing(marks):
                # A picked line is painted only while the frame shows its thing there.
                # Whatever stands in front of it, a stopped car, someone waiting, a
                # hanging basket in flower, hides it. A line already up is given the
                # benefit of the doubt.
                spots = [along(marks, u) for u in (-.8, -.4, 0., .4, .8)]
                slope = marks[:,5]+np.arctan(bends(marks[:,6])[1])
                across = np.stack([-np.sin(slope), np.cos(slope)], axis=1)
                way = heading(edge_way)(spots[2])
                turned = np.abs(way[:,0]*np.sin(slope)-way[:,1]*np.cos(slope))>.5
                outline = marks[:,11]==CONTOUR
                reach = across*np.where(outline, 8*self.scale, marks[:,4]+4*self.scale)[:,None]
                # Round a shape the paint differs across the line; a slim line differs from both its sides.
                differs = np.median([np.abs(pigment(medium, spot+reach)-pigment(medium, spot-reach)).max(axis=1) for spot in spots], axis=0)
                stands = np.median([np.minimum(np.abs(pigment(fine, spot)-pigment(fine, spot+reach)).max(axis=1),
                                               np.abs(pigment(fine, spot)-pigment(fine, spot-reach)).max(axis=1)) for spot in spots], axis=0)
                return ~turned & (np.where(outline, differs/10, stands/6)>np.where(marks[:,10]>.5, .7, 1.))
        if len(self.strokes):
            previous_motion,current_motion = motion_grays or (self.previous_gray,current_gray)
            self.transport_only(forward,backward,previous_motion,current_motion)
            trusted = self._transport_trusted
            self.age += 1
            target = np.zeros((len(self.strokes),3),np.float32)
            for coat, (blurred, image_scale) in enumerate(sources):
                selected = self.strokes[:,11]==coat
                if selected.any():target[selected] = pigment(blurred,self.strokes[selected,1:3],image_scale,coat==3)
            drawn = self.strokes[:,11]==DRAWN
            pulls, contours, ridges = (self.strokes[:,11]==coat for coat in LONG)
            long = pulls | contours | ridges
            picked = (self.strokes[:,0]>=self.picked_ids[0]) & (self.strokes[:,0]<self.picked_ids[1])
            if pulls.any():
                target[pulls] = pigment(medium, self.strokes[pulls,1:3])
            if ridges.any():
                target[ridges] = pigment(fine, self.strokes[ridges,1:3])
            if (picked & ridges).any():
                target[picked & ridges] = restated(self.strokes[picked & ridges])
            if self.piles is not None:
                body = ~(drawn | contours)
                target[body] = self.mixed(target[body], self.strokes[body,6])
            if drawn.any():
                target[drawn] = line_pigment(self.strokes[drawn,1:3], self.strokes[drawn,5], self.strokes[drawn,6])
            if contours.any():
                lines = self.strokes[contours]
                slope = lines[:,5]+np.arctan(bends(lines[:,6])[1])
                target[contours] = border_pigment(lines[:,1:3], slope, lines[:,6])
            delta = target-self.strokes[:, 7:10]
            mismatch = np.max(np.abs(delta), axis=1)
            # Give a bad estimate one frame to recover, then retire the mark.
            bad = (~trusted & (mismatch>24)) | (mismatch>65)
            if drawn.any():
                # A line lives only while the edge it was drawn on is still there
                # and still runs its way; otherwise a passing figure leaves hairs.
                edge = pigment(tensor, self.strokes[drawn,1:3])
                tangent = .5*np.arctan2(2*edge[:,2], edge[:,0]-edge[:,1])+np.pi/2
                bad[drawn] |= ((np.sqrt(edge[:,0]+edge[:,1])<5.5) |
                               (pigment(firm, self.strokes[drawn,1:3])[:,0]<.5) |
                               (np.abs(np.sin(tangent-self.strokes[drawn,5]))>.5))
            covered = np.zeros(len(self.strokes), bool)
            if long.any():
                # Long strokes are not carried by motion. One is lost while what it
                # was laid on is covered or gone, and comes back when that comes back.
                marks = self.strokes[long]
                spots = [along(marks, u) for u in (-.9, -.5, 0., .5, .9)]
                if len(self.doubt)!=len(self.strokes):
                    self.doubt = np.zeros(len(self.strokes), bool)
                if looking:
                    tone = [pigment(medium, spot) for spot in spots]
                    uneven = np.max([np.abs(t-tone[2]).max(axis=1) for t in tone], axis=0)>44
                    edge = np.min([pigment(strength, spot)[:,0] for spot in spots[1:4]], axis=0)
                    grip = np.min([pigment(firm, spot)[:,0] for spot in spots[1:4]], axis=0)
                    thin = np.min([np.abs(pigment(lift, spot)[:,0]) for spot in spots[1:4]], axis=0)
                    way = heading(edge_way)(spots[2])
                    slope = marks[:,5]+np.arctan(bends(marks[:,6])[1])
                    turned = np.abs(way[:,0]*np.sin(slope)-way[:,1]*np.cos(slope))>.5
                    gone = (edge<4.5/self.keen) | (grip<.4/self.keen**.5) | turned
                    if self.hand=='border':
                        # A border holds for as long as the paint still differs across it.
                        out = np.stack([-np.sin(slope), np.cos(slope)], axis=1)*(8*self.scale)
                        gone = np.max([np.abs(pigment(medium, spot+out)-pigment(medium, spot-out)).max(axis=1)
                                       for spot in spots[1:4]], axis=0)<12
                    verdict = np.select([marks[:,11]==PULL, marks[:,11]==CONTOUR], [uneven, gone], (thin<5) | turned)
                    if picked.any():
                        verdict[picked[long]] = ~showing(marks[picked[long]])
                    self.doubt[long] = verdict
                # Between looks at the scene the last verdict on a stroke's footing stands.
                # A picked line is never doubted for a change of light.
                verdict = self.doubt[long]
                lost = np.where(picked[long], verdict, verdict | (mismatch[long]>50))
                if self.unrest is not None:
                    # A broad pull would smear over anything crossing it, so it hides
                    # at once and stays hidden until the place has been still a moment.
                    # A drawn line dissolves while it is crossed, so it never lies across
                    # a passing car. Nobody can tell then whether its edge is still
                    # there, so that is judged once the place is still again.
                    # Someone's legs touch a line at one place and it can stay; a car covers more.
                    touched = np.sum([sample(self.unrest, spot*self.factor)[:,0]>6 for spot in spots], axis=0)
                    crossed = touched>=np.where(marks[:,11]==PULL, 1, 2)
                    lost = np.where(marks[:,11]==PULL, lost | crossed, lost & ~crossed)
                    covered[long] = crossed & (marks[:,11]!=PULL)
                bad[long] = lost
            people_marks = (self.strokes[:,11]==3) & (self.detail=='people')
            if people_marks.any():
                presence = sample(attention,self.strokes[people_marks,1:3]*self.factor)[:,0]
                bad[people_marks] |= presence<.025
            self.failures = np.where(covered, self.failures, np.where(bad, self.failures+1, np.maximum(0, self.failures-1)))
            # A long stroke that was only covered returns as soon as it is clear.
            back = long & ~bad & ~covered & ~picked
            self.failures[back] = np.minimum(self.failures[back], 1)
            # A picked line is hidden, and brought back, only once the frame has said
            # so for a moment: a flicker in the frame is not a flicker in the line.
            self.failures[picked] = np.minimum(self.failures[picked], 9)
            # Drawn lines come and go over a second or more, and only once their edge
            # has been gone a while: none should be seen to appear or to vanish.
            lines = contours | ridges
            dying = np.where(lines, self.failures>=6, (self.failures>=2) | (pulls & bad))
            step = np.where(dying, np.select([lines, pulls], [-.06, -.2], -.28), np.select([lines, pulls], [.04, .12], .24))
            step = np.where(covered, -.2, step)
            step[picked & bad & ~dying & ~covered] = 0
            self.strokes[:, 10] = np.clip(self.strokes[:, 10]+step, 0, 1)
            # Pigment settles per mark, not per pixel. Deadband ignores camera noise.
            delta[np.abs(delta)<2.5] = 0
            delta[long & (bad | covered)] = 0
            self.strokes[:, 7:10] += np.clip(delta*.22, -7, 7)
            if people_marks.any():
                # Small moving figures must refresh faster than broad scenery
                # or old coat colors swallow heads and limbs for several frames.
                self.strokes[people_marks,7:10] += np.clip(delta[people_marks]*.42,-12,12)-np.clip(delta[people_marks]*.22,-7,7)
            inside = ((self.strokes[:, 1]>0) & (self.strokes[:, 1]<self.w-1) &
                      (self.strokes[:, 2]>0) & (self.strokes[:, 2]<self.h-1))
            keep = inside & ((self.strokes[:, 10]>.015) | (long & (self.failures<45)))
            self.retired += int((~keep).sum())
            if len(self.doubt)==len(keep):
                self.doubt = self.doubt[keep]
            self.strokes, self.age, self.failures = self.strokes[keep], self.age[keep], self.failures[keep]

        additions = []
        spare = max(0, self.reserve-int(np.isin(self.strokes[:,11], LONG).sum()))
        remaining = max(0,self.limit-len(self.strokes)-spare)
        for coat, (pts, variety, step, (blurred, image_scale)) in enumerate(zip(
                self.candidates, self.variants, self.steps, sources)):
            # A full population cannot accept candidates. Keep transporting and
            # recoloring its marks, but defer placement work until space opens.
            if not remaining:
                break
            if coat == 3:
                if self.detail=='people':
                    priority = sample(attention,pts*self.factor)[:,0]
                    # Dense source-colored paint across the person, not just
                    # glints at high-contrast edges. Feather effort at the margin.
                    eligible = priority>variety[:,3]*.90+.04
                else:
                    contrast = pigment(accent_feature, pts, image_scale)[:,0]
                    structure = pigment(coherence, pts, image_scale)[:,0]
                    eligible = (contrast > 14) & (structure > .30)
                    eligible &= variety[:,3] < .80
            else:
                if coat == 0:
                    eligible = np.ones(len(pts), bool)
                else:
                    if feature is None:
                        r,g,b = cv2.split(cv2.absdiff(fine,broad))
                        feature = cv2.max(cv2.max(r,g),b)
                    detail = pigment(feature, pts)[:,0]
                    eligible = detail > (8 if coat==1 else 17)
                if coat==2:
                    eligible &= variety[:,3]<.70
            existing = self.strokes[(self.strokes[:,11]==coat) & (self.strokes[:,10]>.18)]
            if len(existing):
                # A fine center should cover its own site without excluding
                # neighboring tiny features through the coarse motion grid.
                ow, oh = (self.w,self.h) if coat==3 else (self.fw,self.fh)
                occupancy_factor = np.array([ow/self.w,oh/self.h], np.float32)
                occupancy = np.ones((oh,ow), np.uint8)
                xy = np.clip(existing[:,1:3]*occupancy_factor, [0,0], [ow-1,oh-1]).astype(int)
                occupancy[xy[:,1],xy[:,0]] = 0
                distance = cv2.distanceTransform(occupancy, cv2.DIST_L2, cv2.DIST_MASK_PRECISE)
                # Include quantization error from the occupancy raster and
                # bilinear query; an existing center must cover its own site.
                eligible &= sample(distance, pts*occupancy_factor)[:,0]>step*occupancy_factor[0]*.64+1.5
            ids = np.flatnonzero(eligible)
            if not first:
                # A fixed replenishment budget prevents a noisy frame from exploding count.
                budget = 1200 if coat==3 and self.detail=='people' else max(24,int(len(pts)*(.015 if coat==3 else .035)))
                ids = ids[:budget]
            ids = ids[:remaining]
            remaining -= len(ids)
            if not len(ids):
                continue
            xy, rv = pts[ids], variety[ids]
            c = pigment(blurred, xy, image_scale,coat==3)
            if coat != 3 and dx is None:
                field = cv2.GaussianBlur(gray, (0,0), 3*brush_scale)
                dx = cv2.Sobel(field, cv2.CV_32F, 1, 0, ksize=3)*.125
                dy = cv2.Sobel(field, cv2.CV_32F, 0, 1, ksize=3)*.125
            gx = pigment(ax if coat==3 else dx,xy,image_scale)[:,0]
            gy = pigment(ay if coat==3 else dy,xy,image_scale)[:,0]
            energy = np.hypot(gx,gy)
            angle = np.arctan2(gy,gx)+np.pi/2
            # Long flowing passages in quiet regions; short marks at small features.
            base = (rv[:,0]-.5)*1.2-.30
            if coat==3 and self.detail=='people':
                base = -np.pi/2+(rv[:,0]-.5)*.65
            angle = np.where(energy>1.5, angle+(rv[:,1]-.5)*.32, base)
            lengths = step*([1.35,1.2,.90,.60][coat]+rv[:,0]*[.85,.65,.55,.85][coat])
            widths = step*([.55,.40,.30,.25][coat]+rv[:,1]*[.16,.14,.20,.22][coat])
            if coat==3 and self.detail=='people':
                lengths = step*(1.+rv[:,0])
                # Overlapping loaded marks build the body/clothing as paint.
                # Hairline marks leave coarse underpaint showing through it.
                widths = step*(.95+rv[:,1]*.25)
            if self.hand in ('varied','full') and coat<3:
                # A painter does not repeat one mark: long pulls, short turned
                # dabs and slim touches sit among the approved filberts.
                kind = (rv[:,2]*7.31)%1
                pull, dab, slim = kind<.22, (kind>=.22)&(kind<.44), (kind>=.44)&(kind<.60)
                lengths = lengths*np.select([pull,dab,slim],[1.9+rv[:,3]*.9,.55,1.6],1.)
                widths = widths*np.select([pull,dab,slim],[.8,1.15,.45],1.)
                angle = angle+np.where(dab,(rv[:,3]-.5)*1.,0.)
            # Stop each axis at the first distinct color boundary.
            for axis in [0,1]:
                axis_angle = angle+(np.pi/2 if axis else 0)
                direction = np.stack([np.cos(axis_angle),np.sin(axis_angle)],axis=1)
                extent = widths if axis else lengths
                limit = np.ones(len(xy), np.float32)
                active = np.ones(len(xy), bool)
                for fraction in [.28,.46,.66,.85,1.]:
                    reach = direction*(extent*fraction)[:,None]
                    a = pigment(blurred,xy+reach,image_scale,coat==3)
                    b = pigment(blurred,xy-reach,image_scale,coat==3)
                    threshold = 18 if coat==3 and self.detail=='people' else [29,26,32,22][coat]
                    crossing = np.maximum(np.max(np.abs(a-c),axis=1),np.max(np.abs(b-c),axis=1)) > threshold
                    stop = crossing & active
                    limit[stop] = max(.25,fraction-.13)
                    active[stop] = False
                extent *= limit
            if coat==3 and self.detail=='people':
                widths = np.maximum(widths,.7*self.scale)
            p = np.zeros((len(ids),12),np.float32)
            p[:,0] = np.arange(self.next_id,self.next_id+len(ids))
            p[:,1:3], p[:,3], p[:,4], p[:,5], p[:,6] = xy, lengths, widths, angle, rv[:,2]
            p[:,7:10], p[:,10], p[:,11] = self.mixed(c, rv[:,2]), (1 if first else (.8 if coat==3 and self.detail=='people' else .25)), coat
            additions.append(p)
            self.next_id += len(ids)
        if self.hand in ('lines','full') and remaining:
            # Drawing with the point of the brush: thin lines laid along firm
            # edges, placed sparsely so contours are found and lost, not traced.
            pts, variety = self.line_candidates, self.line_variants
            at = pigment(tensor, pts)
            eligible = (np.sqrt(at[:,0]+at[:,1])>8.) & (pigment(firm, pts)[:,0]>.72) & (variety[:,3]<.5)
            existing = self.strokes[(self.strokes[:,11]==DRAWN) & (self.strokes[:,10]>.18)]
            if len(existing):
                occupancy = np.ones((self.fh,self.fw), np.uint8)
                xy = np.clip(existing[:,1:3]*self.factor, [0,0], [self.fw-1,self.fh-1]).astype(int)
                occupancy[xy[:,1],xy[:,0]] = 0
                distance = cv2.distanceTransform(occupancy, cv2.DIST_L2, cv2.DIST_MASK_PRECISE)
                eligible &= sample(distance, pts*self.factor)[:,0]>7.
            ids = np.flatnonzero(eligible)
            if not first:
                ids = ids[:max(24,int(len(pts)*.02))]
            ids = ids[:remaining]
            if len(ids):
                xy, rv, at = pts[ids], variety[ids], at[ids]
                angle = .5*np.arctan2(2*at[:,2], at[:,0]-at[:,1])+np.pi/2+(rv[:,1]-.5)*.06
                lengths = (16+rv[:,0]*30)*self.scale
                c = pigment(fine, xy)
                direction = np.stack([np.cos(angle),np.sin(angle)],axis=1)
                limit = np.ones(len(xy), np.float32)
                active = np.ones(len(xy), bool)
                for fraction in [.3,.5,.7,.85,1.]:
                    reach = direction*(lengths*fraction)[:,None]
                    crossing = np.maximum(np.max(np.abs(pigment(fine,xy+reach)-c),axis=1),
                                          np.max(np.abs(pigment(fine,xy-reach)-c),axis=1)) > 34
                    stop = crossing & active
                    limit[stop] = max(.3,fraction-.15)
                    active[stop] = False
                p = np.zeros((len(ids),12),np.float32)
                p[:,0] = np.arange(self.next_id,self.next_id+len(ids))
                p[:,1:3], p[:,3], p[:,4], p[:,5], p[:,6] = xy, lengths*limit, (.9+rv[:,1]*.9)*self.scale, angle, rv[:,2]
                p[:,7:10], p[:,10], p[:,11] = line_pigment(xy, angle, rv[:,2]), (1 if first else .3), DRAWN
                additions.append(p)
                self.next_id += len(ids)
        remaining += spare
        hidden = None
        if self.picked is not None and first:
            # Lines picked by hand on the still scene: laid once, where they were picked.
            found, breadth, outline, owner, where, slender = picked_strokes(self.picked, (self.w,self.h), self.scale, self.slim)
            rv = self.line_variants[(np.arange(len(found))*7919)%len(self.line_variants)]
            p = np.zeros((len(found),12), np.float32)
            code = (np.round(found[:,4]/BEND*63)+64)*128+np.round(found[:,5]/BEND*63)+64
            p[:,0] = np.arange(self.next_id, self.next_id+len(p))
            p[:,1:3], p[:,3], p[:,5], p[:,6] = found[:,:2], found[:,2], found[:,3], (code+rv[:,2]*.99)/16384
            # An outline is drawn with the point of the brush, a longer stretch with more
            # of it, and never broader than its shape can carry. A line is as broad as
            # the marking it restates.
            point = (1.6+3.*rv[:,0])*self.scale*self.border_weight*(.75+.5*np.minimum(1, found[:,2]/(55*self.scale)))
            p[:,4], p[:,11] = np.where(outline, np.clip(.08*breadth, 1.2*self.scale, point), breadth), np.where(outline, CONTOUR, RIDGE)
            slope = p[:,5]+np.arctan(bends(p[:,6])[1])
            p[:,7:10] = np.where(outline[:,None], border_pigment(p[:,1:3], slope, p[:,6]), self.mixed(restated(p), p[:,6]))
            up = showing(p)
            p[:,10], hidden = up, p[~up,0]
            self.picked_ids, self.picked_owner = (self.next_id, self.next_id+len(p)), owner
            self.next_id += len(p)
            self.laid[0] += len(p)
            remaining = max(0, remaining-len(p))
            additions.append(p)
            slim = (where>0) & slender[np.maximum(where-1, 0)]
            def small(mask):
                return cv2.resize(mask.astype(np.float32), (self.fw,self.fh), interpolation=cv2.INTER_AREA)
            self.thing_mask = small((where>0) & ~slim)
            self.line_mask = .35*cv2.GaussianBlur(small(cv2.dilate(slim.astype(np.uint8), np.ones((9,9), np.uint8))), (0,0), 1.5)
            self.where = cv2.resize(where.astype(np.float32), (self.fw,self.fh), interpolation=cv2.INTER_NEAREST).astype(int)
            # How far the paint of each shape may be shifted: a share of its breadth.
            across = np.zeros(len(self.picked)+1, np.float32)
            np.maximum.at(across, owner+1, np.where(outline, breadth, 2*breadth))
            self.give = cv2.GaussianBlur(np.maximum(.16*across, 3*self.scale)[self.where], (0,0), 1.5)
        # New lines are looked for every fourth update at the study's pace: things,
        # their outlines, and markings to restate, all at once. At a slower pace the
        # three take turns, one to a look, so no single update carries them all.
        cycle, turns = (4, (0, 0, 0)) if self.pace==1 else (3*self.pace, (0, self.pace, 2*self.pace))
        outlining, restating, finding = (first or self.turn%cycle==turn for turn in turns)
        # A kind of line looked for less often is looked for at as many more sites,
        # and may lay as many more, so that lines are found at the same rate.
        seldom = cycle//4
        if self.hand in LINE_HANDS and self.picked is None and remaining and (outlining or restating or finding):
            calm = np.zeros((self.fh,self.fw), np.float32) if self.unrest is None else self.unrest
            def still(marks):
                return np.max([sample(calm, along(marks, u)*self.factor)[:,0] for u in (-.9, -.5, 0., .5, .9)], axis=0)<4
            def within(points, margin):
                return ((points[:,0]>margin) & (points[:,0]<self.w-margin) &
                        (points[:,1]>margin) & (points[:,1]<self.h-margin))
            def record(found, rv, widths, coat):
                p = np.zeros((len(found),12), np.float32)
                code = (np.round(found[:,4]/BEND*63)+64)*128+np.round(found[:,5]/BEND*63)+64
                p[:,1:3], p[:,3], p[:,4], p[:,5] = found[:,:2], np.minimum(found[:,2], 300*self.scale), widths, found[:,3]
                p[:,6], p[:,10], p[:,11] = (code+rv[:,2]*.99)/16384, (1 if first else (.3 if coat==PULL else .04)), coat
                return p
            def lay(marks, crowd, budget, wide):
                # Longest first; skip any that would crowd a stroke already down.
                marks = marks[still(marks)] if len(marks) else marks
                if not len(marks):
                    return marks
                def course(m):
                    return np.round(np.stack([along(m, u) for u in np.linspace(-1, 1, 9)], axis=1)*self.factor).astype(np.int32)
                def weight(m):
                    return np.maximum(1, np.round(m[:,4]*2*self.factor[0]*wide)).astype(int)
                taken = np.zeros((self.fh,self.fw), np.uint8)
                old = self.strokes[self.strokes[:,11]==marks[0,11]]
                for route, thick in zip(course(old), weight(old)):
                    cv2.polylines(taken, [route], False, 1, int(thick))
                chosen, routes, thicks = [], course(marks), weight(marks)
                for i in np.argsort(-marks[:,3], kind='stable'):
                    route = np.clip(routes[i], 0, [self.fw-1,self.fh-1])
                    if taken[route[:,1],route[:,0]].mean()>crowd:
                        continue
                    cv2.polylines(taken, [routes[i]], False, 1, int(thicks[i]))
                    chosen.append(i)
                    if len(chosen)>=budget:
                        break
                return marks[chosen]
            def sites(eligible, most):
                ids = np.flatnonzero(eligible & (sample(calm, pts*self.factor)[:,0]<4))
                return ids if first else np.roll(ids, -self.turn*211)[:most*seldom]
            def settle(image):
                # Nudge a walker sideways onto the crest of the edge or line it follows.
                def onto(points, direction):
                    side = np.stack([-direction[:,1], direction[:,0]], axis=1)*(1.5*self.scale)
                    # Both sides in one lookup: a walk makes thousands of these.
                    either = pigment(image, np.concatenate([points+side, points-side]))[:,0]
                    lean = either[:len(points)]-either[len(points):]
                    return points+side*np.clip(lean*.25, -.4, .4)[:,None]
                return onto
            def drawn_line(ids, alive, onto, coat, widths, faces_in=None):
                # A drawn line may curve but not hook: it gives up at a corner.
                path, valid = trace(pts[ids], heading(edge_way), alive, 6*self.scale, 36, onto, .96)
                rv = self.line_variants[ids]
                found = fit(path, valid, (rv[:,3]<.3) & (faces_in is None), 2.2*self.scale, .22)
                good = (found[:,6]<=2.2*self.scale) & (found[:,2]>30*self.scale)
                found, rv = found[good], rv[good]
                if faces_in is not None and len(found):
                    # Run every outline the same way round its thing, so that "across"
                    # the line always means away from the thing.
                    back = faces_in(found)
                    found[back,3] += np.pi
                    found[back,4] *= -1
                return record(found, rv, widths(found, rv), coat)
            laid = []
            # Long pulls: a loaded brush drawn through a quiet passage, along its lie.
            pts = self.pull_candidates
            ids = sites(pigment(cv2.absdiff(fine, broad), pts).max(axis=1)<12, 800) if self.hand=='gesture' and outlining else []
            if len(ids):
                origin = pigment(medium, pts[ids])
                def same(points):
                    return (np.abs(pigment(medium, points)-origin).max(axis=1)<20) & within(points, 6)
                path, valid = trace(pts[ids], heading(form_way), same, 12*self.scale, 22)
                rv = self.pull_variants[ids]
                found = fit(path, valid, rv[:,3]<.35, 5*self.scale)
                good = (found[:,6]<=5*self.scale) & (found[:,2]>45*self.scale)
                p = record(found[good], rv[good], (9+13*rv[good,1])*self.scale, PULL)
                # Narrow a pull that would spill over a neighbouring colour.
                c = pigment(medium, p[:,1:3])
                limit, active = np.ones(len(p), np.float32), np.ones(len(p), bool)
                across = np.stack([-np.sin(p[:,5]), np.cos(p[:,5])], axis=1)
                for fraction in (.55, .8, 1.05, 1.35):
                    for u in (-.8, -.4, 0., .4, .8):
                        # The bristles splay past the nominal width, so look that far.
                        spot, reach = along(p, u), across*(p[:,4]*fraction)[:,None]
                        crossing = np.maximum(np.abs(pigment(medium, spot+reach)-c).max(axis=1),
                                              np.abs(pigment(medium, spot-reach)-c).max(axis=1))>22
                        stop = crossing & active
                        limit[stop] = max(.3, fraction/1.35-.2)
                        active[stop] = False
                p[:,4] = np.maximum(p[:,4]*limit, 5*self.scale)
                p[:,7:10] = self.mixed(c, p[:,6])
                laid.append(lay(p, .4, 900 if first else 30*seldom, .8))
            pts = self.line_candidates
            thin = pigment(lift, pts)[:,0] if restating else None
            # Contour lines: the point of the brush run round the outline of a thing.
            if finding or self.solid is None:
                self.solid, self.off_outline = self.things(image)
                self.thing_mask = cv2.resize(self.solid, (self.fw,self.fh), interpolation=cv2.INTER_AREA)
            solid, off_outline = self.solid, self.off_outline
            def faces_in(found):
                side = np.stack([-np.sin(found[:,3]), np.cos(found[:,3])], axis=1)*(8*self.scale)
                return pigment(solid, found[:,:2]+side)[:,0]>pigment(solid, found[:,:2]-side)[:,0]
            if self.hand=='border' and outlining:
                # Only round things that have held their place for a second or so:
                # a patch that comes and goes between looks is texture, not a thing.
                self.votes = solid if self.votes is None or first else self.votes*.6+solid*.4
                solid = (self.votes>.75).astype(np.float32)
                self.thing_mask = cv2.resize(solid, (self.fw,self.fh), interpolation=cv2.INTER_AREA)
                # A border goes right round each thing: a chain of strokes that follow
                # its outline exactly, each as long as the outline runs without a corner.
                path, valid = self.outlines(solid, analysis_scale)
                found = fit(path, valid, np.zeros(len(path), bool), 3*self.scale, .3)
                found = found[(found[:,6]<=3*self.scale) & (found[:,2]>8*self.scale)]
                back = faces_in(found)
                found[back,3] += np.pi
                found[back,4] *= -1
                rv = self.line_variants[(np.arange(len(found))*7919+self.turn*31)%len(self.line_variants)]
                p = record(found, rv, (2.6+1.6*rv[:,0])*self.scale, CONTOUR)
                p[:,7:10] = border_pigment(p[:,1:3], p[:,5]+np.arctan(bends(p[:,6])[1]), p[:,6])
                laid.append(lay(p, .3, 1400 if first else 60*seldom, .5))
            elif self.hand!='border':
                ids = sites((pigment(strength, pts)[:,0]>6.5/self.keen) & (pigment(firm, pts)[:,0]>.62/self.keen**.5) &
                            (pigment(off_outline, pts)[:,0]<3), 600) if outlining else []
                if len(ids):
                    # One picture of the three things a walker asks at every step.
                    footing = np.dstack([strength, firm, off_outline])
                    def on_edge(points):
                        at = pigment(footing, points)
                        return ((at[:,0]>5.5/self.keen) & (at[:,1]>.5/self.keen**.5) &
                                within(points, 3) & (at[:,2]<5))
                    # A longer line is drawn with more of the brush.
                    p = drawn_line(ids, on_edge, settle(strength), CONTOUR, lambda found, rv:
                        (1.6+3.*rv[:,0])*self.scale*self.border_weight*(.75+.5*np.minimum(1, found[:,2]/(150*self.scale))), faces_in)
                    p[:,7:10] = border_pigment(p[:,1:3], p[:,5]+np.arctan(bends(p[:,6])[1]), p[:,6])
                    laid.append(lay(p, .25, 1400 if first else 40*seldom, .5))
                # Restated lines: a road marking or a pole put back in one stroke of its own colour.
                # Light markings only need it under the pulls, which smear over them;
                # dark poles and rails are things in their own right and are always drawn.
                ids = sites(((np.abs(thin) if self.hand=='gesture' else -thin)>14) & (pigment(firm, pts)[:,0]>.6), 600) if restating else []
                if len(ids):
                    sign, origin = np.sign(thin[ids]), pigment(fine, pts[ids])
                    marking = np.dstack([fine, lift])
                    def on_line(points):
                        at = pigment(marking, points)
                        return ((at[:,3]*sign>8) & within(points, 3) &
                                (np.abs(at[:,:3]-origin).max(axis=1)<34))
                    p = drawn_line(ids, on_line, settle(np.abs(lift)), RIDGE, lambda found, rv: np.full(len(found), 9*self.scale))
                    if self.hand!='gesture':
                        # A dark line lying across the ground is a shadow or a crack; one
                        # standing up is a pole, a rail or a trunk.
                        p = p[np.abs(np.cos(p[:,5]))<.45]
                    # As wide as the thing it restates, which must end on both sides of it.
                    c = pigment(fine, p[:,1:3])
                    slope = p[:,5]+np.arctan(bends(p[:,6])[1])
                    across, open_ = np.stack([-np.sin(slope), np.cos(slope)], axis=1), np.ones(len(p), bool)
                    for reach in (2.5, 4., 5.5, 7.):
                        side = across*reach*self.scale
                        crossing = np.minimum(np.abs(pigment(fine, p[:,1:3]+side)-c).max(axis=1),
                                              np.abs(pigment(fine, p[:,1:3]-side)-c).max(axis=1))>22
                        stop = crossing & open_
                        p[stop,4] = reach*self.scale
                        open_[stop] = False
                    # Anything wider than that is a shape, not a line: the coats paint it.
                    p, c = p[~open_], c[~open_]
                    p[:,7:10] = self.mixed(c, p[:,6])
                    laid.append(lay(p, .25, 1400 if first else 40*seldom, .5))
            for p in laid:
                p = p[:remaining]
                remaining -= len(p)
                p[:,0] = np.arange(self.next_id, self.next_id+len(p))
                self.next_id += len(p)
                self.laid[int(not first)] += len(p)
                additions.append(p)
        self.turn += 1
        if additions:
            added = np.concatenate(additions)
            self.births += len(added)
            self.strokes = np.concatenate([self.strokes,added])
            self.age = np.concatenate([self.age,np.zeros(len(added),np.int32)])
            self.failures = np.concatenate([self.failures,np.zeros(len(added),np.float32)])
            if len(self.doubt)+len(added)!=len(self.strokes):
                self.doubt = np.zeros(len(self.strokes)-len(added), bool)
            self.doubt = np.concatenate([self.doubt,np.zeros(len(added),bool)])
            order = np.argsort(self.strokes[:,11],kind='stable')
            self.strokes,self.age,self.failures = self.strokes[order],self.age[order],self.failures[order]
            self.doubt = self.doubt[order]
            if hidden is not None:
                self.failures[np.isin(self.strokes[:,0], hidden)] = 9
        if len(self.strokes)>self.limit:
            raise RuntimeError('Stroke population exceeded the experiment memory bound')
        # A separate, opaque painted ground covers newly exposed space while
        # the moving marks replenish. It consists of broad brush deposits too.
        if self.ground is None:
            xy=self.candidates[0][::3];rv=self.variants[0][::3]
            self.ground=np.zeros((len(xy),12),np.float32)
            self.ground[:,0]=1_000_000+np.arange(1,len(xy)+1)
            self.ground[:,1:3]=xy
            self.ground[:,3]=(76+rv[:,0]*35)*self.scale
            self.ground[:,4]=(46+rv[:,1]*18)*self.scale
            self.ground[:,5]=(rv[:,2]-.5)*.8
            self.ground[:,6]=rv[:,3]
            self.ground[:,10]=1
            self.ground[:,11]=-1
        self.ground[:,7:10]=self.mixed(pigment(broad,self.ground[:,1:3]),self.ground[:,6])
        self.previous_gray=current_gray
        self.shown.append(int(((self.strokes[:,11]>=CONTOUR) & (self.strokes[:,10]>.5)).sum()))
        visible = self.strokes[self.strokes[:,10]>.015]
        if self.picked is not None:
            # How much of each picked shape is showing: its paint ripples only that much.
            mine = (self.strokes[:,0]>=self.picked_ids[0]) & (self.strokes[:,0]<self.picked_ids[1])
            owner = self.picked_owner[(self.strokes[mine,0]-self.picked_ids[0]).astype(int)]
            up = np.bincount(owner, weights=self.strokes[mine,10], minlength=len(self.picked))/np.maximum(np.bincount(owner, minlength=len(self.picked)), 1)
            self.present = np.append(0, np.clip(up/.6, 0, 1)).astype(np.float32)[self.where]
            self.showing.append(int((self.strokes[mine,10]>.5).sum()))
        self.towed = self.swim() if self.pulse else self.tow(visible)
        marks = np.concatenate([self.ground,self.adrift(visible)])
        if self.detail=='people':
            selected = marks[:,11]==3
            if selected.any():
                marks[selected,10] *= sample(attention,marks[selected,1:3]*self.factor)[:,0]
        return marks


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('source'); ap.add_argument('output')
    ap.add_argument('--seconds',type=float,default=18)
    ap.add_argument('--width',type=int,default=1920)
    ap.add_argument('--hour',type=float,default=15)
    ap.add_argument('--mode',choices=['transported','anchored'],default='transported')
    ap.add_argument('--detail',choices=['original','selective','people','adaptive'],default='original')
    ap.add_argument('--material',choices=['plain','loaded','scumble','swept','mixed'],default='plain')
    ap.add_argument('--hand',choices=['approved','varied','lines','full','gesture','contour','border'],default='approved',
                    help='varied: long pulls, dabs and slim touches; lines: thin drawn lines on edges; full: both; '
                         'gesture: long curved pulls and contour lines laid on the still scene; contour: those lines without the pulls; '
                         'border: a closed border round each thing, which swells and thins while the thing ripples inside it')
    ap.add_argument('--drift',type=float,default=1.,help='how far drawn lines sway, flex and flow (0: still)')
    ap.add_argument('--ripple',type=float,default=1.,help='how far the paint inside a bordered thing drifts (0: not at all)')
    ap.add_argument('--line-motion',choices=['sway','pulse'],default='sway',
                    help='sway: lines sway and flex and pull the paint along; pulse: lines stay put, swell and thin, and the thing inside ripples')
    ap.add_argument('--border-tone',choices=['own','dark'],default='dark',help="own: the thing's colour deepened; dark: a dark accent")
    ap.add_argument('--border-weight',type=float,default=1.,help='how broad the lines round things are drawn')
    ap.add_argument('--border-depth',type=float,default=1.,help="how far a border in its thing's own colour is deepened and strengthened")
    ap.add_argument('--ripple-everywhere',type=float,default=0.,help='share of the ripple given to the picture outside the things (0: only inside them)')
    ap.add_argument('--stand-out',type=float,default=20,help='how far a patch must differ from its surroundings to count as a thing')
    ap.add_argument('--stir',type=float,default=0.,help='how much livelier drawn lines get near passing people and cars')
    ap.add_argument('--swell',type=float,default=.85,help='the most a pulsing line may swell and thin, as a share of its width')
    ap.add_argument('--keen',type=float,default=1.,help='how readily a faint edge of a thing is given a line (1: only firm edges)')
    ap.add_argument('--outlines',type=pathlib.Path,help='shapes picked out by hand on the tracing page (scene/lines.json): lines are laid on these and nowhere else')
    ap.add_argument('--slim',type=float,default=18.,help='a picked shape slimmer than this many pixels (of 1920) is drawn as one line down its middle, not outlined')
    ap.add_argument('--piles',type=int,default=0,help='mix this many colour piles and paint only from them (0: copy the source colour)')
    ap.add_argument('--person-model',type=pathlib.Path,default=pathlib.Path(__file__).parent/'models/nanodet.onnx')
    args = ap.parse_args()
    root = pathlib.Path(args.output);root.parent.mkdir(parents=True,exist_ok=True)
    cap = cv2.VideoCapture(args.source)
    fps = cap.get(cv2.CAP_PROP_FPS)
    if not cap.isOpened() or fps<=0: raise RuntimeError('Cannot open source')
    w = args.width;h = round(cap.get(cv2.CAP_PROP_FRAME_HEIGHT)/cap.get(cv2.CAP_PROP_FRAME_WIDTH)*w/2)*2
    fw,fh=480,round(480*h/w)
    cv2.setNumThreads(2)
    dis = cv2.DISOpticalFlow_create(cv2.DISOPTICAL_FLOW_PRESET_FAST)
    picked=placed=None
    if args.outlines:
        # The shapes were picked on a still view of the scene. Find how that view sits
        # in this clip, which may be framed a little differently, and move them to match.
        from tracing import prepare
        drawn_on=json.loads(args.outlines.read_text())
        ok,opening=cap.read();cap.release();cap=cv2.VideoCapture(args.source)
        if not ok: raise RuntimeError('Cannot read source')
        opening=cv2.resize(opening,(drawn_on['width'],drawn_on['height']),interpolation=cv2.INTER_AREA)
        views=[cv2.imread(str(args.outlines.with_name(name))) for name in ('plate.jpg','plate-day.jpg')]
        matches=[m for m in (prepare.matched(view,opening) for view in views if view is not None) if m is not None]
        turn,placed=max(matches,key=lambda m:m[1]) if matches else (np.eye(3),0)
        picked=[cv2.perspectiveTransform(np.float32(line['points'])[None],turn)[0]*(w/drawn_on['width']) for line in drawn_on['lines']]
        print(f'{len(picked)} picked shapes, placed on {placed} matching points',flush=True)
    book = Painting(w,h,(fw,fh),args.mode,'original' if args.detail=='adaptive' else args.detail,args.hour,hand=args.hand,piles=args.piles,drift=args.drift,stir=args.stir,ripple=args.ripple,border_tone=args.border_tone,pulse=args.line_motion=='pulse',
                    border_weight=args.border_weight,border_depth=args.border_depth,ripple_everywhere=args.ripple_everywhere,stand_out=args.stand_out,picked=picked,slim=args.slim,swell=args.swell,keen=args.keen)
    adaptive=None;adaptive_cost=[]
    if args.detail=='adaptive':
        from adaptive import AdaptiveBrushwork
        adaptive=AdaptiveBrushwork(w,h,(fw,fh),mode=args.mode)
        if args.hand in ('lines','full')+LINE_HANDS or args.piles:
            covered=np.zeros((h,w),np.uint8);slack=7 if args.piles else 0
            def forgiving(reference,working,error):
                # The finishing pass must not "correct" a drawn line, nor the
                # lean each stroke takes from its pile.
                np.copyto(error,np.maximum(cv2.absdiff(reference,working).max(axis=2)-slack,0))
                error[covered>0]=0
            adaptive.error_filter=forgiving
    detector=people=None;next_detection=0;detection_cost=[];people_counts=[]
    if args.detail=='people':
        from people import PersonDetector, PeopleAttention
        if not args.person_model.is_file():
            raise RuntimeError('Run artwork/stroke-memory/fetch_person_model.py to fetch the optional person model')
        detector=PersonDetector(args.person_model);people=PeopleAttention(w,h,(fw,fh))
    native = pathlib.Path(__file__).parent/'native/target/release/stroke-study-raster'
    error = root.with_suffix('.raster.log').open('w')
    painter = subprocess.Popen([str(native),str(w),str(h),args.material],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=error)
    encoder = subprocess.Popen(['ffmpeg','-v','error','-y','-f','rawvideo','-pixel_format','rgb24',
            '-video_size',f'{w}x{h}','-framerate',str(fps),'-i','pipe:0','-an','-c:v','libx264',
            '-preset','veryfast','-crf','23','-threads','2','-pix_fmt','yuv420p','-movflags','+faststart',str(root)],stdin=subprocess.PIPE)
    def raster(marks,append=False):
        painter.stdin.write(struct.pack('<I',len(marks)|((1<<31) if append else 0)))
        painter.stdin.write(marks.astype('<f4').tobytes());painter.stdin.flush()
        raw=painter.stdout.read(w*h*3)
        if len(raw)!=w*h*3:raise RuntimeError('Native painter stopped before completing the frame')
        return np.frombuffer(raw,np.uint8).reshape(h,w,3)
    xx=np.arange(w,dtype=np.float32);yy=np.arange(h,dtype=np.float32)
    def edge(v,size):
        t=np.clip(np.minimum(v,size-1-v)/(size*.036),0,1)
        return t*t*(3-2*t)
    ex,ey=edge(xx,w),edge(yy,h)
    grid_x,grid_y=np.meshgrid(xx,yy)
    previous=None;paint=None;shown=None;n=0;cost=[];flow_cost=[];native_cost=[];updates=[]
    start=time.perf_counter()
    try:
        while n<round(args.seconds*fps):
            ok, bgr=cap.read()
            if not ok:break
            frame_start=time.perf_counter()
            if n%2==0:
                if bgr.shape[:2]!=(h,w): bgr=cv2.resize(bgr,(w,h),interpolation=cv2.INTER_AREA)
                small=cv2.resize(cv2.cvtColor(bgr,cv2.COLOR_BGR2GRAY),(fw,fh),interpolation=cv2.INTER_AREA)
                flow_start=time.perf_counter()
                if previous is None:forward=backward=np.zeros((fh,fw,2),np.float32)
                else:
                    forward=dis.calc(previous,small,None)
                    backward=dis.calc(small,previous,None)
                flow_cost.append((time.perf_counter()-flow_start)*1000)
                previous=small
                analysis=cv2.resize(bgr,(min(w,960),round(h*min(w,960)/w)),interpolation=cv2.INTER_AREA)
                rgb=grade(cv2.cvtColor(analysis,cv2.COLOR_BGR2RGB),args.hour%24)
                detail_rgb = None if args.detail=='original' else cv2.cvtColor(bgr,cv2.COLOR_BGR2RGB)
                attention=None
                if people is not None:
                    detections=None
                    if n>=next_detection:
                        detection_start=time.perf_counter();detections=detector.detect(bgr)
                        detection_cost.append((time.perf_counter()-detection_start)*1000)
                        next_detection += fps/2
                    attention=people.update(small,forward,backward,2/fps,detections)
                    people_counts.append(len(people.tracks))
                marks=book.update(rgb,forward,backward,detail_rgb,attention)
                if adaptive is not None:
                    reference=grade_reference(detail_rgb,args.hour%24)
                    if book.piles is not None:reference=book.piles.repaint(reference)
                    retained=adaptive.begin(reference,forward,backward)
                    if book.piles is not None:
                        retained=retained.copy();retained[:,7:10]+=book.piles.lean(retained[:,6],retained[:,7:10])
                    drawn=marks[:,11]>=DRAWN  # drawn lines go on last, over the finishing marks
                    if adaptive.error_filter is not None:
                        covered[:]=0;lines=marks[drawn].copy()
                        lines[lines[:,11]==DRAWN,6]=(64*128+64)/16384  # short drawn lines are straight
                        if len(lines):
                            routes=np.round(np.stack([along(lines,u) for u in np.linspace(-1,1,9)],axis=1)).astype(np.int32)
                            for route,width in zip(routes,lines[:,4]):cv2.polylines(covered,[route],False,1,int(round(width*2+6)))
                    marks=np.concatenate([marks[~drawn],retained,marks[drawn]])
                raster_start=time.perf_counter()
                paint=raster(marks)
                native_cost.append((time.perf_counter()-raster_start)*1000)
                if adaptive is not None and n%6==0:
                    # Existing strokes still move and recolor at paint cadence.
                    # Decide where NEW marks earn their cost at roughly 5 Hz.
                    refinement_start=time.perf_counter()
                    for level,radius in enumerate([12.,6.,2.5]):
                        radius*=w/1920
                        before=adaptive.error(paint,radius) if n%30==0 else None
                        added=adaptive.propose(paint,radius,level,[700,1000,1400][level] if n==0 else [140,240,600][level])
                        if len(added):
                            paint=raster(added,append=True)
                        if before is not None:
                            adaptive.checks.append([before,adaptive.error(paint,radius),len(added)])
                    adaptive_cost.append((time.perf_counter()-refinement_start)*1000)
                updates.append(len(marks) if adaptive is None else len(book.strokes)+len(book.ground)+len(adaptive.strokes))
                shown=paint
                if book.towed is not None:
                    # Each drawn line takes the paint around it along as it moves.
                    shift=cv2.resize(book.towed,(w,h),interpolation=cv2.INTER_LINEAR)
                    shown=cv2.remap(paint,grid_x-shift[:,:,0],grid_y-shift[:,:,1],cv2.INTER_LINEAR,borderMode=cv2.BORDER_REPLICATE)
                if n in [0,90,180,300,480]:
                    cv2.imwrite(str(root.with_name(root.stem+f'-unwarped-{n}.png')),cv2.cvtColor(shown,cv2.COLOR_RGB2BGR))
                    if adaptive is not None and adaptive.error_filter is not None:
                        cv2.imwrite(str(root.with_name(root.stem+f'-lines-{n}.png')),255-covered*255)
                    if people is not None:
                        root.with_name(root.stem+f'-attention-{n}.json').write_text(json.dumps(people.tracks.tolist()))
            # Hold paint between updates. Apply the original separable, edge-anchored wave once.
            phase=n/fps/8*np.pi*2
            sx=np.clip(xx+np.sin(phase+xx*8/w)*w*.012*ex,0,w-1)
            sy=np.clip(yy+np.sin(phase+yy*8/h)*h*.012*ey,0,h-1)
            warped=cv2.remap(shown,np.broadcast_to(sx,(h,w)).copy(),np.broadcast_to(sy[:,None],(h,w)).copy(),cv2.INTER_LINEAR)
            encoder.stdin.write(warped.tobytes())
            cost.append((time.perf_counter()-frame_start)*1000)
            if n%90==0:print(f'{n}/{round(args.seconds*fps)} frames; {len(book.strokes)} marks',flush=True)
            n+=1
    finally:
        cap.release();painter.stdin.close();encoder.stdin.close()
        paint_exit=painter.wait();encode_exit=encoder.wait();error.close()
    if paint_exit or encode_exit:raise RuntimeError('Native painter or encoder failed')
    stats={'frames':n,'width':w,'height':h,'fps':fps,'paint_fps':fps/2,'duration_seconds':n/fps,
           'wall_seconds':time.perf_counter()-start,'frame_p50_ms':float(np.median(cost)),
           'frame_p95_ms':float(np.percentile(cost,95)),'flow_p50_ms':float(np.median(flow_cost)),
           'native_roundtrip_p50_ms':float(np.median(native_cost)),'maximum_strokes':max(updates),
           'born':book.births,'retired':book.retired,'mode':args.mode,'hour':args.hour,'detail':args.detail,'material':args.material,'hand':args.hand,'piles':args.piles,'long_strokes_laid_first_and_since':book.laid,'drift':args.drift,'stir':args.stir,'things_found_mean':float(np.mean(book.thing_counts or [0])),
           'drawn_lines_visible_min_mean_max':[min(book.shown[30:] or [0]),float(np.mean(book.shown[30:] or [0])),max(book.shown[30:] or [0])],
           'flow_dimensions':[fw,fh],'source':str(pathlib.Path(args.source).resolve())}
    if book.rippling:
        stats.update(paint_rippling_share_min_mean_max=[min(book.rippling),float(np.mean(book.rippling)),max(book.rippling)])
    if picked is not None:
        stats.update(picked_shapes=len(picked),picked_placed_on_points=placed,picked_strokes=book.picked_ids[1]-book.picked_ids[0],
                     picked_strokes_showing_min_mean_max=[min(book.showing),float(np.mean(book.showing)),max(book.showing)])
    if people is not None:
        stats.update(person_detection_p50_ms=float(np.median(detection_cost)),person_detection_calls=len(detection_cost),maximum_person_tracks=max(people_counts),person_model=str(args.person_model.resolve()))
    if adaptive is not None:
        checks=np.array(adaptive.checks)
        stats.update(adaptive_born=adaptive.born,adaptive_retired=adaptive.retired,adaptive_final_strokes=len(adaptive.strokes),adaptive_capacity_hits=adaptive.capacity_hits,
                     adaptive_accepted_by_scale=adaptive.accepted_by_scale,adaptive_proposed_by_scale=adaptive.proposed_by_scale,
                     adaptive_refinement_p50_ms=float(np.median(adaptive_cost)),adaptive_refinement_hz=fps/6,
                     adaptive_sampled_error_before=float(checks[:,0].mean()),adaptive_sampled_error_after=float(checks[:,1].mean()))
    root.with_suffix('.json').write_text(json.dumps(stats,indent=2));print(json.dumps(stats),flush=True)


if __name__=='__main__':main()
