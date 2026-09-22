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


class Painting:
    def __init__(self, width, height, flow_size, mode, detail='original', detail_hour=None, limit=45000):
        self.w, self.h = width, height
        self.fw, self.fh = flow_size
        self.factor = np.array([self.fw/width, self.fh/height], np.float32)
        self.mode = mode
        self.detail = detail
        self.detail_hour = detail_hour
        self.limit = limit
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

    def transport_only(self, forward, backward, previous_gray, current_gray):
        changed = np.zeros(len(self.strokes), bool)
        self._transport_trusted = np.zeros(len(self.strokes), bool)
        if not len(self.strokes):
            return changed
        pts = self.strokes[:, 1:3]*self.factor
        moved, trusted = transport(pts, forward, backward, previous_gray, current_gray)
        displacement = (moved-pts)/self.factor
        displacement[np.linalg.norm(displacement, axis=1)<.22] = 0
        if self.mode == 'transported':
            changed = trusted & np.any(displacement != 0, axis=1)
            self.strokes[trusted, 1:3] += displacement[trusted]
        self._transport_trusted = trusted
        return changed

    def update(self, image, forward, backward, detail_image=None, attention=None):
        if self.detail=='people' and attention is None:
            raise ValueError('Person-guided brushwork requires an attention field')
        first = not len(self.strokes)
        rgb = image.astype(np.float32)
        analysis_scale = image.shape[1]/self.w
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
        if len(self.strokes):
            self.transport_only(forward,backward,self.previous_gray,current_gray)
            trusted = self._transport_trusted
            self.age += 1
            target = np.empty((len(self.strokes),3),np.float32)
            for coat, (blurred, image_scale) in enumerate(sources):
                selected = self.strokes[:,11]==coat
                if selected.any():target[selected] = pigment(blurred,self.strokes[selected,1:3],image_scale,coat==3)
            delta = target-self.strokes[:, 7:10]
            mismatch = np.max(np.abs(delta), axis=1)
            # Give a bad estimate one frame to recover, then retire the mark.
            bad = (~trusted & (mismatch>24)) | (mismatch>65)
            people_marks = (self.strokes[:,11]==3) & (self.detail=='people')
            if people_marks.any():
                presence = sample(attention,self.strokes[people_marks,1:3]*self.factor)[:,0]
                bad[people_marks] |= presence<.025
            self.failures = np.where(bad, self.failures+1, np.maximum(0, self.failures-1))
            dying = self.failures>=2
            self.strokes[:, 10] = np.clip(self.strokes[:, 10]+np.where(dying, -.28, .24), 0, 1)
            # Pigment settles per mark, not per pixel. Deadband ignores camera noise.
            delta[np.abs(delta)<2.5] = 0
            self.strokes[:, 7:10] += np.clip(delta*.22, -7, 7)
            if people_marks.any():
                # Small moving figures must refresh faster than broad scenery
                # or old coat colors swallow heads and limbs for several frames.
                self.strokes[people_marks,7:10] += np.clip(delta[people_marks]*.42,-12,12)-np.clip(delta[people_marks]*.22,-7,7)
            inside = ((self.strokes[:, 1]>0) & (self.strokes[:, 1]<self.w-1) &
                      (self.strokes[:, 2]>0) & (self.strokes[:, 2]<self.h-1))
            keep = inside & (self.strokes[:, 10]>.015)
            self.retired += int((~keep).sum())
            self.strokes, self.age, self.failures = self.strokes[keep], self.age[keep], self.failures[keep]

        additions = []
        remaining = max(0,self.limit-len(self.strokes))
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
            p[:,7:10], p[:,10], p[:,11] = c, (1 if first else (.8 if coat==3 and self.detail=='people' else .25)), coat
            additions.append(p)
            self.next_id += len(ids)
        if additions:
            added = np.concatenate(additions)
            self.births += len(added)
            self.strokes = np.concatenate([self.strokes,added])
            self.age = np.concatenate([self.age,np.zeros(len(added),np.int32)])
            self.failures = np.concatenate([self.failures,np.zeros(len(added),np.float32)])
            order = np.argsort(self.strokes[:,11],kind='stable')
            self.strokes,self.age,self.failures = self.strokes[order],self.age[order],self.failures[order]
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
        self.ground[:,7:10]=pigment(broad,self.ground[:,1:3])
        self.previous_gray=current_gray
        marks = np.concatenate([self.ground,self.strokes])
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
    ap.add_argument('--material',choices=['plain','loaded','scumble','swept'],default='plain')
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
    book = Painting(w,h,(fw,fh),args.mode,'original' if args.detail=='adaptive' else args.detail,args.hour)
    adaptive=None;adaptive_cost=[]
    if args.detail=='adaptive':
        from adaptive import AdaptiveBrushwork
        adaptive=AdaptiveBrushwork(w,h,(fw,fh),mode=args.mode)
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
    previous=None;paint=None;n=0;cost=[];flow_cost=[];native_cost=[];updates=[]
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
                    retained=adaptive.begin(grade_reference(detail_rgb,args.hour%24),forward,backward)
                    marks=np.concatenate([marks,retained])
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
                if n in [0,90,180,300,480]:
                    cv2.imwrite(str(root.with_name(root.stem+f'-unwarped-{n}.png')),cv2.cvtColor(paint,cv2.COLOR_RGB2BGR))
                    if people is not None:
                        root.with_name(root.stem+f'-attention-{n}.json').write_text(json.dumps(people.tracks.tolist()))
            # Hold paint between updates. Apply the original separable, edge-anchored wave once.
            phase=n/fps/8*np.pi*2
            sx=np.clip(xx+np.sin(phase+xx*8/w)*w*.012*ex,0,w-1)
            sy=np.clip(yy+np.sin(phase+yy*8/h)*h*.012*ey,0,h-1)
            warped=cv2.remap(paint,np.broadcast_to(sx,(h,w)).copy(),np.broadcast_to(sy[:,None],(h,w)).copy(),cv2.INTER_LINEAR)
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
           'born':book.births,'retired':book.retired,'mode':args.mode,'hour':args.hour,'detail':args.detail,'material':args.material,
           'flow_dimensions':[fw,fh],'source':str(pathlib.Path(args.source).resolve())}
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
