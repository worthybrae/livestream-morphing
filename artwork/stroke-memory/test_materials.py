"""Exercise the native material selector and texture stability through its pipe."""
import pathlib
import struct
import subprocess
import unittest

import numpy as np


class MaterialTests(unittest.TestCase):
    def render(self, material, marks):
        native=pathlib.Path(__file__).parent/'native/target/release/stroke-study-raster'
        if not native.exists():
            self.skipTest('Build the native study raster first')
        packet=struct.pack('<I',len(marks))+marks.astype('<f4').tobytes()
        result=subprocess.run([str(native),'180','100',material],input=packet,
                              stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True)
        return np.frombuffer(result.stdout,np.uint8).reshape(100,180,3)

    def test_material_selection_changes_paint_and_transports_texture_without_crawling(self):
        # A broad deposit plus a tiny bright accent: materials must change
        # the former without erasing the small feature or sliding its texture.
        marks=np.array([[1,65,45,40,12,.27,.61,145,106,79,1,0],
                        [2,150,75,3,1.2,-.6,.31,210,213,201,1,6]],np.float32)
        baseline=self.render('plain',marks)
        for material in ['loaded','scumble','swept']:
            with self.subTest(material=material):
                painted=self.render(material,marks)
                self.assertGreater(np.abs(painted.astype(int)-baseline.astype(int)).sum(),10000)
                self.assertGreater(painted[70:81,145:156].max(),175)
                np.testing.assert_array_equal(painted[70:81,145:156],baseline[70:81,145:156])
                np.testing.assert_array_equal(painted,self.render(material,marks))
                moved=marks.copy();moved[:,1]+=3
                translated=self.render(material,moved)
                np.testing.assert_array_equal(painted[:,:-3],translated[:,3:])


if __name__=='__main__':unittest.main()
