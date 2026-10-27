from pathlib import Path
import json,subprocess,shutil,hashlib
v2=Path(__file__).resolve().parent.parent;root=v2.parent;reference=v2/'.work/v1';reference.mkdir(parents=True,exist_ok=True)
# Works in an extracted source archive without .git. V1 bytes are verified by the provenance inventory.
provenance=json.loads((v2/'PROVENANCE.json').read_text())
for item in provenance['objects']:
 if item['kind']!='own':continue
 p=item['path'];data=(root/p).read_bytes()
 if p=='README.md':
  suffix=b'\nNative Rust V2: see [v2/README.md](v2/README.md).\n'
  if data.endswith(suffix):data=data[:-len(suffix)]
 assert hashlib.sha256(data).hexdigest()==item['sha256'],p
 dest=reference/p;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data)
subprocess.run(['npm','ci','--ignore-scripts'],cwd=reference,check=True)
subprocess.run(['npm','test'],cwd=reference,check=True)
subprocess.run(['npm','run','typecheck'],cwd=reference,check=True)
