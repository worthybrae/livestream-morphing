"""Local person-guided allocation of brushwork, independent of paint rendering.

NanoDet preprocessing/box decoding follows OpenCV Zoo's Apache-2.0 example.
See PERSON_MODEL.md and licenses/nanodet-LICENSE for attribution and model pin.
No source images leave this process. The detector supplies boxes, never pixels.
"""
import math

import cv2
import numpy as np

from motion import transport


class PersonDetector:
    def __init__(self, model_path, threshold=.28):
        self.net = cv2.dnn.readNet(str(model_path))
        self.threshold = threshold
        self.mean = np.array([103.53,116.28,123.675],np.float32)
        self.std = np.array([57.375,57.12,58.395],np.float32)

    def tile(self, bgr):
        image = (cv2.resize(bgr,(416,416)).astype(np.float32)-self.mean)/self.std
        self.net.setInput(cv2.dnn.blobFromImage(image))
        outputs = self.net.forward(self.net.getUnconnectedOutLayersNames())
        # OpenCV 4 and 5 return the graph outputs in different orders.
        classes = {v.shape[1]:v[0] for v in outputs if v.shape[-1]==80}
        distances = {v.shape[1]:v[0] for v in outputs if v.shape[-1]==32}
        result = []
        for count, scores in classes.items():
            active = (scores[:,0]>=self.threshold) & (scores.argmax(axis=1)==0)
            if not active.any():
                continue
            side = math.isqrt(count)
            if side*side!=count or count not in distances:
                raise RuntimeError('Unexpected person model output shapes')
            stride = 416//side
            y,x = np.divmod(np.flatnonzero(active),side)
            centers = np.stack([x,y],axis=1)*stride+.5*(stride-1)
            logits = distances[count][active].reshape(-1,4,8)
            probabilities = np.exp(logits-logits.max(axis=2,keepdims=True))
            probabilities /= probabilities.sum(axis=2,keepdims=True)
            reach = (probabilities*np.arange(8)).sum(axis=2)*stride
            boxes = np.concatenate([centers-reach[:,:2],centers+reach[:,2:]],axis=1)
            boxes = np.clip(boxes,0,416)
            result.append(np.column_stack([boxes,scores[active,0]]))
        if not result:
            return np.empty((0,5),np.float32)
        return self.suppress(np.concatenate(result),.55)

    def suppress(self, boxes, overlap):
        if not len(boxes):
            return np.empty((0,5),np.float32)
        xywh = boxes[:,:4].copy();xywh[:,2:] -= xywh[:,:2]
        keep = cv2.dnn.NMSBoxes(xywh.tolist(),boxes[:,4].tolist(),self.threshold,overlap)
        if not len(keep):
            return np.empty((0,5),np.float32)
        return boxes[np.asarray(keep).reshape(-1)].astype(np.float32)

    def detect(self, bgr):
        h,w = bgr.shape[:2]
        size = min(w,h,max(416,round(w*.4)))
        xs = np.linspace(0,w-size,max(1,math.ceil((w-size)/(size*.75))+1)).round().astype(int)
        ys = np.linspace(0,h-size,max(1,math.ceil((h-size)/(size*.75))+1)).round().astype(int)
        result = []
        for y in ys:
            for x in xs:
                boxes = self.tile(bgr[y:y+size,x:x+size])
                if not len(boxes):
                    continue
                boxes[:,:4] *= size/416
                # A crop edge can hide the head or feet from the detector.
                # Extend the attention area into neighboring source context.
                # This only changes where real source pigment is sampled.
                bh = boxes[:,3]-boxes[:,1]
                if y>0:
                    clipped = boxes[:,1]<3
                    boxes[clipped,1] -= np.minimum(bh[clipped]*.4,40)
                if y+size<h:
                    clipped = boxes[:,3]>size-3
                    boxes[clipped,3] += np.minimum(bh[clipped]*.3,40)
                boxes[:,[0,2]] += x;boxes[:,[1,3]] += y
                boxes[:,:4] = np.clip(boxes[:,:4],[0,0,0,0],[w,h,w,h])
                width = boxes[:,2]-boxes[:,0];height = boxes[:,3]-boxes[:,1]
                result.append(boxes[(height>width*1.15) & (height>18*w/1920)])
        return self.suppress(np.concatenate(result),.35) if result else np.empty((0,5),np.float32)


