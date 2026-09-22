import pathlib
import struct
import subprocess
import unittest

import cv2
import numpy as np

from adaptive import AdaptiveBrushwork
from render import grade, grade_reference


class AdaptiveTests(unittest.TestCase):
    def test_transport_only_moves_adaptive_marks_without_birth_retirement_or_failure_changes(self):
        book=AdaptiveBrushwork(160,96,(40,24))
        book.strokes=np.array([
            [2_000_000,40,32,8,3,0,.1,20,30,40,1,4],
            [2_000_001,80,48,8,3,0,.2,50,60,70,1,5],
            [2_000_002,112,64,8,3,0,.3,80,90,100,1,6]],np.float32)
        book.failures=np.array([0,1,0],np.float32)
        before=book.strokes.copy();failures=book.failures.copy()
        counters=(book.born,book.retired,book.next_id)
        forward=np.zeros((24,40,2),np.float32);forward[:]=[2,-1]

        changed=book.transport_only(forward,-forward,None,None)

        np.testing.assert_array_equal(book.strokes[:,0],before[:,0])
        np.testing.assert_array_equal(book.strokes[:,3:],before[:,3:])
        np.testing.assert_allclose(book.strokes[:,1:3],before[:,1:3]+[8,-4])
        self.assertTrue(changed.all())
        np.testing.assert_array_equal(book.failures,failures)
        self.assertEqual((book.born,book.retired,book.next_id),counters)

    def test_reference_grading_retains_the_established_day_and_night_palette(self):
        rng=np.random.default_rng(61)
        source=rng.integers(0,256,(64,128,3),np.uint8)
        for hour in [0,5.5,6.5,12,18.5,20,23]:
            difference=np.abs(grade(source,hour).astype(int)-grade_reference(source,hour).astype(int))
            self.assertLessEqual(difference.max(),1)

    def test_already_described_complexity_and_camera_noise_do_not_demand_more_marks(self):
        book=AdaptiveBrushwork(640,360,(160,90))
        yy,xx=np.indices((360,640))
        source=np.broadcast_to(np.where(((xx//12+yy//12)%2)[...,None],190,70),(360,640,3)).astype(np.uint8)
        zero=np.zeros((90,160,2),np.float32)
        book.begin(source,zero,zero)
        self.assertEqual(len(book.propose(source,2.5*book.scale,2,500)),0)
        rng=np.random.default_rng(62)
        noise=rng.integers(107,114,(360,640,3),np.uint8)
        book.begin(noise,zero,zero)
        self.assertEqual(len(book.propose(np.full_like(noise,110),2.5*book.scale,2,500)),0)

    def test_refinement_uses_smaller_marks_for_nested_shapes_and_reduces_actual_raster_error(self):
        native=pathlib.Path(__file__).parent/'native/target/release/stroke-study-raster'
        if not native.exists():
            self.skipTest('Build the native study raster before running this integration test')
        w,h=640,360
        source=np.empty((h,w,3),np.uint8);source[:]=[81,83,74]
        source[100:228,60:188]=[160,165,150]
        source[100:228,370:498]=[160,165,150]
        # Equal-size regions: one broad color plane, one containing narrow
        # contrasting forms. No object labels or semantic attention are given.
        for x in range(374,495,12):
            source[105:223,x:x+5]=[30,40,55]
        zero=np.zeros((90,160,2),np.float32)
        book=AdaptiveBrushwork(w,h,(160,90));book.begin(source,zero,zero)
        raster=subprocess.Popen([str(native),str(w),str(h)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
        def paint(marks):
            raster.stdin.write(struct.pack('<I',len(marks)));raster.stdin.write(marks.astype('<f4').tobytes());raster.stdin.flush()
            return np.frombuffer(raster.stdout.read(w*h*3),np.uint8).reshape(h,w,3)
        try:
            canvas=paint(np.empty((0,12),np.float32))
            before=book.error(canvas,2.5*book.scale)
            for level,radius in enumerate([12,6,2.5]):
                book.propose(canvas,radius*book.scale,level,1000)
                canvas=paint(book.strokes)
            self.assertLess(book.error(canvas,2.5*book.scale),before*.60)
            marks=book.strokes
            plain=marks[(marks[:,1]>60)&(marks[:,1]<188)]
            nested=marks[(marks[:,1]>370)&(marks[:,1]<498)]
            self.assertGreater(len(plain),0)
            self.assertGreater(len(nested),0)
            self.assertLess(np.median(nested[:,4]),np.median(plain[:,4]))
            self.assertGreater(len(nested),len(plain))
            retained=book.strokes.copy()
            book.begin(source,zero,zero)
            np.testing.assert_array_equal(book.strokes,retained)
        finally:
            raster.stdin.close();raster.stdout.close();raster.wait()


if __name__=='__main__':
    unittest.main()
