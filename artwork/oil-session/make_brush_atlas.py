#!/usr/bin/env python3
"""Generate reproducible oil-brush deposits from pressure and individual fibers.
No source photographs or external brush assets. Runtime reads the packed atlas.
"""
from pathlib import Path
import numpy as np,cv2
W,H,N=128,64,12
u,v=np.meshgrid(np.linspace(-1,1,W),np.linspace(-1,1,H))
def smooth(x):
 t=np.clip(x,0,1);return t*t*(3-2*t)
textures=[];previews=[]
for n in range(N):
 rng=np.random.default_rng(9137+n*733)
 kind=n%3;t=(u+1)/2
 width=.70+.13*np.sin(t*np.pi)-t*.17
 if kind==0: width*=np.sqrt(np.maximum(0,1-(np.abs(u)*.91)**6))
 if kind==2: width*=np.sqrt(np.maximum(0,1-u*u))
 width*=1+.045*np.sin(u*(4+n*.2)+n)+.025*np.sin(u*17+n)
 heel=smooth((u+1)*10);tip=smooth((1-u)*9)
 body=np.maximum(0,1-(v/np.maximum(width,.001))**2)
 height=body*.22*heel*tip
 # The fibers splay, gather into loaded bundles, and end at different lengths.
 for j in range(42):
  pos=-.92+j/41*1.84
  spread=pos*width
  wander=np.sin(u*rng.uniform(1.5,4)+rng.uniform(0,6.28))*rng.uniform(.005,.022)
  center=spread+wander
  sigma=rng.uniform(.011,.030)
  start=rng.uniform(-1,-.83);end=rng.uniform(.68,1.0)
  if kind==1 and j%7==0:end-=.15
  span=smooth((u-start)*12)*smooth((end-u)*10)
  load=.6+.4*np.sin(u*rng.uniform(.4,1.4)+rng.uniform(0,6.28))
  fiber=np.exp(-((v-center)/sigma)**2)*span*load*rng.uniform(.13,.28)
  height+=fiber
 # Broad pigment ridges under the fine fibers, like a loaded brush unloading.
 height*=.85+.15*np.sin(u*3.2+n)+.06*np.sin(u*10+v*3+n)
 if kind==1:height*=.92+.08*np.sin(v*23+u*2+n)
 mask=smooth((height-.014)/.055)
 du=np.gradient(height,axis=1)*W*.10
 dv=np.gradient(height,axis=0)*H*.10
 tex=np.stack([mask*255,127+np.clip(du,-1,1)*126,127+np.clip(dv,-1,1)*126,np.clip(height,0,1)*255],axis=-1).astype(np.uint8)
 textures.append(tex)
 light=np.clip(.55-dv*.65-du*.25,0,1)
 preview=np.stack([.65*light,.40*light,.25*light],axis=-1)*mask[:,:,None]+.1*(1-mask[:,:,None])
 previews.append((preview*255).astype(np.uint8))
root=Path(__file__).resolve().parents[2]
packed=[]
for texture in textures:
 for level in range(5):
  packed.append(cv2.resize(texture,(W>>level,H>>level),interpolation=cv2.INTER_AREA).tobytes())
(root/'livestream-morphing-rs/assets/oil_brushes.rgba').write_bytes(b''.join(packed))
contact=np.concatenate([np.concatenate(previews[i:i+3],axis=1) for i in range(0,N,3)],axis=0)
cv2.imwrite('/tmp/oil-brush-atlas.png',cv2.cvtColor(cv2.resize(contact,None,fx=3,fy=3,interpolation=cv2.INTER_NEAREST),cv2.COLOR_RGB2BGR))
print('Wrote',sum(map(len,packed)),'atlas bytes including mip levels')
