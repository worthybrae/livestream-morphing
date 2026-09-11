"""Persistent Loaded bristle worker. The live engine owns decoding and the wave.

A bounded local socket carries RGB frames; ctypes shares the raster canvas
without a second process or frame copy. No model, network access or disk state.
"""
import ctypes
import json
import os
import pathlib
import socket
import struct
import sys
import time
from concurrent.futures import ThreadPoolExecutor

import cv2
import numpy as np
from render import Painting, grade, grade_reference, palette
from adaptive import AdaptiveBrushwork


class Raster:
    def __init__(self, width, height):
        suffix = '.dylib' if sys.platform == 'darwin' else '.so'
        path = os.environ.get('LOADED_RASTER', str(pathlib.Path(__file__).parent / ('native/target/release/libstroke_study_raster' + suffix)))
        self.lib = ctypes.CDLL(path)
        self.lib.paint_new.argtypes = [ctypes.c_size_t, ctypes.c_size_t, ctypes.c_bool]
        self.lib.paint_new.restype = ctypes.c_void_p
        self.lib.paint_frame.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_float), ctypes.c_size_t, ctypes.c_bool]
        self.lib.paint_frame.restype = ctypes.POINTER(ctypes.c_uint8)
        self.lib.paint_clear.argtypes = self.lib.paint_free.argtypes = [ctypes.c_void_p]
        self.lib.paint_clear.restype = self.lib.paint_free.restype = None
        self.lib.paint_grade.argtypes = [ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_uint8), ctypes.c_size_t, ctypes.c_double, ctypes.POINTER(ctypes.c_double)]
        self.lib.paint_grade.restype = None
        self.lib.paint_select.argtypes = [ctypes.POINTER(ctypes.c_float),ctypes.POINTER(ctypes.c_float),ctypes.c_size_t,ctypes.c_size_t,ctypes.POINTER(ctypes.c_double),ctypes.c_size_t,ctypes.c_size_t,ctypes.c_double,ctypes.POINTER(ctypes.c_uint32)]
        self.lib.paint_select.restype = ctypes.c_size_t
        self.handle = self.lib.paint_new(width, height, True)
        if not self.handle:
            raise ValueError('Unsupported canvas dimensions')
        self.w, self.h = width, height

    def paint(self, marks, append=False):
        marks = np.ascontiguousarray(marks, dtype=np.float32)
        ptr = self.lib.paint_frame(self.handle, marks.ctypes.data_as(ctypes.POINTER(ctypes.c_float)), len(marks), append)
        if not ptr:
            raise RuntimeError('Invalid brush packet')
        return np.ctypeslib.as_array(ptr, shape=(self.h*self.w*3,)).reshape(self.h, self.w, 3)

    def grade(self, source, hour):
        source = np.ascontiguousarray(source, dtype=np.uint8)
        result = np.empty_like(source)
        night, tint = palette(hour%24)
        tint = np.ascontiguousarray(tint, dtype=np.float64)
        self.lib.paint_grade(source.ctypes.data_as(ctypes.POINTER(ctypes.c_uint8)), result.ctypes.data_as(ctypes.POINTER(ctypes.c_uint8)), source.size//3, float(night), tint.ctypes.data_as(ctypes.POINTER(ctypes.c_double)))
        return result

    def select(self, work, target, candidates, budget, scale):
        work=np.ascontiguousarray(work,dtype=np.float32)
        target=np.ascontiguousarray(target,dtype=np.float32)
        out=np.empty(min(budget,len(candidates)),np.uint32)
        n=self.lib.paint_select(work.ctypes.data_as(ctypes.POINTER(ctypes.c_float)),target.ctypes.data_as(ctypes.POINTER(ctypes.c_float)),work.shape[1],work.shape[0],candidates.ctypes.data_as(ctypes.POINTER(ctypes.c_double)),len(candidates),budget,scale,out.ctypes.data_as(ctypes.POINTER(ctypes.c_uint32)))
        return out[:n]

    def clear(self):
        self.lib.paint_clear(self.handle)

    def close(self):
        if self.handle:
            self.lib.paint_free(self.handle)
            self.handle = None


class LoadedPainting:
    def __init__(self, width, height):
        cv2.setNumThreads(max(1,min(4,int(os.environ.get('LOADED_CV_THREADS','4')))))
        self.w, self.h = width, height
        self.fw, self.fh = 480, round(480*height/width)
        self.dis = cv2.DISOpticalFlow_create(cv2.DISOPTICAL_FLOW_PRESET_FAST)
        self.book = Painting(width, height, (self.fw, self.fh), 'transported', limit=14000)
        self.adaptive = AdaptiveBrushwork(width, height, (self.fw, self.fh))
        self.raster = Raster(width, height)
        self.adaptive.accept_filter = self.raster.select
        self.previous = None
        self.updates = 0
        self.cost = {}
        self.pool = ThreadPoolExecutor(max_workers=1)

    def compact_ids(self):
        # float32 ids must stay exact AND the three populations must not collide
        # during a long live session. Identity changes do not alter the painting.
        if self.book.next_id < 800_000 and self.adaptive.next_id < 8_000_000:
            return False
        self.book.strokes[:, 0] = np.arange(1, len(self.book.strokes)+1)
        self.book.next_id = len(self.book.strokes)+1
        self.adaptive.strokes[:, 0] = 2_000_000+np.arange(len(self.adaptive.strokes))
        self.adaptive.next_id = 2_000_000+len(self.adaptive.strokes)
        self.raster.clear()
        return True

    def paint(self, rgb, hour):
        if rgb.shape != (self.h, self.w, 3) or not np.isfinite(hour):
            raise ValueError('Invalid live paint input')
        started = time.perf_counter()
        self.compact_ids()
        small = cv2.resize(cv2.cvtColor(rgb, cv2.COLOR_RGB2GRAY), (self.fw, self.fh), interpolation=cv2.INTER_AREA)
        if self.previous is None:
            forward = backward = np.zeros((self.fh, self.fw, 2), np.float32)
        else:
            forward = self.dis.calc(self.previous, small, None)
            backward = self.dis.calc(small, self.previous, None)
        self.previous = small
        t = time.perf_counter(); self.cost['flow'] = t-started
        aw = min(self.w, 960)
        analysis = cv2.resize(rgb, (aw, round(self.h*aw/self.w)), interpolation=cv2.INTER_AREA)
        def update_adaptive():
            return self.adaptive.begin(self.raster.grade(rgb, hour%24), forward, backward)
        future = self.pool.submit(update_adaptive)
        marks = self.book.update(self.raster.grade(analysis, hour%24), forward, backward)
        t2 = time.perf_counter(); self.cost['base'] = t2-t
        retained = future.result()
        t = time.perf_counter(); self.cost['adaptive_update'] = t-t2
        paint = self.raster.paint(np.concatenate([marks, retained]))
        t2 = time.perf_counter(); self.cost['raster'] = t2-t
        if self.updates%3 == 0:
            for level, radius in enumerate([12., 6., 2.5]):
                added = self.adaptive.propose(paint, radius*self.w/1920, level,
                    [700, 1000, 1400][level] if self.updates == 0 else [140, 240, 600][level])
                if len(added):
                    paint = self.raster.paint(added, append=True)
        self.cost['refine'] = time.perf_counter()-t2
        self.updates += 1
        return paint


def read_exact(sock, size):
    buf = bytearray(size)
    view = memoryview(buf)
    while view:
        n = sock.recv_into(view)
        if not n:
            raise EOFError()
        view = view[n:]
    return buf


def main():
    w, h = map(int, sys.argv[1:3])
    if not (64 <= w <= 1920 and 36 <= h <= 2160 and w*h <= 4_147_200):
        raise ValueError('Unsupported live canvas')
    painting = LoadedPainting(w, h)
    # stdin is a Unix socket supplied by the Rust supervisor. If it disappears,
    # this worker exits instead of retaining a canvas or becoming an orphan.
    with socket.socket(fileno=0) as sock:
        sock.sendall(b'PAINT1')
        totals={};count=0
        try:
            while True:
                hour, = struct.unpack('<d', read_exact(sock, 8))
                rgb = np.frombuffer(read_exact(sock, w*h*3), np.uint8).reshape(h, w, 3)
                paint = painting.paint(rgb, hour)
                sock.sendall(memoryview(paint).cast('B'))
                count+=1
                for name,seconds in painting.cost.items():totals[name]=totals.get(name,0.)+seconds
                if count==30:
                    print('Loaded paint '+json.dumps({'updates':painting.updates,'base_marks':len(painting.book.strokes),
                        'base_coats':[int(np.count_nonzero(painting.book.strokes[:,11]==coat)) for coat in range(3)],
                        'base_area':int(np.sum(painting.book.strokes[:,3]*painting.book.strokes[:,4])),
                        'detail_marks':len(painting.adaptive.strokes),'mean_ms':{k:round(v*1000/count,2) for k,v in totals.items()}}),file=sys.stderr,flush=True)
                    totals={};count=0
        except EOFError:
            pass
        finally:
            painting.raster.close()


if __name__ == '__main__':
    main()
