"""Persistent Loaded bristle worker. The live engine owns decoding and the wave.

A bounded local socket carries RGB frames; ctypes shares the raster canvas
without a second process or frame copy. No model, network access or disk state.
"""
import ctypes
import json
import os
import pathlib
import queue
import socket
import struct
import sys
import time
import threading
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass

import cv2
import numpy as np
from render import DRAWN, Painting, Piles, along, grade, grade_reference, palette
from adaptive import AdaptiveBrushwork

# Looks beyond the approved one, chosen with LOADED_LOOK: the study options
# (render.py) the live painter then runs with. Unset, nothing here is used.
LOOKS = {
    # Tone mixed in a few piles; borders round still things in their own colour,
    # swelling and thinning; the paint drifting and bending a little everywhere.
    'borders': dict(hand='contour', piles=6, drift=2.6, stir=2., ripple=1.6, border_tone='own', pulse=True,
                    border_weight=1.25, border_depth=.6, ripple_everywhere=.4, swell=.4, keen=2.2, reserve=400),
}


@dataclass(frozen=True)
class PaintTelemetry:
    planner: bool
    mutated_pixels: int
    transport_ms: float
    planner_ms: float
    dirty_ms: float
    worker_cpu_ms: float
    full_raster_ms: float
    worker_rss_bytes: int

    def pack(self):
        return struct.pack('<4sB3xQ5f4xQ', b'PNT2', int(self.planner),
            self.mutated_pixels, self.transport_ms, self.planner_ms,
            self.dirty_ms, self.worker_cpu_ms, self.full_raster_ms,
            self.worker_rss_bytes)


@dataclass(frozen=True)
class PreparedFrame:
    rgb: np.ndarray
    gray: np.ndarray
    forward: np.ndarray
    backward: np.ndarray
    prepare_ms: float
    previous_gray: np.ndarray = None


class PaintWorker:
    _stop = object()
    def __init__(self, painting):
        self.painting = painting
        self.to_prepare = queue.Queue(maxsize=1)
        self.to_commit = queue.Queue(maxsize=1)
        self.slot = threading.Semaphore(1)
        self.closed = threading.Event()
        self.preparer = threading.Thread(target=self._prepare, name='paint-prepare')
        self.preparer.start()

    def submit(self, rgb, hour):
        while not self.closed.is_set():
            try:
                self.to_prepare.put((rgb, hour), timeout=.1)
                return
            except queue.Full:
                pass
        raise RuntimeError('Paint worker closed')

    def finish(self):
        while not self.closed.is_set():
            try:
                self.to_prepare.put(self._stop, timeout=.1)
                return
            except queue.Full:
                pass

    def _prepare(self):
        while True:
            item = self.to_prepare.get()
            if item is self._stop:
                self.to_commit.put(self._stop)
                return
            rgb, hour = item
            self.slot.acquire()
            try:
                prepared = self.painting.prepare(rgb)
                self.to_commit.put((prepared, hour))
            except BaseException as error:
                self.to_commit.put(error)
                return

    def commit_next(self):
        item = self.to_commit.get()
        if item is self._stop:
            return None
        if isinstance(item, BaseException):
            self.slot.release()
            raise item
        prepared, hour = item
        try:
            return self.painting.paint_prepared(prepared, hour)
        finally:
            self.slot.release()

    def close(self):
        self.closed.set()
        try:
            self.to_prepare.get_nowait()
        except queue.Empty:
            pass
        try:
            self.slot.release()
        except ValueError:
            pass
        self.preparer.join(timeout=5)


def current_rss_bytes():
    with open('/proc/self/statm', encoding='ascii') as status:
        resident_pages = int(status.read().split()[1])
    return resident_pages * os.sysconf('SC_PAGE_SIZE')


class PaintFrameStats(ctypes.Structure):
    _fields_ = [('mutated_pixels', ctypes.c_uint64),
        ('dirty_micros', ctypes.c_uint64), ('full_raster_micros', ctypes.c_uint64),
        ('full_fallback', ctypes.c_uint8)]


