#!/usr/bin/env python3
"""Inventory/zip exactly Git-visible deliverables without staging or a commit."""
from pathlib import Path
import subprocess,hashlib,json,stat,zipfile
root=Path(__file__).resolve().parent.parent.parent
files=sorted(set(subprocess.check_output(['git','ls-files','-z'],cwd=root).decode().split('\0')+subprocess.check_output(['git','ls-files','--others','--exclude-standard','-z'],cwd=root).decode().split('\0'))-{'' ,'v2/INDEX.sha256'})
for name in files:
 p=root/name;assert p.is_file() and not p.is_symlink(),name
lines=[hashlib.sha256((root/name).read_bytes()).hexdigest()+'  '+name for name in files]
(root/'v2/INDEX.sha256').write_text('\n'.join(lines)+'\n')
out=root/'v2/out';out.mkdir(exist_ok=True);archive=out/'source.zip'
with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as z:
 for name in files+['v2/INDEX.sha256']:
  p=root/name;info=zipfile.ZipInfo(name,(1980,1,1,0,0,0));info.create_system=3;info.external_attr=(stat.S_IFREG|(0o755 if p.stat().st_mode&0o111 else 0o644))<<16;z.writestr(info,p.read_bytes(),compress_type=zipfile.ZIP_DEFLATED,compresslevel=9)
print(json.dumps({'filesInventoried':len(files),'sourceFiles':len(files)+1,'sourceZipSha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'sourceZipBytes':archive.stat().st_size,'inventorySelfExclusion':'v2/INDEX.sha256 only'},indent=2))
