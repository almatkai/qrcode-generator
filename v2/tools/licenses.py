#!/usr/bin/env python3
"""Capture genuine LICENSE/COPYING/NOTICE files for every resolved crate, including target-only/dev."""
import subprocess,json,pathlib,hashlib
v2=pathlib.Path(__file__).resolve().parent.parent
m=json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1','--manifest-path',str(v2/'Cargo.toml')]))
prior_path=v2/'licenses/dependencies.json'
prior={(r['name'],r['version']):r for r in json.loads(prior_path.read_text())} if prior_path.exists() else {}
rows=[]
for p in sorted(m['packages'],key=lambda p:(p['name'],p['version'])):
 if p['name']=='tezbar-qrcode-generator':continue
 root=pathlib.Path(p['manifest_path']).parent;dest=v2/'licenses/dependencies'/(p['name']+'-'+p['version']);notices=[]
 for f in sorted(root.iterdir()):
  if f.is_file() and f.name.upper().startswith(('LICENSE','COPYING','NOTICE','COPYRIGHT')):
   dest.mkdir(parents=True,exist_ok=True);data=f.read_bytes();(dest/f.name).write_bytes(data);notices.append({'file':str((dest/f.name).relative_to(v2)),'sha256':hashlib.sha256(data).hexdigest()})
 # Some crates use a license file nested under a named license directory.
 for f in sorted(root.glob('LICENSES/*')):
  if f.is_file():dest.mkdir(parents=True,exist_ok=True);data=f.read_bytes();(dest/f.name).write_bytes(data);notices.append({'file':str((dest/f.name).relative_to(v2)),'sha256':hashlib.sha256(data).hexdigest()})
 if not notices: notices=prior.get((p['name'],p['version']),{}).get('notices',[])
 rows.append({'name':p['name'],'version':p['version'],'source':p['source'],'license':p['license'],'license_file':p['license_file'],'notices':notices})
(v2/'licenses/dependencies.json').write_text(json.dumps(rows,indent=2)+'\n')
print('Crates',len(rows),'with packaged grants',sum(bool(r['notices']) for r in rows))