class Raster:
    def __init__(self, width, height):
        suffix = '.dylib' if sys.platform == 'darwin' else '.so'
        path = os.environ.get('LOADED_RASTER', str(pathlib.Path(__file__).parent / ('native/target/release/libstroke_study_raster' + suffix)))
        self.lib = ctypes.CDLL(path)
        self.lib.paint_new.argtypes = [ctypes.c_size_t, ctypes.c_size_t, ctypes.c_bool]
        self.lib.paint_new.restype = ctypes.c_void_p
        self.lib.paint_frame.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_float), ctypes.c_size_t, ctypes.c_bool]
        self.lib.paint_frame.restype = ctypes.POINTER(ctypes.c_uint8)
        self.lib.paint_frame_incremental.argtypes = [ctypes.c_void_p,
            ctypes.POINTER(ctypes.c_float), ctypes.c_size_t, ctypes.POINTER(PaintFrameStats)]
        self.lib.paint_frame_incremental.restype = ctypes.POINTER(ctypes.c_uint8)
        self.lib.paint_clear.argtypes = self.lib.paint_free.argtypes = [ctypes.c_void_p]
        self.lib.paint_clear.restype = self.lib.paint_free.restype = None
        self.lib.paint_set_incremental_workers.argtypes = [ctypes.c_void_p,ctypes.c_size_t]
        self.lib.paint_set_incremental_workers.restype = None
        self.lib.paint_grade.argtypes = [ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_uint8), ctypes.c_size_t, ctypes.c_double, ctypes.POINTER(ctypes.c_double)]
        self.lib.paint_grade.restype = None
        self.lib.paint_select.argtypes = [ctypes.POINTER(ctypes.c_float),ctypes.POINTER(ctypes.c_float),ctypes.c_size_t,ctypes.c_size_t,ctypes.POINTER(ctypes.c_double),ctypes.c_size_t,ctypes.c_size_t,ctypes.c_double,ctypes.POINTER(ctypes.c_uint32)]
        self.lib.paint_select.restype = ctypes.c_size_t
        self.lib.paint_rgb_max_error.argtypes = [ctypes.POINTER(ctypes.c_float),ctypes.POINTER(ctypes.c_float),ctypes.POINTER(ctypes.c_float),ctypes.c_size_t]
        self.lib.paint_rgb_max_error.restype = None
        self.handle = self.lib.paint_new(width, height, True)
        if not self.handle:
            raise ValueError('Unsupported canvas dimensions')
        self.w, self.h = width, height
        self.incremental_dirty_ms = 0.
        self.incremental_full_fallback = False

    def paint(self, marks, append=False):
        marks = np.ascontiguousarray(marks, dtype=np.float32)
        ptr = self.lib.paint_frame(self.handle, marks.ctypes.data_as(ctypes.POINTER(ctypes.c_float)), len(marks), append)
        if not ptr:
            raise RuntimeError('Invalid brush packet')
        return np.ctypeslib.as_array(ptr, shape=(self.h*self.w*3,)).reshape(self.h, self.w, 3)

    def paint_incremental(self, marks):
        marks = np.ascontiguousarray(marks, dtype=np.float32)
        stats = PaintFrameStats()
        ptr = self.lib.paint_frame_incremental(self.handle,
            marks.ctypes.data_as(ctypes.POINTER(ctypes.c_float)), len(marks), ctypes.byref(stats))
        if not ptr:
            self.clear()
            started = time.perf_counter()
            ptr = self.lib.paint_frame(self.handle,
                marks.ctypes.data_as(ctypes.POINTER(ctypes.c_float)), len(marks), False)
            if not ptr:
                raise RuntimeError('Invalid brush packet')
            full_ms = (time.perf_counter()-started)*1000.
            self.incremental_dirty_ms = 0.
            self.incremental_full_fallback = True
            mutated = self.w*self.h
        else:
            full_ms = stats.full_raster_micros/1000.
            self.incremental_dirty_ms = stats.dirty_micros/1000.
            self.incremental_full_fallback = bool(stats.full_fallback)
            mutated = stats.mutated_pixels
        pixels = np.ctypeslib.as_array(ptr, shape=(self.h*self.w*3,)).reshape(self.h, self.w, 3)
        return pixels, mutated, full_ms

    def set_incremental_workers(self, workers):
        self.lib.paint_set_incremental_workers(self.handle, workers)

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

    def rgb_max_error(self, source, canvas, output):
        source=np.ascontiguousarray(source,dtype=np.float32)
        canvas=np.ascontiguousarray(canvas,dtype=np.float32)
        if source.shape!=canvas.shape or source.shape[:2]!=output.shape or source.shape[2:]!=(3,):
            raise ValueError('Invalid RGB error buffers')
        self.lib.paint_rgb_max_error(source.ctypes.data_as(ctypes.POINTER(ctypes.c_float)),canvas.ctypes.data_as(ctypes.POINTER(ctypes.c_float)),output.ctypes.data_as(ctypes.POINTER(ctypes.c_float)),output.size)

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
        look = os.environ.get('LOADED_LOOK', '')
        if look and look not in LOOKS:
            # A mistyped setting must not take the stream down: paint the approved look.
            print(f'Loaded paint: unknown LOADED_LOOK {look!r}, painting the approved look', file=sys.stderr, flush=True)
        self.look = LOOKS.get(look)
        self.book = Painting(width, height, (self.fw, self.fh), 'transported', limit=14000, **(self.look or {}))
        self.adaptive = AdaptiveBrushwork(width, height, (self.fw, self.fh))
        self.raster = Raster(width, height)
        self.adaptive.accept_filter = self.raster.select
        self.adaptive.error_filter = self.raster.rgb_max_error
        if self.look:
            if os.environ.get('LOADED_PAINT_MODE') == 'incremental':
                raise ValueError('This look is not painted incrementally yet')
            # The study painted 15 times a second. Its slow motions keep their pace
            # at the cadence the engine is asked for (it may paint slower under load).
            stride = float(os.environ.get('LOADED_PAINT_STRIDE', '3'))
            self.book.rate = 30000/1001/min(12., max(2., stride if np.isfinite(stride) else 3.))
            self.covered = np.zeros((height, width), np.uint8)
            self.adaptive.error_filter = self.forgiving
            self.across, self.down = np.meshgrid(np.arange(width, dtype=np.float32), np.arange(height, dtype=np.float32))
        self.previous = None
        self.updates = 0
        self.cost = {}
        self.pool = ThreadPoolExecutor(max_workers=1)
        self.incremental = os.environ.get('LOADED_PAINT_MODE') == 'incremental'
        self.planner_stride = max(2, min(6, int(os.environ.get('LOADED_PLANNER_STRIDE', '3'))))

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

    def forgiving(self, reference, working, error):
        # The finishing pass must not paint over a drawn line, nor "correct" the
        # lean each stroke takes from its pile.
        self.raster.rgb_max_error(reference, working, error)
        np.subtract(error, 7., out=error)
        np.maximum(error, 0., out=error)
        error[self.covered>0] = 0

    def cover(self, lines):
        # Where the drawn lines run, a little wider than they are.
        self.covered[:] = 0
        if len(lines):
            routes = np.round(np.stack([along(lines, u) for u in np.linspace(-1, 1, 9)], axis=1)).astype(np.int32)
            for route, width in zip(routes, lines[:,4]):
                cv2.polylines(self.covered, [route], False, 1, int(round(width*2+6)))

    def shown(self, paint):
        # The paint drifts and bends as the study's does: a little everywhere, more
        # inside bordered things. The raster's own canvas is left as it was painted.
        if self.book.towed is None:
            return paint
        started = time.perf_counter()
        shift = cv2.resize(self.book.towed, (self.w, self.h), interpolation=cv2.INTER_LINEAR)
        moved = cv2.remap(paint, self.across-shift[:,:,0], self.down-shift[:,:,1], cv2.INTER_LINEAR,
                          borderMode=cv2.BORDER_REPLICATE)
        self.cost['drift'] = time.perf_counter()-started
        return moved

    def prepare(self, rgb):
        if rgb.shape != (self.h, self.w, 3):
            raise ValueError('Invalid live paint input')
        started = time.perf_counter()
        small = cv2.resize(cv2.cvtColor(rgb, cv2.COLOR_RGB2GRAY), (self.fw, self.fh), interpolation=cv2.INTER_AREA)
        previous = self.previous
        if previous is None:
            forward = backward = np.zeros((self.fh, self.fw, 2), np.float32)
        else:
            forward = self.dis.calc(previous, small, None)
            backward = self.dis.calc(small, previous, None)
        self.previous = small
        return PreparedFrame(rgb, small, forward, backward,
            (time.perf_counter()-started)*1000., previous)

    def paint(self, rgb, hour):
        return self.paint_prepared(self.prepare(rgb), hour)

    def paint_prepared(self, prepared, hour):
        if not np.isfinite(hour):
            raise ValueError('Invalid live paint input')
        cpu_started = time.process_time()
        self.compact_ids()
        rgb, forward, backward = prepared.rgb, prepared.forward, prepared.backward
        self.cost['flow'] = prepared.prepare_ms/1000.
        t = time.perf_counter()
        planner = not self.incremental or self.updates%self.planner_stride == 0
        if not planner:
            transport_started = time.perf_counter()
            self.book.transport_only(forward, backward, prepared.previous_gray, prepared.gray)
            self.adaptive.transport_only(forward, backward, prepared.previous_gray, prepared.gray)
            marks = np.concatenate([self.book.ground, self.book.strokes, self.adaptive.strokes])
            transport_ms = (time.perf_counter()-transport_started)*1000.
            native_started = time.perf_counter()
            self.raster.set_incremental_workers(1)
            paint, mutated, full_ms = self.raster.paint_incremental(marks)
            self.cost.update(base=0., adaptive_update=0., raster=time.perf_counter()-native_started,
                             refine=0.)
            self.updates += 1
            return paint, PaintTelemetry(False, mutated,
                self.cost['flow']*1000.+transport_ms, 0., self.raster.incremental_dirty_ms,
                (time.process_time()-cpu_started)*1000., full_ms, current_rss_bytes())
        aw = min(self.w, 960)
        analysis = cv2.resize(rgb, (aw, round(self.h*aw/self.w)), interpolation=cv2.INTER_AREA)
        if self.look and self.book.piles is None:
            # Mixed here, before the finishing pass reads them from its own thread.
            self.book.piles = Piles(self.raster.grade(analysis, hour%24), self.book.pile_count)
        def update_adaptive():
            reference = self.raster.grade(rgb, hour%24)
            if self.look:
                reference = self.book.piles.repaint(reference)
            return self.adaptive.begin(reference, forward, backward,
                (prepared.previous_gray, prepared.gray))
        future = self.pool.submit(update_adaptive)
        marks = self.book.update(self.raster.grade(analysis, hour%24), forward, backward,
            motion_grays=(prepared.previous_gray, prepared.gray))
        t2 = time.perf_counter(); self.cost['base'] = t2-t
        retained = future.result()
        t = time.perf_counter(); self.cost['adaptive_update'] = t-t2
        if self.look:
            retained = retained.copy()
            retained[:,7:10] += Piles.lean(retained[:,6], retained[:,7:10])
            drawn = marks[:,11]>=DRAWN  # drawn lines go on last, over the finishing marks
            self.cover(marks[drawn])
            marks, retained = marks[~drawn], np.concatenate([retained, marks[drawn]])
        paint = self.raster.paint(np.concatenate([marks, retained]))
        t2 = time.perf_counter(); self.cost['raster'] = t2-t
        full_raster_ms = self.cost['raster']*1000.
        if self.incremental or self.updates%3 == 0:
            for level, radius in enumerate([12., 6., 2.5]):
                added = self.adaptive.propose(paint, radius*self.w/1920, level,
                    [700, 1000, 1400][level] if self.updates == 0 else [140, 240, 600][level])
                if len(added):
                    native_started = time.perf_counter()
                    paint = self.raster.paint(added, append=True)
                    full_raster_ms += (time.perf_counter()-native_started)*1000.
        self.cost['refine'] = time.perf_counter()-t2
        if self.incremental:
            marks = np.concatenate([marks, self.adaptive.strokes])
            self.raster.set_incremental_workers(2)
            paint, mutated, incremental_full_ms = self.raster.paint_incremental(marks)
            full_raster_ms += incremental_full_ms
            planner_ms = (self.cost['base']+self.cost['adaptive_update']+
                self.cost['refine'])*1000.-(full_raster_ms-incremental_full_ms)
            self.updates += 1
            return paint, PaintTelemetry(True, mutated, self.cost['flow']*1000.,
                max(0., planner_ms), self.raster.incremental_dirty_ms,
                (time.process_time()-cpu_started)*1000., full_raster_ms, current_rss_bytes())
        self.updates += 1
        return self.shown(paint) if self.look else paint


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
    incremental = os.environ.get('LOADED_PAINT_MODE') == 'incremental'
    # stdin is a Unix socket supplied by the Rust supervisor. If it disappears,
    # this worker exits instead of retaining a canvas or becoming an orphan.
    with socket.socket(fileno=0) as sock:
        sock.sendall(b'PAINT2' if incremental else b'PAINT1')
        totals={};count=0
        worker = PaintWorker(painting) if incremental else None
        reader = None
        if incremental:
            def read_frames():
                try:
                    while True:
                        hour, = struct.unpack('<d', read_exact(sock, 8))
                        rgb = np.frombuffer(read_exact(sock, w*h*3), np.uint8).reshape(h, w, 3)
                        worker.submit(rgb, hour)
                except EOFError:
                    worker.finish()
            reader = threading.Thread(target=read_frames, name='paint-reader')
            reader.start()
        try:
            while True:
                if incremental:
                    result = worker.commit_next()
                    if result is None:
                        break
                    paint, telemetry = result
                    sock.sendall(telemetry.pack())
                else:
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
            if incremental:
                try:
                    sock.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass
                worker.close()
                reader.join(timeout=5)
            painting.raster.close()


if __name__ == '__main__':
    main()
