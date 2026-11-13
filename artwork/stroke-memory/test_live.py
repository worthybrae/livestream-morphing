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
from unittest import mock
import cv2
import numpy as np
from live import (PaintFrameStats, PaintTelemetry, PaintWorker, PreparedFrame,
                  Raster, LoadedPainting, current_rss_bytes)
from render import Painting, grade, grade_reference
from adaptive import AdaptiveBrushwork

class LiveTests(unittest.TestCase):
    def test_paint_worker_preparation_is_ordered_bounded_and_joined(self):
        class FakePainting:
            def __init__(self):
                self.prepared = self.committed = self.maximum_ahead = 0
            def prepare(self, rgb):
                self.prepared += 1
                self.maximum_ahead = max(self.maximum_ahead, self.prepared-self.committed)
                return PreparedFrame(rgb, rgb[...,0], None, None, 0.)
            def paint_prepared(self, frame, hour):
                self.committed += 1
                return int(frame.rgb[0,0,0]), hour
        painting = FakePainting(); worker = PaintWorker(painting)
        def produce():
            for number in range(6):
                worker.submit(np.full((1,1,3), number, np.uint8), float(number))
            worker.finish()
        producer = __import__('threading').Thread(target=produce)
        producer.start()
        responses = []
        while True:
            response = worker.commit_next()
            if response is None:
                break
            responses.append(response)
        producer.join(timeout=2); worker.close()

        self.assertEqual(responses, [(n, float(n)) for n in range(6)])
        self.assertLessEqual(painting.maximum_ahead, 1)
        self.assertFalse(producer.is_alive())
        self.assertFalse(worker.preparer.is_alive())

    def test_incremental_mode_paints_every_frame_and_plans_at_bounded_keyframes(self):
        def trusted_translation(points, forward, backward, previous=None, current=None):
            return points + np.array([1., 0.], np.float32), np.ones(len(points), bool)
        with mock.patch.dict(os.environ, {'LOADED_PAINT_MODE':'incremental',
                                           'LOADED_PLANNER_STRIDE':'3'}), \
             mock.patch('render.transport', side_effect=trusted_translation), \
             mock.patch('adaptive.transport', side_effect=trusted_translation):
            painting = LoadedPainting(160, 90)
            calls = []
            native = painting.raster.paint_incremental
            def capture(marks):
                calls.append(marks.copy())
                return native(marks)
            painting.raster.paint_incremental = capture
            frame = np.full((90, 160, 3), [90, 120, 150], np.uint8)
            telemetry = []
            for _ in range(6):
                _, update = painting.paint(frame, 15.)
                telemetry.append(update)

        self.assertEqual(len(calls), 6)
        self.assertEqual([item.planner for item in telemetry],
                         [True, False, False, True, False, False])
        identities = [packet[(packet[:,0] > 0) & (packet[:,0] < 1_000_000)][0, :3]
                      for packet in calls]
        self.assertEqual(len({tuple(mark) for mark in identities}), 6)
        self.assertTrue(all(item.mutated_pixels > 0 for item in telemetry))

    def test_transport_only_frame_does_not_grade_full_resolution_sources(self):
        def trusted_translation(points, forward, backward, previous=None, current=None):
            return points + np.array([1., 0.], np.float32), np.ones(len(points), bool)
        with mock.patch.dict(os.environ, {'LOADED_PAINT_MODE':'incremental',
                                           'LOADED_PLANNER_STRIDE':'3'}), \
             mock.patch('render.transport', side_effect=trusted_translation), \
             mock.patch('adaptive.transport', side_effect=trusted_translation):
            painting = LoadedPainting(160, 90)
            frame = np.full((90, 160, 3), [90, 120, 150], np.uint8)
            painting.paint(frame, 15.)
            prepared = painting.prepare(frame)
            with mock.patch.object(painting.raster, 'grade',
                                   side_effect=AssertionError('redundant grade')), \
                 mock.patch('live.cv2.resize',
                            side_effect=AssertionError('redundant analysis resize')):
                _, telemetry = painting.paint_prepared(prepared, 15.)
        self.assertFalse(telemetry.planner)

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

    def test_native_rgb_max_error_matches_opencv_for_odd_tail(self):
        raster=Raster(37,19)
        try:
            rng=np.random.default_rng(81)
            source=rng.random((19,37,3),dtype=np.float32)*255
            canvas=rng.random((19,37,3),dtype=np.float32)*255
            expected=np.max(cv2.absdiff(source,canvas),axis=2)
            actual=np.empty((19,37),np.float32)
            raster.rgb_max_error(source,canvas,actual)
            np.testing.assert_array_equal(actual,expected)
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

    @staticmethod
    def thing_on_a_ground(width=640, height=360):
        # Pale blocks on a darker ground: things the borders look outlines.
        scene = np.full((height, width, 3), [90, 100, 110], np.uint8)
        scene[height//9:height//3, width//16:width//4] = [225, 215, 185]
        scene[height//2:height*7//9, width//2:width*11//16] = [225, 215, 185]
        return cv2.GaussianBlur(scene, (0, 0), 1.2)

    def test_no_look_is_painted_unless_asked_for(self):
        p = LoadedPainting(160, 90)
        try:
            p.paint(np.full((90, 160, 3), [90, 120, 150], np.uint8), 15.)
            self.assertIsNone(p.look)
            self.assertEqual(p.book.hand, 'approved')
            self.assertIsNone(p.book.piles)
            self.assertEqual(p.book.reserve, 0)
            self.assertEqual(p.adaptive.error_filter, p.raster.rgb_max_error)
        finally:
            p.pool.shutdown(); p.raster.close()

    def test_borders_look_draws_its_lines_last_and_drifts_the_paint(self):
        scene = self.thing_on_a_ground()
        with mock.patch.dict(os.environ, {'LOADED_LOOK': 'borders'}):
            os.environ.pop('LOADED_PAINT_MODE', None)
            p = LoadedPainting(640, 360)
        try:
            packets = []
            native = p.raster.paint
            def capture(marks, append=False):
                if not append:
                    packets.append(np.array(marks, np.float32))
                return native(marks, append)
            p.raster.paint = capture
            for _ in range(5):
                shown = p.paint(scene, 15.)
            canvas = native(packets[-1]).copy()
        finally:
            p.pool.shutdown(); p.raster.close()

        self.assertEqual(p.book.hand, 'contour')
        self.assertIsNotNone(p.book.piles)
        last = packets[-1]
        lines = last[:,11] >= 9
        self.assertGreater(int(lines.sum()), 0)
        # Every line comes after everything else, the finishing marks included.
        self.assertTrue(lines[int(np.argmax(lines)):].all())
        self.assertTrue((last[lines,0] >= 9_000_000).all())
        self.assertTrue((last[:,0] < 2**24).all())
        self.assertEqual(len(np.unique(last[:,0])), len(last))
        self.assertEqual((shown.shape, shown.dtype), ((360, 640, 3), np.uint8))
        self.assertIsNotNone(p.book.towed)
        self.assertFalse(np.array_equal(shown, canvas))

    def test_a_reserve_leaves_room_for_lines_in_a_full_population(self):
        scene = self.thing_on_a_ground()
        zero = np.zeros((90, 160, 2), np.float32)
        def lines_after(reserve):
            p = Painting(640, 360, (160, 90), 'transported', limit=400, hand='contour', reserve=reserve)
            for _ in range(3):
                p.update(scene, zero, zero)
            self.assertLessEqual(len(p.strokes), 400)
            return int((p.strokes[:,11] >= 9).sum())
        self.assertEqual(lines_after(0), 0)
        self.assertGreater(lines_after(80), 0)

    def test_a_slower_pace_looks_at_the_scene_less_often_and_still_keeps_lines(self):
        scene = self.thing_on_a_ground()
        zero = np.zeros((90, 160, 2), np.float32)
        def painted(pace):
            p = Painting(640, 360, (160, 90), 'transported', hand='contour', pace=pace)
            looks = []
            fields = p.fields
            p.fields = lambda *a, **k: looks.append(p.turn) or fields(*a, **k)
            for _ in range(13):
                p.update(scene, zero, zero)
            self.assertEqual(len(p.doubt), len(p.strokes))
            return p, looks
        every, looks = painted(1)
        self.assertEqual(looks, list(range(13)))
        self.assertEqual(len(every.thing_counts), 4)
        seldom, looks = painted(4)
        self.assertEqual(looks, [0, 4, 8, 12])
        self.assertEqual(len(seldom.thing_counts), 2)
        self.assertGreater(int((seldom.strokes[:,11] >= 9).sum()), 0)
        self.assertEqual(int((seldom.strokes[:,11] >= 9).sum()), int((every.strokes[:,11] >= 9).sum()))

    def test_native_piles_match_the_python_repaint(self):
        from render import Piles
        rng = np.random.default_rng(5)
        piles = Piles(cv2.GaussianBlur(rng.integers(0, 256, (90, 160, 3), dtype=np.uint8), (0, 0), 2), 6)
        raster = Raster(192, 108)
        try:
            self.assertTrue(raster.has_piles)
            for shape in ((1, 1, 3), (7, 5, 3), (1080, 1920, 3)):
                image = rng.integers(0, 256, shape, dtype=np.uint8)
                np.testing.assert_array_equal(raster.piles(image, piles), piles.repaint(image))
        finally:
            raster.close()

    def test_borders_look_keeps_the_lamp_light_at_night(self):
        from render import palette
        with mock.patch.dict(os.environ, {'LOADED_LOOK': 'borders'}):
            os.environ.pop('LOADED_PAINT_MODE', None)
            p = LoadedPainting(160, 90)
        try:
            for hour in (0.02, 3., 22.5):
                night, tint = p.tones(hour)
                self.assertEqual(night, 0.)
                np.testing.assert_allclose(tint, [15, 12, -9], atol=1e-9)
            # By day nothing differs from the stream's own colours.
            for hour in (9., 12., 15., 17.9):
                night, tint = p.tones(hour)
                self.assertEqual((night, palette(hour)[0]), (0., 0.))
                np.testing.assert_allclose(tint, palette(hour)[1], atol=1e-9)
            # Sunset still warms the picture: its tint is mostly kept.
            self.assertGreater(p.tones(18.5)[1][0], 30)
        finally:
            p.pool.shutdown(); p.raster.close()
        plain = LoadedPainting(160, 90)
        try:
            self.assertIsNone(plain.tones(0.02))
        finally:
            plain.pool.shutdown(); plain.raster.close()

    def test_an_unknown_look_paints_the_approved_look(self):
        with mock.patch.dict(os.environ, {'LOADED_LOOK': 'nonesuch'}), mock.patch('sys.stderr'):
            p = LoadedPainting(160, 90)
        try:
            self.assertIsNone(p.look)
            self.assertEqual(p.book.hand, 'approved')
        finally:
            p.pool.shutdown(); p.raster.close()

    def test_borders_look_refuses_incremental_painting(self):
        with mock.patch.dict(os.environ, {'LOADED_LOOK': 'borders', 'LOADED_PAINT_MODE': 'incremental'}):
            with self.assertRaises(ValueError):
                LoadedPainting(160, 90)

    def test_borders_look_keeps_time_at_the_requested_cadence(self):
        with mock.patch.dict(os.environ, {'LOADED_LOOK': 'borders', 'LOADED_PAINT_STRIDE': '2.5'}):
            os.environ.pop('LOADED_PAINT_MODE', None)
            p = LoadedPainting(160, 90)
        try:
            self.assertAlmostEqual(p.book.rate, 30000/1001/2.5, places=6)
        finally:
            p.pool.shutdown(); p.raster.close()

if __name__=='__main__':unittest.main()