def box_iou(a,b):
    lo = np.maximum(a[:2],b[:2]);hi = np.minimum(a[2:4],b[2:4])
    intersection = np.maximum(0,hi-lo).prod()
    union = np.maximum(0,a[2:4]-a[:2]).prod()+np.maximum(0,b[2:4]-b[:2]).prod()-intersection
    return intersection/max(1,union)


class PeopleAttention:
    """Smooth boxes between sparse detector checks using the existing flow.

    Rows store x1/y1/x2/y2, confidence, seconds since last detection, and ID.
    Tracks are anonymous and local; no identity/face recognition is performed.
    """
    def __init__(self, width, height, flow_size):
        self.w,self.h = width,height
        self.fw,self.fh = flow_size
        self.factor = np.array([self.fw/width,self.fh/height],np.float32)
        self.tracks = np.empty((0,7),np.float32)
        self.next_id = 1
        self.previous = None

    def update(self, gray, forward, backward, dt, detections=None):
        self.tracks[:,5] += dt
        if len(self.tracks) and self.previous is not None:
            # Several interior samples keep one unreliable flow vector from
            # pulling a person's entire attention area into the background.
            grid = np.array([[x,y] for y in [.2,.5,.8] for x in [.25,.5,.75]],np.float32)
            for track in self.tracks:
                points = (track[:2]+grid*(track[2:4]-track[:2]))*self.factor
                moved,valid = transport(points,forward,backward,self.previous,gray)
                if valid.sum()>=3:
                    delta = np.median((moved[valid]-points[valid])/self.factor,axis=0)
                    if np.linalg.norm(delta)<max(6,(track[3]-track[1])*.35):
                        track[:4] += np.tile(delta,2)
        if detections is not None:
            available = set(range(len(self.tracks)))
            new = []
            for box in sorted(detections,key=lambda b:-b[4]):
                best = max(available,key=lambda j:box_iou(box,self.tracks[j]),default=None)
                if best is not None and box_iou(box,self.tracks[best])>.15:
                    track = self.tracks[best]
                    track[:4] = track[:4]*.25+box[:4]*.75
                    track[4] = max(track[4]*.8,box[4]);track[5] = 0
                    available.remove(best)
                else:
                    new.append([*box[:5],0,self.next_id]);self.next_id += 1
            if new:
                self.tracks = np.concatenate([self.tracks,np.array(new,np.float32)])
        self.tracks = self.tracks[self.tracks[:,5]<2.0]
        self.previous = gray
        return self.field()

    def field(self):
        result = np.zeros((self.fh,self.fw),np.float32)
        smooth = lambda v:(v:=np.clip(v,0,1))*v*(3-2*v)
        for box in self.tracks:
            cx,cy = (box[:2]+box[2:4])*.5*self.factor
            bw,bh = np.maximum(2,(box[2:4]-box[:2])*self.factor)
            rx,ry = bw*.72,bh*.63
            x0,x1 = max(0,int(cx-rx)),min(self.fw,math.ceil(cx+rx)+1)
            y0,y1 = max(0,int(cy-ry)),min(self.fh,math.ceil(cy+ry)+1)
            if x1<=x0 or y1<=y0:
                continue
            x = np.abs((np.arange(x0,x1)-cx)/rx)
            y = np.abs((np.arange(y0,y1)-cy)/ry)
            mask = smooth((1-x)/.40)[None,:]*smooth((1-y)/.30)[:,None]
            fade = 1-smooth((box[5]-1.2)/.8)
            result[y0:y1,x0:x1] = np.maximum(result[y0:y1,x0:x1],mask*fade)
        return result
