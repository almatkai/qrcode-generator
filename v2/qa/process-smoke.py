#!/usr/bin/env python3
"""Real production executable + OS JSONL pipes. All desktop services are a fake HOST.
Never acknowledges copy unless it has read/hashed the generated PNG. No live desktop RPC.
"""
import json,os,sys,uuid,subprocess,tempfile,select,time,hashlib,pathlib
BINARY=str(pathlib.Path(sys.argv[1]).resolve())
(pathlib.Path(__file__).resolve().parent.parent/'.work').mkdir(parents=True,exist_ok=True)
class Pipe:
 def __init__(self,root,grants=None):
  self.root=root;self.p=subprocess.Popen([BINARY],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,bufsize=0);self.buffer=b'';self.reverse=[]
  hello=self.read();assert hello['type']=='hello' and hello['protocol']==1 and hello['id']=='com.tezbar.native.qrcode-generator'
  assert self.request('initialize',dict(protocol=1,extensionId=hello['id'],platform='macOS',packageDir=str(root),dataDir=str(root/'data'),tempDir=str(root/'temp'),grantedCapabilities=grants if grants is not None else ['clipboard.read','selection.read','clipboard.writeImage','shell.open']))['result']['ready']
 def send(self,v):self.p.stdin.write((json.dumps(v,ensure_ascii=True,separators=(',',':'))+'\n').encode());self.p.stdin.flush()
 def read(self,timeout=5):
  end=time.monotonic()+timeout
  while b'\n' not in self.buffer:
   assert select.select([self.p.stdout],[],[],max(0,end-time.monotonic()))[0],'protocol timeout'
   b=os.read(self.p.stdout.fileno(),65536);assert b,'unexpected EOF';self.buffer+=b;assert len(self.buffer)<16*1024*1024
  line,self.buffer=self.buffer.split(b'\n',1);assert b'\r' not in line;return json.loads(line)
 def start(self,method,params):
  rid='h:'+str(uuid.uuid4());self.send(dict(type='request',id=rid,method=method,params=params));return rid
 def result(self,rid,reply=None):
  while True:
   m=self.read()
   if m['type']=='request':
    self.reverse.append(m);value=reply(m) if reply else None
    self.send(dict(type='response',id=m['id'],result=value))
   elif m['type']=='response' and m['id']==rid:return m
   elif m['type'] not in ('cancel','event'):raise AssertionError(m)
 def request(self,method,params,reply=None):return self.result(self.start(method,params),reply)
 def open(self,sid,command,reply=None):return self.request('session.open',dict(sessionId=sid,commandName=command,preferences={},argumentValues={}),reply)
 def call(self,sid,action=None,form=None,op='extension:invoke-action',reply=None):
  payload={}
  if action is not None:payload['actionId']=action
  if form is not None:payload['formValues']=form
  return self.request('session.call',dict(sessionId=sid,operation=op,payload=payload),reply)
 def close(self,eof=False):
  if not eof:assert 'result' in self.request('shutdown',{})
  self.p.stdin.close();assert self.p.wait(timeout=6)==0,self.p.stderr.read();assert not self.p.stderr.read()
