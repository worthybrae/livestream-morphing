"""Reuse the recorded-study player, with accurate prototype/cadence descriptions."""
import pathlib
import subprocess
import sys

root=pathlib.Path(sys.argv[1]).resolve()
repo=pathlib.Path(__file__).resolve().parents[2]
subprocess.run([sys.executable,str(repo/'artwork/editions/make_gallery.py'),str(root),
                '--catalog',str(root/'study-catalog.json')],check=True)
p=root/'index.html';s=p.read_text()
s=s.replace('Living Ink / Moving studies','Living Ink / Brush memory')
s=s.replace('Rendered locally with the native Rust drawing engine from the same recorded Abbey Road, London footage. Every source frame is preserved at 1920 × 1080 and 29.97 fps.',
    'A local experiment using optical-flow analysis and cached native brush masks, compared with the live fine-knife renderer on the same recorded Abbey Road footage. Output is 1920 × 1080 at 29.97 fps, with 14.985 painting updates per second. The recording uses a fixed 3 PM London color setting.')
s=s.replace("'Local render measurement: '+(item.metrics.frame_p50_ms||0).toFixed(1)+' ms per frame (median). This is a local measurement, not an estimate of a production hosting bill.'",
    "'Local render time: '+(item.metrics.wall_seconds||item.metrics.total_ms/1000||0).toFixed(1)+' seconds for '+(item.metrics.duration_seconds||0).toFixed(1)+' seconds of video. The prototype is experimental and slower than real time. These local implementations use different execution paths; this is not a production billing estimate.'")
s=s.replace('The public livestream retains its current look.',
    'Compare Fine detail with Moving brushwork at the same moment: look at the railings, crossing edges, and people. The original three coats retain their brush geometry; an extra finishing coat follows small source features. The fixed-brushwork control isolates motion transport. Production remains on Fresh pigment.')
p.write_text(s)
