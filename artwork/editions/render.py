#!/usr/bin/env python3
"""Reproduce the collection locally; never changes the public stream."""
import argparse,json,pathlib,subprocess,time,hashlib
p=argparse.ArgumentParser();p.add_argument('output');p.add_argument('--source',required=True);p.add_argument('--catalog',default=str(pathlib.Path(__file__).parent/'catalog.json'));p.add_argument('--width',default=1920,type=int);p.add_argument('--slugs',nargs='*');p.add_argument('--crf',type=int,choices=range(18,41),default=23);p.add_argument('--workers',type=int,choices=range(1,7),default=3);args=p.parse_args()
repo=pathlib.Path(__file__).resolve().parents[2];root=pathlib.Path(args.output).resolve();source=pathlib.Path(args.source).resolve()
catalog=json.loads(pathlib.Path(args.catalog).read_text())
source_sha256=hashlib.sha256(source.read_bytes()).hexdigest()
for folder in ['videos','stills','metrics','presets','intermediate']: (root/folder).mkdir(parents=True,exist_ok=True)
for item in catalog:
 if args.slugs and item['slug'] not in args.slugs: continue
 slug=item['slug'];preset=root/'presets'/f'{slug}.json'
 if not preset.exists():preset.write_text(json.dumps({'edition':item['id'],'detail':1,'pigment':1,**item.get('params',{})},indent=2))
 output=root/'intermediate'/f'{slug}.ts';start=time.monotonic()
 command=['docker','run','--rm','-e',f'ENCODE_CRF={args.crf}','-e',f'RENDER_WORKERS={args.workers}','-v',f'{repo}/livestream-morphing-rs:/work','-v',f'{root}:/art','-v',f'{source.parent}:/source:ro','-w','/work','livestream-morphing-sdd-builder','/work/target/release/livestream-morphing-rs','render',f'/source/{source.name}',f'/art/intermediate/{slug}.ts',str(args.width),'edition',f'/art/presets/{slug}.json']
 with (root/'metrics'/f'{slug}.stderr').open('w') as error:
  completed=subprocess.run(command,stdout=subprocess.PIPE,stderr=error,text=True,check=True)
 metrics=json.loads(completed.stdout.strip().splitlines()[-1]);metrics["render_workers"]=args.workers;metrics["source_sha256"]=source_sha256;metrics["renderer_sha256"]=hashlib.sha256((repo/"livestream-morphing-rs/target/release/livestream-morphing-rs").read_bytes()).hexdigest();(root/'metrics'/f'{slug}.json').write_text(json.dumps(metrics,indent=2))
 subprocess.run(['ffmpeg','-v','error','-y','-i',str(output),'-map','0:v:0','-c','copy','-movflags','+faststart',str(root/'videos'/f'{slug}.mp4')],check=True)
 subprocess.run(['ffmpeg','-v','error','-y','-ss','3','-i',str(root/'videos'/f'{slug}.mp4'),'-frames:v','1',str(root/'stills'/f'{slug}.png')],check=True)
 print(f'{slug}: {metrics["frames"]} frames, {metrics["frame_p50_ms"]:.1f}ms/frame, {time.monotonic()-start:.1f}s total',flush=True)
(root/'catalog.json').write_text(json.dumps(catalog,indent=2))
