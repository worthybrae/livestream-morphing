#!/usr/bin/env python3
"""A continuous-source, 40-second tour through the eight shortlisted materials."""
import argparse,pathlib,subprocess,json
p=argparse.ArgumentParser();p.add_argument('output');a=p.parse_args();root=pathlib.Path(a.output).resolve();clips=root/'showreel-parts';clips.mkdir(exist_ok=True)
slugs=['graphite','cyanotype','isobars','risograph','phosphor','woodcut','woven','mineral']
catalog={i['slug']:i for i in json.loads((pathlib.Path(__file__).parent/'catalog.json').read_text())}
for i,slug in enumerate(slugs):
 source=root/'videos'/f'{slug}.mp4';out=clips/f'{i:02}.mp4'
 # Each cut advances along the same original footage and the same signature phase.
 # No dissolves, doubled edges, time reversal, or independently restarted waves.
 vf=f'trim=start_frame={i*150}:end_frame={(i+1)*150},setpts=PTS-STARTPTS'
 subprocess.run(['ffmpeg','-v','error','-y','-i',str(source),'-vf',vf,'-an','-c:v','libx264','-preset','fast','-crf','23','-pix_fmt','yuv420p','-threads','2','-movflags','+faststart',str(out)],check=True)
 print(f'Showreel: {slug}',flush=True)
(clips/'concat.txt').write_text('\n'.join("file '"+f'{i:02}.mp4'+"'" for i in range(len(slugs))))
subprocess.run(['ffmpeg','-v','error','-y','-f','concat','-safe','0','-i',str(clips/'concat.txt'),'-c','copy','-movflags','+faststart',str(root/'showreel.mp4')],check=True)
(root/'showreel.json').write_text(json.dumps({'description':'Eight materials; the source camera and signature phase advance continuously across cuts.','frames_per_material':150,'fps':'30000/1001','order':[catalog[s]['name'] for s in slugs]},indent=2))
