"""Reuse the recorded-study player, with accurate prototype/cadence descriptions."""
import pathlib
import json
import subprocess
import sys

root=pathlib.Path(sys.argv[1]).resolve()
repo=pathlib.Path(__file__).resolve().parents[2]
subprocess.run([sys.executable,str(repo/'artwork/editions/make_gallery.py'),str(root),
                '--catalog',str(root/'study-catalog.json')],check=True)
p=root/'index.html';s=p.read_text()
materials=any(item.get('params',{}).get('material','plain')!='plain'
              for item in json.loads((root/'study-catalog.json').read_text()))
s=s.replace('Living Ink / Moving studies','Living Ink / Brush materials' if materials else 'Living Ink / Brush memory')
s=s.replace('Rendered locally with the native Rust drawing engine from the same recorded Abbey Road, London footage. Every source frame is preserved at 1920 × 1080 and 29.97 fps.',
    'A local experiment using optical-flow analysis and cached native brush masks, compared with the live fine-knife renderer on the same recorded Abbey Road footage. Output is 1920 × 1080 at 29.97 fps, with 14.985 painting updates per second. The recording uses a fixed 3 PM London color setting.')
s=s.replace("'Local render measurement: '+(item.metrics.frame_p50_ms||0).toFixed(1)+' ms per frame (median). This is a local measurement, not an estimate of a production hosting bill.'",
    "'Local render time: '+(item.metrics.wall_seconds||item.metrics.total_ms/1000||0).toFixed(1)+' seconds for '+(item.metrics.duration_seconds||0).toFixed(1)+' seconds of video. The prototype is experimental and slower than real time. These local implementations use different execution paths; this is not a production billing estimate.'")
description=('Compare the raised grooves of Loaded bristle, the broken deposits of Scumbled pigment, and the longer curved marks of Swept oils with the Adaptive effort reference. Texture belongs to each persistent brushstroke and travels with it. Small accents use fuller, simpler deposits to keep the recovered detail readable. Each material feeds back into the adaptive stroke planner. The source palette, 15 fps paint movement and 30 fps psychedelic wave are shared by every version.' if materials else
    'Start with Adaptive effort and compare Moving brushwork at the same moment. The painting is checked against the source after each brush size; smaller marks are added where they recover unresolved shapes. Watch the car, crossing, figures and quiet road. New-stroke decisions run about five times a second, while existing paint moves at 15 fps and the wave at 30 fps. Human focus and Fine detail are the earlier comparisons. Production remains on Fresh pigment.')
s=s.replace('The public livestream retains its current look.',description)
if materials:
    s=s.replace('<button id="play">Play</button>',
                '<button id="play">Play</button><button id="fullscreen">Full screen</button>')
    s=s.replace("el('previous').onclick=", "el('fullscreen').onclick=()=>{if(film.requestFullscreen)film.requestFullscreen().catch(()=>{el('error').textContent='Use the video controls to enter full screen.'});else if(film.webkitEnterFullscreen)film.webkitEnterFullscreen()};el('previous').onclick=")
p.write_text(s)
