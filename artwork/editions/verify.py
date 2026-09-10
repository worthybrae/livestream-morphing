#!/usr/bin/env python3
"""Probe finished movies, hash their inputs, and write a reproducible collection manifest."""
import argparse,hashlib,json,pathlib,subprocess
p=argparse.ArgumentParser();p.add_argument('output');p.add_argument('--frames',type=int,default=2700);p.add_argument('--catalog',default=str(pathlib.Path(__file__).parent/'catalog.json'));p.add_argument('--source',required=True);p.add_argument('--crf',type=int,default=23);a=p.parse_args();root=pathlib.Path(a.output).resolve();repo=pathlib.Path(__file__).resolve().parents[2]
def digest(path):
 h=hashlib.sha256()
 with path.open('rb') as f:
  for block in iter(lambda:f.read(1048576),b''):h.update(block)
 return h.hexdigest()
items=json.loads(pathlib.Path(a.catalog).read_text());verified=[]
for item in items:
 slug=item['slug'];expected_frames=item.get('frames',a.frames);video=root/'videos'/f'{slug}.mp4'
 if not video.exists():raise RuntimeError(f'Missing movie: {slug}')
 result=json.loads(subprocess.check_output(['ffprobe','-v','error','-select_streams','v:0','-count_frames','-show_entries','stream=width,height,r_frame_rate,nb_read_frames,duration','-of','json',str(video)]))['streams'][0]
 if (result['width'],result['height'],result['r_frame_rate'],int(result['nb_read_frames']))!=(1920,1080,'30000/1001',expected_frames):raise RuntimeError(f'Unexpected output format: {slug}: {result}')
 verified.append({**item,'video':str(video.relative_to(root)),'bytes':video.stat().st_size,'sha256':digest(video),'probe':result,'preset':json.loads((root/'presets'/f'{slug}.json').read_text()),'metrics':json.loads((root/'metrics'/f'{slug}.json').read_text())})
 print(f'{slug}: verified {expected_frames} frames / 1080p / 29.97 fps',flush=True)
manifest={'signature':{'amplitude':0.012,'frequency':8,'period_seconds':8,'application':'once, after material construction','time_source':'source frame number × source frame duration'},'default_source_sha256':digest(pathlib.Path(a.source)),'source_note':'Per-edition metrics record source_sha256 when available; the default source applies otherwise.','renderer_sha256':digest(repo/'livestream-morphing-rs/target/release/livestream-morphing-rs'),'encoder_crf':a.crf,'codec':'H.264, yuv420p','editions':verified}
(root/'manifest.json').write_text(json.dumps(manifest,indent=2));print(root/'manifest.json')
