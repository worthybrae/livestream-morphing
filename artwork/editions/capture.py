#!/usr/bin/env python3
"""Capture bounded already-downloaded source segments for local art studies."""
import argparse,urllib.request,pathlib,json,time,subprocess
p=argparse.ArgumentParser();p.add_argument('output');p.add_argument('--segments',type=int,default=15);a=p.parse_args()
root=pathlib.Path(a.output).resolve();root.mkdir(parents=True,exist_ok=True)
base='https://livestream-morphing-production.up.railway.app';seen=set();items=[];deadline=time.monotonic()+150
while len(items)<a.segments and time.monotonic()<deadline:
 playlist=urllib.request.urlopen(base+'/api/raw-stream',timeout=15).read().decode()
 paths=[s for s in playlist.splitlines() if s.startswith('/api/raw-segments/')]
 for path in paths:
  if path in seen:continue
  seen.add(path)
  if items and int(path.rsplit('-',1)[-1][:-3])!=int(items[-1]['path'].rsplit('-',1)[-1][:-3])+1:
   raise RuntimeError('Capture gap: use a continuous source session')
  data=urllib.request.urlopen(base+path,timeout=15).read();filename=f'{len(items):03}.ts';(root/filename).write_bytes(data)
  items.append({'path':path,'file':filename,'bytes':len(data)})
  print(f'Captured {len(items)}/{a.segments}',flush=True)
  if len(items)==a.segments:break
 if len(items)<a.segments:time.sleep(3)
if len(items)!=a.segments:raise RuntimeError('Capture deadline exceeded')
(root/'capture.json').write_text(json.dumps({'captured_at_utc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'segments':items},indent=2))
(root/'concat.txt').write_text('\n'.join("file '"+i['file']+"'" for i in items))
subprocess.run(['ffmpeg','-v','error','-y','-f','concat','-safe','0','-i',str(root/'concat.txt'),'-map','0:v:0','-c','copy',str(root/'street.ts')],check=True)
