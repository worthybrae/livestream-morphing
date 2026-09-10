#!/usr/bin/env python3
"""Offline diagnostic, not part of production. Requires OpenCV and NumPy.
Measures paint changes after removing the signature warp and source motion.
This is a flicker diagnostic, not a score for artistic quality.
"""
import argparse,json,cv2,numpy as np
p=argparse.ArgumentParser();p.add_argument('source');p.add_argument('videos',nargs='+');p.add_argument('--frames',type=int,default=180);a=p.parse_args()
w,h=640,360
grid_x,grid_y=np.meshgrid(np.arange(w,dtype=np.float32),np.arange(h,dtype=np.float32))
def inverse_signature(image,number):
 phase=number/30/8*np.pi*2
 xx=grid_x.copy();yy=grid_y.copy()
 for _ in range(4):
  xx-=(xx+np.sin(phase+xx*8/w)*w*.012-grid_x)/(1+np.cos(phase+xx*8/w)*8*.012)
  yy-=(yy+np.sin(phase+yy*8/h)*h*.012-grid_y)/(1+np.cos(phase+yy*8/h)*8*.012)
 return cv2.remap(image,xx,yy,cv2.INTER_LINEAR)
def read(cap):
 ok,image=cap.read()
 return cv2.resize(image,(w,h),interpolation=cv2.INTER_AREA) if ok else None
source=cv2.VideoCapture(a.source);caps=[cv2.VideoCapture(v) for v in a.videos]
previous=None;previous_paint=None;scores=[[] for _ in caps];changes=[[] for _ in caps]
for n in range(a.frames):
 raw=read(source);paint=[read(c) for c in caps]
 if raw is None or any(im is None for im in paint):break
 if n%2:continue
 gray=cv2.cvtColor(raw,cv2.COLOR_BGR2GRAY)
 paint=[inverse_signature(im,n) for im in paint]
 if previous is not None:
  flow=cv2.calcOpticalFlowFarneback(gray,previous,None,.5,3,15,3,5,1.2,0)
  mx=grid_x+flow[:,:,0];my=grid_y+flow[:,:,1]
  before=cv2.remap(previous,mx,my,cv2.INTER_LINEAR)
  # Exclude genuine source changes, clipped warp borders, and unreliable flow.
  mask=np.abs(gray.astype(np.float32)-before.astype(np.float32))<2
  mask[:12]=False;mask[-12:]=False;mask[:,:16]=False;mask[:,-16:]=False
  for i,im in enumerate(paint):
   warped=cv2.remap(previous_paint[i],mx,my,cv2.INTER_LINEAR)
   delta=np.abs(im.astype(np.float32)-warped.astype(np.float32)).mean(axis=2)
   scores[i].append(float(delta[mask].mean()))
   changes[i].append(float((delta[mask]>15).mean()))
 previous=gray;previous_paint=paint
for path,values,large in zip(a.videos,scores,changes):
 print(json.dumps({'video':path,'pairs':len(values),'motion_compensated_mae':float(np.mean(values)),
  'p95_pair_mae':float(np.percentile(values,95)), 'fraction_large_changes':float(np.mean(large))}))
