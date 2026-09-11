"""Error-driven brush allocation for the recorded moving-paint study.

Inspired by Hertzmann's coarse-to-fine painterly rendering (SIGGRAPH 1998).
The actual raster is checked between scales. Within a batch, a conservative
brush-footprint model predicts improvement and charges for every new mark.
This is an independent adaptation, not a reproduction of the paper.
"""
import cv2
import numpy as np

from motion import transport


def at(image, points):
    if not len(points):
        channels=1 if image.ndim==2 else image.shape[2]
        return np.empty((0,channels),np.float32)
    return np.concatenate([cv2.remap(image,part[:,0:1].astype(np.float32),part[:,1:2].astype(np.float32),
        cv2.INTER_LINEAR,borderMode=cv2.BORDER_REPLICATE).reshape(len(part),-1)
        for part in np.array_split(points,max(1,(len(points)+16383)//16384))])


def footprint(x,y,length,width,angle,w,h):
    """Approximate filbert coverage used to price a proposed paint deposit."""
    sn,cs=np.sin(angle),np.cos(angle)
    bx=abs(cs)*length+abs(sn)*width*1.2+2
    by=abs(sn)*length+abs(cs)*width*1.2+2
    x0,x1=max(0,int(x-bx)),min(w,int(x+bx)+1)
    y0,y1=max(0,int(y-by)),min(h,int(y+by)+1)
    yy,xx=np.mgrid[y0:y1,x0:x1].astype(np.float32)
    u=((xx-x)*cs+(yy-y)*sn)/length
    v=(-(xx-x)*sn+(yy-y)*cs)/width
    caps=np.sqrt(np.maximum(0,1-(np.maximum(0,np.abs(u)-.5)/.5)**2))
    alpha=np.clip((caps*.85-np.abs(v))*width,0,1)*.82
    alpha[np.abs(u)>=1]=0
    return (slice(y0,y1),slice(x0,x1)),alpha[...,None]


class AdaptiveBrushwork:
    def __init__(self,width,height,flow_size,limit=20000,mode='transported'):
        self.w,self.h=width,height
        self.fw,self.fh=flow_size
        self.factor=np.array([self.fw/width,self.fh/height],np.float32)
        self.scale=width/1920
        self.limit=limit
        self.mode=mode
        self.strokes=np.empty((0,12),np.float32)
        self.failures=np.empty(0,np.float32)
        self.previous=None
        self.next_id=2_000_000
        self.born=0;self.retired=0;self.capacity_hits=0
        self.accepted_by_scale=[0,0,0]
        self.proposed_by_scale=[0,0,0]
        self.checks=[]
        self.accept_filter=None

    def begin(self,graded_source,forward,backward):
        self.first=self.previous is None
        self.references={}
        self.source=cv2.GaussianBlur(graded_source.astype(np.float32),(0,0),max(.35,.65*self.scale))
        gray=cv2.cvtColor(graded_source,cv2.COLOR_RGB2GRAY)
        current=cv2.resize(gray,(self.fw,self.fh),interpolation=cv2.INTER_AREA)
        # Existing marks only need motion and pigment. Build the orientation
        # field lazily on the less frequent updates that propose new strokes.
        self.edge_gray=gray
        self.dx=self.dy=None
        if len(self.strokes):
            points=self.strokes[:,1:3]*self.factor
            moved,trusted=transport(points,forward,backward,self.previous,current)
            displacement=(moved-points)/self.factor
            displacement[np.linalg.norm(displacement,axis=1)<.22]=0
            if self.mode=='transported':
                self.strokes[trusted,1:3]+=displacement[trusted]
            target=np.empty((len(self.strokes),3),np.float32)
            for level,radius in enumerate([12.,6.,2.5]):
                selected=self.strokes[:,11]==4+level
                if selected.any():
                    target[selected]=at(self.reference(radius*self.scale),self.strokes[selected,1:3])
            delta=target-self.strokes[:,7:10]
            mismatch=np.max(np.abs(delta),axis=1)
            bad=(mismatch>45)|(~trusted & (mismatch>24))
            self.failures=np.where(bad,self.failures+1,np.maximum(0,self.failures-1))
            dying=self.failures>=2
            self.strokes[:,10]=np.clip(self.strokes[:,10]+np.where(dying,-.40,.25),0,1)
            delta[np.abs(delta)<2.5]=0
            self.strokes[:,7:10]+=np.clip(delta*.35,-10,10)
            keep=(self.strokes[:,10]>.015)&(self.strokes[:,1]>0)&(self.strokes[:,1]<self.w-1)&(self.strokes[:,2]>0)&(self.strokes[:,2]<self.h-1)
            self.retired+=int((~keep).sum())
            self.strokes,self.failures=self.strokes[keep],self.failures[keep]
        self.previous=current
        return self.strokes

    def reference(self,radius):
        if radius not in self.references:
            self.references[radius]=cv2.GaussianBlur(self.source,(0,0),max(.45,radius*.20))
        return self.references[radius]

    def error(self,canvas,radius):
        reference=self.reference(radius)
        painted=cv2.GaussianBlur(canvas.astype(np.float32),(0,0),max(.4,.6*self.scale))
        return float(np.mean(cv2.mean(cv2.absdiff(reference,painted))[:3]))

    def propose(self,canvas,radius,level,budget):
        budget=min(budget,self.limit-len(self.strokes))
        if budget<=0:
            self.capacity_hits+=1
            return np.empty((0,12),np.float32)
        reference=self.reference(radius)
        working=cv2.GaussianBlur(canvas.astype(np.float32),(0,0),max(.4,.6*self.scale))
        channels=cv2.split(cv2.absdiff(reference,working))
        error=cv2.max(cv2.max(channels[0],channels[1]),channels[2])
        grid=max(3,round(radius*2))
        gh,gw=(self.h+grid-1)//grid,(self.w+grid-1)//grid
        padded=cv2.copyMakeBorder(error,0,gh*grid-self.h,0,gw*grid-self.w,cv2.BORDER_CONSTANT,value=0)
        cells=padded.reshape(gh,grid,gw,grid).transpose(0,2,1,3).reshape(gh,gw,grid*grid)
        average=cells.mean(axis=2)
        maxima=cells.argmax(axis=2)
        peak=np.take_along_axis(cells,maxima[...,None],axis=2)[...,0]
        eligible=(average>10)&(peak>18)
        cy,cx=np.nonzero(eligible)
        score=average[cy,cx]+peak[cy,cx]*.25
        order=np.argsort(-score,kind='stable')[:budget*5]
        cy,cx=cy[order],cx[order]
        offsets=maxima[cy,cx]
        xs=cx*grid+offsets%grid;ys=cy*grid+offsets//grid
        colors=reference[ys,xs]
        if self.dx is None:
            field=cv2.GaussianBlur(self.edge_gray.astype(np.float32),(0,0),max(.5,1.4*self.scale))
            self.dx=cv2.Sobel(field,cv2.CV_32F,1,0,ksize=3)*.125
            self.dy=cv2.Sobel(field,cv2.CV_32F,0,1,ksize=3)*.125
        gx,gy=self.dx[ys,xs],self.dy[ys,xs]
        angles=np.arctan2(gy,gx)+np.pi/2
        # Stable spatial seeds vary shape without regenerating existing marks.
        seed=((xs.astype(np.int64)*73856093)^(ys.astype(np.int64)*19349663)^((level+1)*83492791))&0x7fffffff
        u=(seed%1009)/1009.;v=((seed//1009)%1013)/1013.
        angles=np.where(np.hypot(gx,gy)>1.5,angles+(u-.5)*.26,(u-.5)*1.2-.3)
        lengths=radius*(1.6+u*1.7);widths=radius*(.80+v*.30)
        # Stop before the proposed brush would swallow another local color.
        centers=np.column_stack([xs,ys]).astype(np.float32)
        for extent,axis in [(lengths,0),(widths,np.pi/2)]:
            direction=np.column_stack([np.cos(angles+axis),np.sin(angles+axis)])
            limit=np.ones(len(xs));active=np.ones(len(xs),bool)
            for fraction in [.30,.50,.75,1.]:
                reach=direction*(extent*fraction)[:,None]
                a,b=at(reference,centers+reach),at(reference,centers-reach)
                crossing=np.maximum(np.max(np.abs(a-colors),axis=1),np.max(np.abs(b-colors),axis=1))>22
                stop=crossing&active;limit[stop]=max(.25,fraction-.13);active[stop]=False
            extent*=limit
        widths=np.maximum(widths,.7*self.scale)
        if self.accept_filter is not None:
            candidates=np.ascontiguousarray(np.column_stack([xs,ys,lengths,widths,angles,colors]),dtype=np.float64)
            indices=self.accept_filter(working,reference,candidates,budget,self.scale)
            self.proposed_by_scale[level]+=int(indices[-1])+1 if len(indices)==budget else len(xs)
            accepted=[]
            for i in indices:
                p=np.array([self.next_id,xs[i],ys[i],lengths[i],widths[i],angles[i],u[i],*colors[i],1. if self.first else .85,4+level],np.float32)
                self.next_id+=1;accepted.append(p)
        else:
            accepted=[]
            for i,(x,y) in enumerate(zip(xs,ys)):
                self.proposed_by_scale[level]+=1
                region,alpha=footprint(x,y,lengths[i],widths[i],angles[i],self.w,self.h)
                present=working[region];target=reference[region];color=colors[i]
                predicted=present*(1-alpha)+color*alpha
                gain=float(np.sum((target-present)**2-(target-predicted)**2)/3)
                # Charge for area AND each stroke. Texture-scale noise or a mark
                # that mixes several incompatible shapes does not earn its cost.
                cost=120*float(alpha.sum())+1200*self.scale*self.scale
                if gain<=cost:
                    continue
                p=np.array([self.next_id,x,y,lengths[i],widths[i],angles[i],u[i],*color,1. if self.first else .85,4+level],np.float32)
                self.next_id+=1;accepted.append(p)
                working[region]=predicted
                if len(accepted)>=budget:
                    break
        if not accepted:
            return np.empty((0,12),np.float32)
        marks=np.array(accepted,np.float32)
        self.strokes=np.concatenate([self.strokes,marks])
        self.failures=np.concatenate([self.failures,np.zeros(len(marks),np.float32)])
        self.born+=len(marks);self.accepted_by_scale[level]+=len(marks)
        return marks