def fields(text):return dict(url=text,format='png',color='custom',customColor='#000000',shorten=False,utmEnabled=False,utmSource='',utmMedium='',utmCampaign='',utmTerm='',utmContent='',__tezbarDraftRevision=1)
with tempfile.TemporaryDirectory(prefix='qr-process-',dir=pathlib.Path(__file__).resolve().parent.parent/'.work') as t:
 root=pathlib.Path(t)
 for flag in ('--protocol-info','--self-test'):
  value=json.loads(subprocess.check_output([BINARY,flag]));assert value['sdkVersion']=='0.1.1' and value['commands']==['index','clipboard','selection']
 host=Pipe(root/'first');assert host.open('form','index')['result']['root']['type']=='Form';f=fields('Қазақша 🙂 https://example.test')
 v=host.call('form','generate-show',f)['result'];assert v['root']['type']=='Detail';assert 'raycast-height' not in v['root']['props']['markdown'];original=v['root']['props']['markdown']
 # A -> unsubmitted B/invalid -> A is a fresh intentional generation, not a dead form.
 for intermediate in ('B',''):
  host.call('form','edit');assert host.call('form',op='extension:refresh-session')['result']['root']['type']=='Form'
  edit=dict(f,url=intermediate,__tezbarDraftRevision=f['__tezbarDraftRevision']+1)
  assert host.call('form','field:url',edit)['result']['root']['type']=='Form'
  stale=dict(f);f['__tezbarDraftRevision']+=2
  host.call('form','field:url',f);assert host.call('form',op='extension:refresh-session')['result']['root']['type']=='Form'
  assert 'error' in host.call('form','generate-show',stale)
  again=host.call('form','generate-show',f)['result'];assert again['root']['type']=='Detail' and again['root']['props']['markdown']==original
 assert not host.reverse
 # Capability denial is visible and not interpreted as empty text/success.
 denied=Pipe(root/'denied',[]);v=denied.open('clip','clipboard');assert 'Could not read' in v['result']['message'];assert not denied.reverse;denied.close(eof=True)
 # Acquire exactly once; cancellation after effect dispatch then late ack cannot replay.
 v=host.open('clip','clipboard',lambda m:'  raw clipboard 🙂  ')['result'];assert len(host.reverse)==1;copy=v['actions'][1]['id'];png_hash=[]
 rid=host.start('session.call',dict(sessionId='clip',operation='extension:invoke-action',payload=dict(actionId=copy)))
 rpc=host.read();assert rpc['method']=='host.clipboard.writeImage';image=pathlib.Path(rpc['params']['file']);png_hash.append(hashlib.sha256(image.read_bytes()).hexdigest());assert image.read_bytes().startswith(b'\x89PNG\r\n\x1a\n')
 host.send(dict(type='cancel',requestId=rid,reason='QA cancel after dispatch'))
 while True:
  m=host.read()
  if m.get('id')==rid:assert 'error' in m;break
 # SDK may emit reverse cancel first; eventual late response is ignored.
 host.send(dict(type='response',id=rpc['id'],result={'ok':True}))
 v=host.call('clip',op='extension:refresh-session')['result'];assert image.exists();retry=next(a['id'] for a in v['actions'] if a['id'].startswith('retry-copy:'))
 before=len(host.reverse);host.call('clip',copy);assert len(host.reverse)==before
 def ack(m):
  assert m['method']=='host.clipboard.writeImage';file=pathlib.Path(m['params']['file']);assert hashlib.sha256(file.read_bytes()).hexdigest()==png_hash[0];return {'ok':True}
 host.call('clip',retry,reply=ack);assert not image.exists();before=len(host.reverse);host.call('clip',copy);host.call('clip',op='extension:refresh-session');assert len(host.reverse)==before
 assert host.request('session.dispose',dict(sessionId='clip'))['result']=={};assert 'error' in host.call('clip',copy)
 for name in ['selection']:
  count=len(host.reverse);v=host.open(name,name,lambda m:'selected exactly once')['result'];assert len(host.reverse)==count+1;host.call(name,op='extension:search-text-changed');assert len(host.reverse)==count+1;host.request('session.dispose',dict(sessionId=name))
 # Empty/whitespace and raw Unicode transport policy.
 v=host.open('empty','clipboard',lambda m:' \n\ufeff ')['result'];assert 'No text found' in v['message'];host.request('session.dispose',dict(sessionId='empty'))
 host.close()
 # Real EOF and closed stdout terminate the pinned runner, including pending acquisition.
 h=Pipe(root/'eof');h.start('session.open',dict(sessionId='pending',commandName='selection',preferences={},argumentValues={}));m=h.read();assert m['method']=='host.selection.read';h.close(eof=True)
 h=Pipe(root/'closed');h.p.stdout.close();h.send(dict(type='request',id='h:'+str(uuid.uuid4()),method='session.open',params=dict(sessionId='x',commandName='index',preferences={},argumentValues={})));assert h.p.wait(timeout=6)!=None;h.p.stdin.close()
print('Production executable OS-pipe protocol/cancel/late-ack/no-replay/denial/dispose/shutdown/EOF/closed-stdout passed; desktop is mock host.')
