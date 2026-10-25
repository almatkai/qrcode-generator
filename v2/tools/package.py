#!/usr/bin/env python3
"""Deterministic private target ZIP from actual built bytes; no release URL or invented pin."""
import argparse,hashlib,json,pathlib,stat,struct,zipfile,subprocess,os
V2=pathlib.Path(__file__).resolve().parent.parent
def sha(b):return hashlib.sha256(b).hexdigest()
def safe(name):
 p=pathlib.PurePosixPath(name)
 if not name or name.startswith('/') or '\\' in name or ':' in name or any(x in ('','.','..') for x in name.split('/')):raise ValueError('unsafe member')
 if any(c<' ' for c in name):raise ValueError('control in member')
 if len(name)>1024 or any(x.endswith((' ','.')) or x.split('.')[0].upper() in {'CON','PRN','AUX','NUL',*[f'COM{i}' for i in range(1,10)],*[f'LPT{i}' for i in range(1,10)]} for x in name.split('/')):raise ValueError('Windows-unsafe member')
 return p

def header(data,target):
 if target=='aarch64-apple-darwin':
  assert data[:4]==b'\xcf\xfa\xed\xfe' and struct.unpack_from('<I',data,4)[0]==0x100000c,'Expected ARM64 Mach-O'
 elif target=='x86_64-pc-windows-msvc':
  assert data[:2]==b'MZ';offset=struct.unpack_from('<I',data,0x3c)[0];assert data[offset:offset+4]==b'PE\0\0' and struct.unpack_from('<H',data,offset+4)[0]==0x8664,'Expected AMD64 PE'
 else:raise ValueError('Unsupported target')
def build(binary,target):
 data=pathlib.Path(binary).read_bytes();header(data,target);name='bin/tezbar-qrcode-generator'+('.exe' if 'windows' in target else '')
 files={name:data,'LICENSE':(V2/'LICENSE').read_bytes(),'NOTICE':(V2/'NOTICE').read_bytes()}
 for folder in ['resources','licenses']:
  for p in sorted((V2/folder).rglob('*')):
   if p.is_file():assert not p.is_symlink();files[p.relative_to(V2).as_posix()]=p.read_bytes()
 m=json.loads((V2/'extension.json').read_text());m['files']={p:sha(b) for p,b in sorted(files.items())};files['extension.json']=(json.dumps(m,sort_keys=True,indent=2)+'\n').encode()
 out=V2/'out'/target;out.mkdir(parents=True,exist_ok=True);archive=out/'qrcode-generator.zip'
 with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as z:
  for p,b in sorted(files.items()):
   safe(p);info=zipfile.ZipInfo(p,(1980,1,1,0,0,0));info.create_system=3;info.external_attr=((stat.S_IFREG|0o755) if p==name else (stat.S_IFREG|0o644))<<16;info.compress_type=zipfile.ZIP_DEFLATED;z.writestr(info,b,compress_type=zipfile.ZIP_DEFLATED,compresslevel=9)
 metadata={'target':target,'sha256':sha(archive.read_bytes()),'bytes':archive.stat().st_size,'executable':name,'executableSha256':sha(data),'inventory':m['files'],'extractedDirectory':'extracted-'+sha(archive.read_bytes())[:12],'note':'Private local QA artifact; no acquisition URL, release, installation or host catalog pin.'}
 (out/'artifact.json').write_text(json.dumps(metadata,indent=2,sort_keys=True)+'\n');validate(archive,out/metadata['extractedDirectory']);return metadata

def validate(archive,destination):
 destination=pathlib.Path(destination);destination.mkdir(parents=True,exist_ok=True)
 with zipfile.ZipFile(archive) as z:
  assert len(z.infolist())<=4096;seen=set();total=0
  for i in z.infolist():
   safe(i.filename);mode=i.external_attr>>16;assert stat.S_IFMT(mode)==stat.S_IFREG;assert i.filename.casefold() not in seen;seen.add(i.filename.casefold());total+=i.file_size;assert total<=512*1024*1024;assert i.file_size<=1024*1024 or i.file_size//max(i.compress_size,1)<=1000
  m=json.loads(z.read('extension.json'));assert set(z.namelist())==set(m['files'])|{'extension.json'}
  for p,digest in m['files'].items():assert sha(z.read(p))==digest
  for i in z.infolist():
   p=destination/i.filename;p.parent.mkdir(parents=True,exist_ok=True)
   if p.exists():assert p.read_bytes()==z.read(i.filename)
   else:p.write_bytes(z.read(i.filename))
   p.chmod(0o700 if i.filename.startswith('bin/') else 0o600)
 return m
if __name__=='__main__':
 a=argparse.ArgumentParser();a.add_argument('binary');a.add_argument('--target',required=True);args=a.parse_args();print(json.dumps(build(args.binary,args.target),indent=2))
