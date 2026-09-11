# Optional person detector

Person-guided brushwork uses OpenCV Zoo's NanoDet-m-plus-1.5x_416, locally
through the OpenCV DNN CPU backend. No inference service is called and source
images are not uploaded. The model is checked approximately twice per second;
the existing optical flow carries attention regions between checks.

Upstream model and preprocessing/box-decoding reference:
https://github.com/opencv/opencv_zoo/tree/47534e27c9851bb1128ccc0102f1145e27f23f98/models/object_detection_nanodet

`people.py` adapts the preprocessing and distributional box decoding from the
upstream `nanodet.py`. The model and example are licensed under Apache-2.0;
the full upstream license is preserved in `licenses/nanodet-LICENSE`.

Artifact: `object_detection_nanodet_2022nov.onnx` (3,800,954 bytes).

SHA-256: `4b82da9944b88577175ee23a459dce2e26e6e4be573def65b1055dc2d9720186`.

Run `python3 artwork/stroke-memory/fetch_person_model.py` once to place the
verified model in the ignored `models/` directory. The default and selective
detail studies do not load this optional model.

Detection uses overlapping image crops to retain small pedestrians. Boxes are
anonymous attention areas, not segmentation masks or face/body keypoints.
Misses and false positives remain possible, especially in occlusions, foliage,
and poor lighting. Only actual source colors become paint; the detector does
not invent facial features or supply imagery to composite into the artwork.
