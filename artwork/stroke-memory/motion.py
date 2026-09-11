"""Motion transport for the local, persistent-stroke experiment."""
import numpy as np
import cv2


def transport(points, forward, backward, previous=None, current=None):
    """Forward transport, rejected where the round trip disagrees or exits frame."""
    def sample(field, at):
        return np.concatenate([cv2.remap(field, part[:, 0:1], part[:, 1:2], cv2.INTER_LINEAR,
                         borderMode=cv2.BORDER_REPLICATE).reshape(len(part), -1)
                         for part in np.array_split(at, max(1,(len(at)+16383)//16384))])
    displacement = sample(forward, points)
    moved = points + displacement
    returned = sample(backward, moved)
    h, w = forward.shape[:2]
    trusted = ((moved[:, 0] >= 1) & (moved[:, 0] < w-1) &
               (moved[:, 1] >= 1) & (moved[:, 1] < h-1) &
               (np.linalg.norm(displacement + returned, axis=1) < 1.1))
    if previous is not None and current is not None:
        before=sample(previous,points).astype(np.float32)
        here=sample(current,points).astype(np.float32)
        there=sample(current,moved).astype(np.float32)
        still_error=np.max(np.abs(here-before),axis=1)
        moved_error=np.max(np.abs(there-before),axis=1)
        trusted &= moved_error < np.maximum(6.,still_error+2.)
        # A flat/unchanging passage provides no reason to drift across the canvas.
        trusted &= ~((still_error<2.5) & (moved_error>=still_error*.7))
    return moved, trusted
