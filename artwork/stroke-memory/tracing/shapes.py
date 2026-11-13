#!/usr/bin/env python3
"""Outline every object in the prepared scene, once, with Segment Anything 2.

Reads plate.jpg (and plate-day.jpg when there is one) from the scene folder and
writes shapes.json: the outline of each thing the model can tell apart, smallest
first. The tracing page lights these up under the pointer so they can be picked.

Run after prepare.py, with the optional environment (see sam.py).
"""
import argparse
import json
import pathlib
import time

import cv2
import numpy as np

import sam


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--scene', type=pathlib.Path, default=sam.HERE.parent/'scene')
    ap.add_argument('--device', default='cpu')
    ap.add_argument('--points', type=int, default=48, help='how finely the view is probed, per side')
    args = ap.parse_args()
    from sam2.automatic_mask_generator import SAM2AutomaticMaskGenerator
    finder = SAM2AutomaticMaskGenerator(sam.model(args.device), points_per_side=args.points, points_per_batch=64,
                                        pred_iou_thresh=.75, stability_score_thresh=.9, min_mask_region_area=120)
    kept, small = [], []
    for view, name in (('night', 'plate.jpg'), ('day', 'plate-day.jpg')):
        image = cv2.imread(str(args.scene/name))
        if image is None:
            continue
        height, width = image.shape[:2]
        began = time.perf_counter()
        masks = finder.generate(sam.seen(image))
        took = time.perf_counter()-began
        added = 0
        # The surest first, so that of two near-identical finds the better one stays.
        for found in sorted(masks, key=lambda m: -m['predicted_iou']):
            mask = found['segmentation']
            area = int(mask.sum())
            if area>.3*width*height:
                continue  # the road, the sky: not something to pick out
            little = cv2.resize(mask.astype(np.uint8), (width//4, height//4), interpolation=cv2.INTER_AREA)>0
            if any((little & other).sum()>.85*(little | other).sum() for other in small):
                continue
            points = sam.outline(mask)
            if points is None:
                continue
            small.append(little)
            kept.append({'points': points, 'area': area, 'view': view})
            added += 1
        print(f'{view}: {len(masks)} found in {took:.0f} s, {added} kept', flush=True)
    kept.sort(key=lambda shape: shape['area'])
    (args.scene/'shapes.json').write_text(json.dumps({'width': width, 'height': height, 'shapes': kept}))
    print(f'{args.scene/"shapes.json"}: {len(kept)} shapes')


if __name__=='__main__':
    main()
