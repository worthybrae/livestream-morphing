import unittest
import numpy as np
import cv2
from motion import transport
from render import Painting


class TransportTests(unittest.TestCase):
    def detail_scene(self):
        source = np.full((160,960,3),110,np.uint8)
        source[30:125,400:404] = 25
        source[80:84,550:800] = 235
        return source, cv2.resize(source,(480,80),interpolation=cv2.INTER_AREA), np.zeros((40,240,2),np.float32)

    def test_finishing_recovers_narrow_features_without_repainting_quiet_areas_or_base_coats(self):
        source, analysis, flow = self.detail_scene()
        original = Painting(960,160,(240,40),'transported')
        detailed = Painting(960,160,(240,40),'transported','selective')
        original.update(analysis,flow,flow)
        detailed.update(analysis,flow,flow,source)
        np.testing.assert_array_equal(original.strokes,detailed.strokes[detailed.strokes[:,11]<3])
        accents = detailed.strokes[detailed.strokes[:,11]==3]
        self.assertGreater(len(accents),30)
        self.assertTrue((accents[:,1]>390).all())
        self.assertTrue((accents[:,7]<40).any())
        self.assertTrue((accents[:,7]>220).any())

    def test_fine_features_retain_identity_without_duplicate_births_while_static(self):
        source, analysis, flow = self.detail_scene()
        book = Painting(960,160,(240,40),'transported','selective')
        book.update(analysis,flow,flow,source)
        initial = book.strokes[book.strokes[:,11]==3].copy()
        for _ in range(8):
            book.update(analysis,flow,flow,source)
        accents = book.strokes[book.strokes[:,11]==3]
        np.testing.assert_array_equal(initial,accents)

    def test_estimated_flow_transports_real_image_features_in_the_forward_direction(self):
        rng=np.random.default_rng(91)
        previous=cv2.GaussianBlur(rng.integers(0,256,(96,128),np.uint8),(5,5),0)
        current=cv2.warpAffine(previous,np.array([[1,0,3],[0,1,2]],np.float32),(128,96))
        dis=cv2.DISOpticalFlow_create(cv2.DISOPTICAL_FLOW_PRESET_FAST)
        forward=dis.calc(previous,current,None);backward=dis.calc(current,previous,None)
        points=np.array([[40.,35.],[60.,50.],[85.,65.]],np.float32)
        moved,trusted=transport(points,forward,backward,previous,current)
        np.testing.assert_allclose(moved,[[43,37],[63,52],[88,67]],atol=.4)
        self.assertTrue(trusted.all())

    def test_background_motion_cannot_drag_a_stationary_foreground_mark(self):
        points = np.array([[12., 15.]],np.float32)
        previous=np.full((32,40),180,np.uint8);previous[:,11:14]=30
        current=previous.copy()
        forward=np.zeros((32,40,2),np.float32);forward[:]=[4,0]
        moved, trusted=transport(points,forward,-forward,previous,current)
        self.assertFalse(trusted[0])

    def test_static_scene_does_not_keep_spawning_duplicate_strokes(self):
        book = Painting(192,108,(48,27),'transported')
        frame = np.full((108,192,3),110,np.uint8)
        flow = np.zeros((27,48,2),np.float32)
        initial = len(book.update(frame,flow,flow))
        for _ in range(8):
            book.update(frame,flow,flow)
        self.assertLessEqual(len(book.strokes),initial*1.05)

    def test_strokes_follow_translation_without_changing_order(self):
        points = np.array([[8., 9.], [20., 12.], [26., 21.]], np.float32)
        forward = np.zeros((32, 40, 2), np.float32)
        forward[:] = [3., -2.]
        backward = -forward
        moved, trusted = transport(points, forward, backward)
        np.testing.assert_allclose(moved, [[11., 7.], [23., 10.], [29., 19.]])
        self.assertTrue(trusted.all())

    def test_occluded_and_outside_strokes_are_not_carried_into_another_object(self):
        points = np.array([[8., 9.], [20., 12.], [38., 21.]], np.float32)
        forward = np.zeros((32, 40, 2), np.float32)
        forward[:] = [3., 0.]
        backward = -forward
        backward[8:15, 20:26] = [0., 0.]
        _, trusted = transport(points, forward, backward)
        np.testing.assert_array_equal(trusted, [True, False, False])


if __name__ == '__main__':
    unittest.main()
