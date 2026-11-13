"""Meta's Segment Anything 2, as the tracing page uses it: outlines of objects.

Needs the optional environment (torch, and sam2 installed from
github.com/facebookresearch/sam2) and the checkpoint in models/. Nothing else
in the painter depends on any of it.
"""
import pathlib

import cv2
import numpy as np

HERE = pathlib.Path(__file__).resolve().parent
CHECKPOINT = HERE.parent/'models'/'sam2.1_hiera_base_plus.pt'
CONFIG = 'configs/sam2.1/sam2.1_hiera_b+.yaml'


def model(device='cpu'):
    from sam2.build_sam import build_sam2
    if not CHECKPOINT.is_file():
        raise RuntimeError(f'Missing {CHECKPOINT}')
    return build_sam2(CONFIG, str(CHECKPOINT), device=device)


def seen(image):
    """The view as the model should be shown it, RGB. A night plate is lifted
    first: the model was taught on ordinary photographs, not on the dark."""
    lab = cv2.cvtColor(image, cv2.COLOR_BGR2LAB)
    if lab[:,:,0].mean()<110:
        lab[:,:,0] = cv2.createCLAHE(clipLimit=2.5, tileGridSize=(8,8)).apply(lab[:,:,0])
        lifted = (cv2.cvtColor(lab, cv2.COLOR_LAB2BGR)/255.)**.75*255
        image = lifted.clip(0, 255).astype(np.uint8)
    return cv2.cvtColor(image, cv2.COLOR_BGR2RGB)


def outline(mask):
    """The outer edge of a mask as one closed run of points, or None if it has none worth drawing."""
    found, _ = cv2.findContours(mask.astype(np.uint8), cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
    if not found:
        return None
    rim = max(found, key=cv2.contourArea)
    if cv2.contourArea(rim)<80:
        return None
    points = cv2.approxPolyDP(rim, 1.3, True)[:,0,:].astype(np.float32)
    if len(points)<3:
        return None
    # One pass of corner cutting takes the pixel staircase out of slanted edges.
    after = np.roll(points, -1, axis=0)
    points = np.stack([points*.75+after*.25, points*.25+after*.75], axis=1).reshape(-1, 2)
    return np.round(np.concatenate([points, points[:1]]), 1).tolist()
