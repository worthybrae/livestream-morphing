import unittest
from unittest.mock import patch

import cv2
import numpy as np

from people import PersonDetector, PeopleAttention
from render import Painting


class AttentionTests(unittest.TestCase):
    def test_detector_handles_both_opencv_output_orderings_and_empty_results(self):
        classes=[np.zeros((1,n,80),np.float32) for n in [2704,676,169]]
        boxes=[np.zeros((1,n,32),np.float32) for n in [2704,676,169]]
        classes[0][0,700,0]=.9
        class Graph:
            def setInput(self,blob):pass
            def getUnconnectedOutLayersNames(self):return ()
            def forward(self,names):return self.outputs
        graph=Graph()
        with patch('cv2.dnn.readNet',return_value=graph):
            detector=PersonDetector('unused.onnx')
        graph.outputs=[v for pair in zip(classes,boxes) for v in pair]
        interleaved=detector.tile(np.zeros((416,416,3),np.uint8))
        self.assertEqual(len(interleaved),1)
        graph.outputs=classes+boxes
        grouped=detector.tile(np.zeros((416,416,3),np.uint8))
        np.testing.assert_array_equal(grouped,interleaved)
        classes[0][:]=0
        self.assertEqual(detector.tile(np.zeros((416,416,3),np.uint8)).shape,(0,5))

    def test_attention_follows_motion_survives_a_missed_detection_and_eventually_expires(self):
        tracker = PeopleAttention(128,96,(128,96))
        rng = np.random.default_rng(51)
        previous = rng.integers(0,255,(96,128),np.uint8)
        current = cv2.warpAffine(previous,np.array([[1,0,4],[0,1,2]],np.float32),(128,96))
        zero = np.zeros((96,128,2),np.float32)
        original = np.array([[40,24,60,64,.9]],np.float32)
        initial = tracker.update(previous,zero,zero,1/15,original)
        self.assertEqual(initial[44,50],1)
        self.assertEqual(initial[10,10],0)
        forward = zero.copy();forward[:]=[4,2]
        moved = tracker.update(current,forward,-forward,1/15)
        np.testing.assert_allclose(tracker.tracks[0,:4],original[0,:4]+[4,2,4,2])
        self.assertEqual(moved[46,54],1)
        missing = tracker.update(current,zero,zero,1/15,np.empty((0,5),np.float32))
        self.assertEqual(missing[46,54],1)
        for _ in range(35):
            empty = tracker.update(current,zero,zero,1/15)
        self.assertEqual(len(tracker.tracks),0)
        self.assertEqual(empty.max(),0)

    def test_redetection_preserves_track_identity_and_softens_box_jitter(self):
        tracker = PeopleAttention(128,96,(128,96))
        gray = np.zeros((96,128),np.uint8);flow=np.zeros((96,128,2),np.float32)
        tracker.update(gray,flow,flow,1/15,np.array([[40,20,60,70,.8]],np.float32))
        identity = tracker.tracks[0,6]
        tracker.update(gray,flow,flow,1/15,np.array([[42,21,63,72,.85]],np.float32))
        self.assertEqual(len(tracker.tracks),1)
        self.assertEqual(tracker.tracks[0,6],identity)
        self.assertGreater(tracker.tracks[0,0],40)
        self.assertLess(tracker.tracks[0,0],42)

    def test_detail_budget_goes_to_a_person_instead_of_busy_background(self):
        frame = np.full((160,960,3),110,np.uint8)
        yy,xx = np.indices((160,200))
        frame[:,:200] = np.where(((xx//4+yy//4)%2)[...,None],220,25)
        frame[35:130,600:616] = [30,60,95]
        frame[20:35,604:613] = [180,140,105]
        analysis=cv2.resize(frame,(480,80),interpolation=cv2.INTER_AREA)
        flow=np.zeros((40,240,2),np.float32)
        attention=np.zeros((40,240),np.float32);attention[4:35,148:156]=1
        original=Painting(960,160,(240,40),'transported')
        people=Painting(960,160,(240,40),'transported','people')
        original.update(analysis,flow,flow)
        people.update(analysis,flow,flow,frame,attention)
        np.testing.assert_array_equal(original.strokes,people.strokes[people.strokes[:,11]<3])
        marks=people.strokes[people.strokes[:,11]==3]
        self.assertGreater(len(marks),100)
        self.assertTrue((marks[:,1]>585).all())
        self.assertTrue((marks[:,1]<630).all())
        # A flat coat still gets coverage; it must not need an edge/glint to
        # replace the large underpaint that otherwise obscures small figures.
        body=marks[(marks[:,1]>602)&(marks[:,1]<614)&(marks[:,2]>50)&(marks[:,2]<110)]
        self.assertGreater(len(body),40)
        self.assertLess(np.median(body[:,7]),40)


if __name__=='__main__':
    unittest.main()
