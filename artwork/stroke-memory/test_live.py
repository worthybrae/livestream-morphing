"""Production port checks: material parity, bounded identities, and native math."""
import copy
import ctypes
import os
import pathlib
import socket
import struct
import subprocess
import sys
import unittest
import cv2
import numpy as np
from live import PaintFrameStats, PaintTelemetry, Raster, LoadedPainting, current_rss_bytes
from render import Painting, grade, grade_reference
from adaptive import AdaptiveBrushwork

class LiveTests(unittest.TestCase):
    def test_incremental_raster_clears_and_retries_full_once_after_native_failure(self):
        class FakeNative:
            def __init__(self):
                self.incremental_calls = self.clear_calls = self.full_calls = 0
                self.pixels = (ctypes.c_uint8 * 12)(*range(12))
            def paint_frame_incremental(self, handle, marks, count, stats):
                self.incremental_calls += 1
                return ctypes.POINTER(ctypes.c_uint8)()
            def paint_clear(self, handle):
                self.clear_calls += 1
            def paint_frame(self, handle, marks, count, append):
                self.full_calls += 1
                return ctypes.cast(self.pixels, ctypes.POINTER(ctypes.c_uint8))
        raster = Raster.__new__(Raster)
        raster.lib, raster.handle, raster.w, raster.h = FakeNative(), 1, 2, 2

        pixels, mutated, full_ms = raster.paint_incremental(np.empty((0, 12), np.float32))

        self.assertEqual(raster.lib.incremental_calls, 1)
        self.assertEqual(raster.lib.clear_calls, 1)
        self.assertEqual(raster.lib.full_calls, 1)
        self.assertEqual(mutated, 4)
        self.assertGreaterEqual(full_ms, 0.)
        np.testing.assert_array_equal(pixels.ravel(), np.arange(12, dtype=np.uint8))

    def test_paint2_telemetry_is_fixed_width_and_round_trips(self):
        value = PaintTelemetry(True, 1234, 1.5, 2.5, 3.5, 4.5, 5.5, 4096)

        packet = value.pack()

        self.assertEqual(len(packet), 48)
        self.assertEqual(struct.unpack('<4sB3xQ5f4xQ', packet),
            (b'PNT2', 1, 1234, 1.5, 2.5, 3.5, 4.5, 5.5, 4096))

    def test_current_rss_is_positive_and_page_aligned(self):
        resident = current_rss_bytes()

        self.assertGreater(resident, 0)
        self.assertEqual(resident % os.sysconf('SC_PAGE_SIZE'), 0)

    def test_incremental_worker_sends_paint2_telemetry_before_pixels(self):
        client, peer = socket.socketpair()
        environment = os.environ.copy()
        environment['LOADED_PAINT_MODE'] = 'incremental'
        process = subprocess.Popen(
            [sys.executable, '-u', str(pathlib.Path(__file__).with_name('live.py')), '64', '36'],
            stdin=peer, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, env=environment)
        peer.close()
        try:
            self.assertEqual(client.recv(6), b'PAINT2')
            client.sendall(struct.pack('<d', 15.0) + bytes([90, 100, 110]) * (64 * 36))
            header = bytearray()
            while len(header) < 48:
                header.extend(client.recv(48 - len(header)))
            values = struct.unpack('<4sB3xQ5f4xQ', header)
            self.assertEqual(values[0], b'PNT2')
            self.assertEqual(values[1], 1)
            pixels = bytearray()
            while len(pixels) < 64 * 36 * 3:
                pixels.extend(client.recv(64 * 36 * 3 - len(pixels)))
            self.assertEqual(len(pixels), 64 * 36 * 3)
        finally:
            client.close()
            process.wait(timeout=5)
            process.stderr.close()

    def test_base_strokes_replenish_without_exceeding_the_live_budget(self):
        p=Painting(320,180,(160,90),'transported',limit=200)
        zero=np.zeros((90,160,2),np.float32)
        rng=np.random.default_rng(19)
        for _ in range(20):
            image=cv2.GaussianBlur(rng.integers(0,256,(180,320,3),dtype=np.uint8),(0,0),1)
            marks=p.update(image,zero,zero)
            self.assertLessEqual(len(p.strokes),200)
            self.assertGreater(len(marks),0)
        self.assertGreater(p.retired,0)
        self.assertGreater(p.births,200)

    def test_fused_palette_tracks_original_through_day_and_night(self):
        raster=Raster(192,108)
        try:
            rgb=np.random.default_rng(28).integers(0,256,(108,192,3),dtype=np.uint8)
            for hour in [0,4.5,6.5,12,18.5,21,23.999]:
                result=raster.grade(rgb,hour)
                for reference in [grade(rgb,hour),grade_reference(rgb,hour)]:
                    self.assertLessEqual(int(np.abs(result.astype(int)-reference).max()),1)
        finally:raster.close()

    def test_native_acceptance_preserves_brush_effort_decisions(self):
        w,h=320,180;raster=Raster(w,h)
        rng=np.random.default_rng(51)
        image=cv2.GaussianBlur(rng.integers(0,256,(h,w,3),dtype=np.uint8),(0,0),3)
        image[45:115,120:160]=[230,60,80]
        zero=np.zeros((90,160,2),np.float32)
        a=AdaptiveBrushwork(w,h,(160,90));a.begin(image,zero,zero)
        b=copy.deepcopy(a);b.accept_filter=raster.select
        canvas=np.full((h,w,3),90,np.uint8)
        try:
            for level,radius in enumerate([12,6,2.5]):
                aa=a.propose(canvas,radius*w/1920,level,100)
                bb=b.propose(canvas,radius*w/1920,level,100)
                np.testing.assert_array_equal(aa,bb)
        finally:raster.close()

    def test_long_running_id_compaction_keeps_every_painted_pixel(self):
        p=LoadedPainting(320,180)
        try:
            rgb=cv2.GaussianBlur(np.random.default_rng(2).integers(0,256,(180,320,3),dtype=np.uint8),(0,0),3)
            p.paint(rgb,15)
            marks=np.concatenate([p.book.ground,p.book.strokes,p.adaptive.strokes])
            before=p.raster.paint(marks).copy()
            p.book.next_id=900_000
            self.assertTrue(p.compact_ids())
            marks=np.concatenate([p.book.ground,p.book.strokes,p.adaptive.strokes])
            after=p.raster.paint(marks)
            np.testing.assert_array_equal(before,after)
            self.assertEqual(len(np.unique(marks[:,0])),len(marks))
            self.assertLess(p.book.next_id,800_000)
        finally:
            p.pool.shutdown();p.raster.close()

if __name__=='__main__':unittest.main()
