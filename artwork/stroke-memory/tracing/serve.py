#!/usr/bin/env python3
"""Serve the line tracing page and save what is drawn on it.

Local use only. The page lives next to this file; the scene it traces, and the
lines saved from it, live in the scene folder (see prepare.py).
"""
import argparse
import json
import pathlib
import threading
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = pathlib.Path(__file__).resolve().parent
KINDS = {'.html': 'text/html; charset=utf-8', '.jpg': 'image/jpeg', '.png': 'image/png', '.json': 'application/json'}


class Asker:
    """Says what is at a point, with Segment Anything 2: up to a few readings of it,
    a part, the thing, the whole it belongs to, smallest first. Loaded on first use
    and only if the optional environment is there (see sam.py); the page works
    without it.

    The model sees any picture at about a thousand pixels across, so in the whole
    frame a road marking is a few pixels wide. When the page is zoomed in, the
    model is also shown just that part of the scene, enlarged, and can tell far
    more there.
    """
    KEPT = 20  # parts of the scene held ready at once, most recently used last

    def __init__(self, scene):
        self.scene, self.lock, self.net = scene, threading.Lock(), None
        self.plates, self.looks = {}, {}

    def look(self, view, x0, y0, w, h):
        """The model made ready to answer about one part of one view, and how much that part was enlarged."""
        import cv2
        import sam
        from sam2.sam2_image_predictor import SAM2ImagePredictor
        if view not in self.plates:
            image = cv2.imread(str(self.scene/{'day': 'plate-day.jpg'}.get(view, 'plate.jpg')))
            if image is None:
                raise FileNotFoundError(view)
            self.plates[view] = sam.seen(image)
        key = (view, x0, y0, w, h)
        if key in self.looks:
            self.looks[key] = self.looks.pop(key)
        else:
            self.net = self.net or sam.model()
            part = self.plates[view][y0:y0+h, x0:x0+w]
            larger = max(1, min(4, 1024//w))
            if larger>1:
                part = cv2.resize(part, (w*larger, h*larger), interpolation=cv2.INTER_CUBIC)
            ready = SAM2ImagePredictor(self.net)
            ready.set_image(part)
            self.looks[key] = (ready, larger)
            while len(self.looks)>self.KEPT:
                self.looks.pop(next(iter(self.looks)))
        return self.looks[key]

    def readings(self, view, x, y, span):
        import numpy as np
        import sam
        with self.lock:
            self.look(view, 0, 0, 1, 1) if view not in self.plates else None
            height, width = self.plates[view].shape[:2]
            parts = [(0, 0, width, height)]
            for share in (4, 2):
                w, h = width//share, height//share
                if span<=w*1.2:
                    # On a quarter-step grid, so pointing around nearby reuses the same part.
                    x0 = int(np.clip(round((x-w/2)/(w/4))*(w//4), 0, width-w))
                    y0 = int(np.clip(round((y-h/2)/(h/4))*(h//4), 0, height-h))
                    parts.insert(0, (x0, y0, w, h))
                    break
            found = []
            for x0, y0, w, h in parts:
                ready, larger = self.look(view, x0, y0, w, h)
                masks, scores, _ = ready.predict(point_coords=np.array([[(x-x0)*larger, (y-y0)*larger]], np.float32),
                                                 point_labels=np.array([1]), multimask_output=True)
                for mask, score in zip(masks>0, scores):
                    area = int(mask.sum())/larger**2
                    whole = (w, h)==(width, height)
                    # In a part of the scene, a reading cut off by the part's own edge is no thing.
                    cut = not whole and ((mask[0].any() and y0>0) or (mask[-1].any() and y0+h<height) or
                                         (mask[:,0].any() and x0>0) or (mask[:,-1].any() and x0+w<width))
                    # Nor is the ground the whole scene stands on, nor the same reading twice.
                    if cut or area>.3*width*height or any(abs(area-other['area'])<.15*max(area, other['area']) for other in found):
                        continue
                    points = sam.outline(mask)
                    if points is not None:
                        found.append({'points': [[px/larger+x0, py/larger+y0] for px, py in points], 'area': round(area),
                                      'score': round(float(score), 3)})
        return sorted(found, key=lambda reading: reading['area'])


def handler(scene):
    asker = Asker(scene)

    class Handler(BaseHTTPRequestHandler):
        def send(self, status, body, kind='text/plain; charset=utf-8'):
            self.send_response(status)
            self.send_header('Content-Type', kind)
            self.send_header('Content-Length', str(len(body)))
            self.send_header('Cache-Control', 'no-store')
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            path = self.path.split('?')[0]
            if path=='/pick':
                try:
                    asked = urllib.parse.parse_qs(urllib.parse.urlsplit(self.path).query)
                    readings = asker.readings(asked.get('view', ['night'])[0], float(asked['x'][0]), float(asked['y'][0]),
                                              float(asked.get('span', ['1e9'])[0]))
                except ImportError:
                    return self.send(501, b'the model is not installed for this server')
                except (KeyError, ValueError, FileNotFoundError):
                    return self.send(400, b'nothing to look at there')
                return self.send(200, json.dumps({'readings': readings}).encode(), KINDS['.json'])
            file = HERE/'trace.html' if path=='/' else scene/path[len('/scene/'):] if path.startswith('/scene/') else None
            # Only the page and the plain files of the scene folder are ever served.
            if file is None or file.parent not in (HERE, scene) or not file.is_file() or file.suffix not in KINDS:
                return self.send(404, b'Not found')
            self.send(200, file.read_bytes(), KINDS[file.suffix])

        def do_POST(self):
            if self.path!='/save':
                return self.send(404, b'Not found')
            size = int(self.headers.get('Content-Length', 0))
            if not 0<size<8_000_000:
                return self.send(413, b'Too large')
            try:
                drawing = json.loads(self.rfile.read(size))
                lines = [{'points': [[float(x), float(y)] for x, y in line['points']]}
                         for line in drawing['lines'] if len(line['points'])>=2]
                saved = {'width': int(drawing['width']), 'height': int(drawing['height']), 'lines': lines}
            except (KeyError, TypeError, ValueError):
                return self.send(400, b'Not a drawing')
            target = scene/'lines.json'
            if target.is_file():
                # One step of safety against a slip of the eraser.
                (scene/'lines.previous.json').write_bytes(target.read_bytes())
            scratch = scene/'lines.json.part'
            scratch.write_text(json.dumps(saved))
            scratch.replace(target)
            self.send(200, json.dumps({'lines': len(lines)}).encode(), KINDS['.json'])

        def log_message(self, *args):
            pass
    return Handler


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--port', type=int, default=8765)
    ap.add_argument('--scene', type=pathlib.Path, default=HERE.parent/'scene')
    args = ap.parse_args()
    scene = args.scene.resolve()
    print(f'Tracing page: http://localhost:{args.port}  (scene: {scene})', flush=True)
    ThreadingHTTPServer(('127.0.0.1', args.port), handler(scene)).serve_forever()


if __name__=='__main__':
    main()
