// Execute own immutable TS/TSX and locked qrcode 1.5.4. React/native boundaries are labeled seams.
const fs=require('fs'),path=require('path'),vm=require('vm'),assert=require('assert/strict'),crypto=require('crypto');
const v1=path.resolve(__dirname,'../.work/v1');const esbuild=require(path.join(v1,'node_modules/esbuild'));
const productionQR=require(path.join(v1,'node_modules/qrcode'));const PNG=require(path.join(v1,'node_modules/pngjs')).PNG;
assert.equal(require(path.join(v1,'node_modules/qrcode/package.json')).version,'1.5.4');
let slots=[],pos=0,effects=[],pending=[],prefs={},calls=[],fetches=[];
const hooks={useState(init){const i=pos++;if(!(i in slots))slots[i]=typeof init==='function'?init():init;return[slots[i],v=>{slots[i]=typeof v==='function'?v(slots[i]):v;}];},useRef(v){const i=pos++;return slots[i]??(slots[i]={current:v});},useMemo(f){return f();},useCallback(f){return f;},useEffect(f){effects.push(f);}};
const jsx=(type,props)=>({type,props:props||{}});
function type(name){const f=props=>jsx(name,props);return new Proxy(f,{get(_,p){return type(name+'.'+String(p));}});}
// The real library stays genuine for in-memory encoding, but its file APIs are poisoned
// before any TSX is loaded. Accidentally returning productionQR cannot write a file.
const blocked=()=>{throw Error('Oracle boundary: uncaptured file API');};
assert.deepEqual(Object.keys(productionQR).sort(),['create','toCanvas','toString','toDataURL','toBuffer','toFile','toFileStream'].sort());
for(const key of ['toFile','toFileStream'])Object.defineProperty(productionQR,key,{value:blocked,writable:false,configurable:false});
const fakeRoot=path.resolve(__dirname,'../.work/oracle-'+crypto.randomUUID());
const home=path.join(fakeRoot,'home'),tmp=path.join(fakeRoot,'temp');
const fakeOS=Object.freeze({homedir:()=>home,tmpdir:()=>tmp});
function capturedPath(file){assert.equal(typeof file,'string');const resolved=path.resolve(file);assert([home,tmp].some(root=>resolved===root||resolved.startsWith(root+path.sep)),'Oracle boundary: path escaped fake home/temp');return resolved;}
const fakeFS=new Proxy(Object.freeze({mkdirSync(file){capturedPath(file);calls.push(['mkdir',file]);},writeFileSync(file,data){capturedPath(file);calls.push(['write',file,data]);}}),{get(target,key){if(key==='__esModule')return false;if(key in target)return target[key];throw Error('Oracle boundary: unsupported fs API '+String(key));}});
const QR=Object.freeze({create:productionQR.create,toBuffer:productionQR.toBuffer,toString:productionQR.toString,toDataURL:productionQR.toDataURL,
 toFile(file,text,options,callback){capturedPath(file);if(typeof options==='function'){callback=options;options={};}const work=(async()=>{const data=path.extname(file)==='.svg'?await productionQR.toString(text,{...options,type:'svg'}):await productionQR.toBuffer(text,options);calls.push(['write',file,data]);})();if(callback){work.then(()=>callback(null),callback);return;}return work;},
 toFileStream:blocked});
// Only explicitly routed dependencies are available to immutable source fixtures.
function builtin(p){if(p==='fs'||p==='node:fs')return fakeFS;if(p==='os'||p==='node:os')return fakeOS;if(p==='path'||p==='node:path')return path;throw Error('Oracle boundary: uncaptured require '+p);}
global.fetch=async()=>{throw Error('Oracle boundary: uncaptured fetch');};
const api={Form:type('Form'),Action:type('Action'),ActionPanel:type('ActionPanel'),Detail:type('Detail'),Icon:{},getPreferenceValues:()=>prefs,showToast:async(...v)=>{calls.push(['toast',...v]);return{};},Toast:{Style:{Animated:'animated',Success:'success',Failure:'failure'}},open:async p=>calls.push(['open',p]),Clipboard:{readText:async()=>{calls.push(['read']);return ' raw \n🙂 ';},copy:async p=>calls.push(['copy',p])},getSelectedText:async()=>{calls.push(['selection']);return ' select ';}};
function compile(source,filename,requireFn){const module={exports:{}};const code=esbuild.transformSync(source,{loader:filename.endsWith('.tsx')?'tsx':'ts',format:'cjs',jsx:'automatic'}).code;
 const wrapper=vm.runInThisContext('(function(require,module,exports,__filename,__dirname){'+code+'\n})',{filename});wrapper(requireFn,module,module.exports,filename,path.dirname(filename));return module.exports;}
const cache={};const map=JSON.parse(fs.readFileSync(path.join(v1,'node_modules/@raycast/utils/dist/module.js.map')));
function utility(name){if(cache['utils:'+name])return cache['utils:'+name];const i=map.sources.findIndex(p=>p==='src/'+name+'.tsx'||p==='src/'+name+'.ts');assert(i>=0);return cache['utils:'+name]=compile(map.sourcesContent[i],map.sources[i],p=>p==='react'?hooks:p==='@raycast/api'?api:p==='./useLatest'?utility('useLatest'):builtin(p));}
const utils={...utility('useForm'),showFailureToast:async(...v)=>calls.push(['failure',...v])};
function load(name){if(cache[name])return cache[name];const file=path.join(v1,'src',name);let src=fs.readFileSync(file,'utf8');return cache[name]=compile(src,file,p=>{if(p==='@raycast/api')return api;if(p==='@raycast/utils')return utils;if(p==='react')return hooks;if(p==='react/jsx-runtime')return {jsx,jsxs:jsx,Fragment:'Fragment'};if(p==='qrcode')return QR;if(p.startsWith('.')){const f=p.slice(2);return load(fs.existsSync(path.join(v1,'src',f+'.ts'))?f+'.ts':f+'.tsx');}return builtin(p);});}
const config=load('config.ts'),url=load('url.ts'),q=load('utils.tsx');
const colors=['#000000','#fff',' AbC ','0A66C2','invalid','#ffff00','#808080','#aaa','\u0085#fff\u0085','\ufeff#AbC\ufeff'];
const draft=url=>({url,format:'png',color:'custom',customColor:'#000000',shorten:false,utmEnabled:true,utmSource:' a ',utmMedium:'',utmCampaign:'a+b',utmTerm:'',utmContent:'é'});
const urls=['plain text','mailto:hi@example.com',' HTTPS://EXAMPLE.COM:443/a?q=~+%20#x ','https://example.com/?utm_source=1&x=y&utm_source=2','http:example.com','https://例え.テスト/?x=%FF','https://user:pass@example.com/path','\ufeffhttps://example.com\ufeff'].map(text=>{const d=draft(text);return{draft:d,result:url.appendUtmParams(text,{source:d.utmSource,medium:d.utmMedium,campaign:d.utmCampaign,term:d.utmTerm,content:d.utmContent})};});
const names=['https://www.example.com:443/a','https://a.com/x\nhttp://b.test/y','user@example.com','plain text','a:b','ftp://x/y','https://u:p@www.Example.com/a','你好🙂','one\ntwo','a:\rb','a:\u2028b','a:\u2029http://b.test','///'];
const result={colors:colors.map(raw=>({raw,resolved:config.resolveColorPreference(raw),low:config.isLowContrast(raw)})),urls,names:names.map(text=>{try{return{text,result:path.basename(q.getQRCodePath(text))};}catch{return{text,result:null};}}),qr:[]};
function render(){pos=0;return load('index.tsx').default();}
const flatten=n=>!n?[]:Array.isArray(n)?n.flatMap(flatten):typeof n==='object'?[n,...flatten(n.props?.children)]:[];
function reset(){slots=[];pos=0;effects=[];calls=[];pending=[];}
// Warm compilation before the execution tripwire (esbuild itself can use private IPC files).
load('index.tsx');load('common.tsx');
const restore=[];let escapedWrites=0;
for(const target of [fs,fs.promises])for(const key of ['writeFile','writeFileSync','appendFile','appendFileSync','createWriteStream','mkdir','mkdirSync','mkdtemp','mkdtempSync','open','openSync','copyFile','copyFileSync','rename','renameSync','link','linkSync','symlink','symlinkSync','truncate','truncateSync'])if(typeof target[key]==='function'){
 const original=target[key];restore.push(()=>{target[key]=original;});target[key]=()=>{escapedWrites++;throw Error('Oracle boundary: real fs '+key);};
}
(async()=>{
// Regression: deliberately misrouted real QR APIs and unknown stream APIs fail before
// touching either a path or a caller's stream. The in-memory encoder remains genuine.
const sentinel=path.join(tmp,'must-never-exist.png');let sinkCalls=0;
const sink={emit(){sinkCalls++;},write(){sinkCalls++;},end(){sinkCalls++;}};
for(const library of [productionQR,QR])assert.throws(()=>library.toFileStream(sink,'guard'),/Oracle boundary/);
assert.throws(()=>productionQR.toFile(sentinel,'guard',()=>{sinkCalls++;}),/Oracle boundary/);
assert.throws(()=>builtin('node:fs').createWriteStream(sentinel),/Oracle boundary/);
assert.throws(()=>builtin('fs/promises'),/Oracle boundary/);
assert.throws(()=>QR.toFile(path.resolve(fakeRoot,'escaped.png'),'guard'),/escaped fake/);
assert.equal(sinkCalls,0);assert.equal(escapedWrites,0);
// A future misrouted direct fs callback is stopped before invoking Node's producer.
assert.throws(()=>fs.writeFileSync(sentinel,'blocked'),/Oracle boundary: real fs/);
assert.equal(escapedWrites,1);escapedWrites=0;
await QR.toFile(sentinel,'guard');
await new Promise((resolve,reject)=>QR.toFile(sentinel,'guard',{},error=>error?reject(error):resolve()));
assert.equal(calls.filter(c=>c[0]==='write'&&c[1]===sentinel).length,2);
assert(!fs.existsSync(sentinel));
for(const text of ['1234567890','HELLO WORLD','hello world','https://example.com/a?x=1','Қазақша 🙂','01234'.repeat(60),'ABCDEF'.repeat(30),'a'.repeat(180)]){
 const matrix=QR.create(text);const opts=config.buildQrOptions({color:'#1D8348',preview:false});const png=PNG.sync.read(await QR.toBuffer(text,opts));result.qr.push({text,size:matrix.modules.size,version:matrix.version,ecc:matrix.errorCorrectionLevel,mask:matrix.maskPattern,matrix:Array.from(matrix.modules.data),pixelSha256:crypto.createHash('sha256').update(png.data).digest('hex')});
}
for(const primary of ['save','copy','inline',undefined]){reset();prefs={defaultColor:'#AbC',defaultFormat:'svg',primaryAction:primary};let tree=render();const all=flatten(tree);const f=all.find(n=>n.props.id==='format');assert.equal(f.props.value,null);assert.equal(f.props.storeValue,true);assert.equal(all.find(n=>n.props.id==='color').props.value,'custom');assert.equal(all.find(n=>n.props.id==='customColor').props.value,'#AAbbCC');const actions=flatten(tree.props.actions).filter(n=>n.props.onSubmit);assert.deepEqual(actions.map(a=>a.props.title),primary==='save'?['Generate and Save','Generate and Show','Generate and Copy to Clipboard']:primary==='copy'?['Generate and Copy to Clipboard','Generate and Save','Generate and Show']:['Generate and Show','Generate and Save','Generate and Copy to Clipboard']);}
// Exercise real callback and real useForm validation. Native fs/open/clipboard are capture seams.
for(const actionTitle of ['Generate and Save','Generate and Show','Generate and Copy to Clipboard']){reset();prefs={};let tree=render();const fields=flatten(tree);fields.find(n=>n.props.id==='url').props.onChange('hello');render();const actions=flatten(render().props.actions);const action=actions.find(n=>n.props.title===actionTitle);action.props.onSubmit({...draft('hello'),utmEnabled:false,inline:false});await new Promise(r=>setTimeout(r,80));assert(calls.some(c=>c[0]==='toast'));if(actionTitle.includes('Show'))assert(slots[0]?.startsWith('data:image/png'));if(actionTitle.includes('Save'))assert(calls.some(c=>c[0]==='open'));if(actionTitle.includes('Copy'))assert(calls.some(c=>c[0]==='copy'));if(!actionTitle.includes('Show'))assert(calls.some(c=>c[0]==='write'));for(const c of calls)if(c[0]==='write'||c[0]==='mkdir')capturedPath(c[1]);}
reset();let tree=render();const invalid=flatten(tree.props.actions).find(n=>n.props.title==='Generate and Show');invalid.props.onSubmit({...draft('hello'),customColor:'invalid'});await new Promise(r=>setTimeout(r,10));assert(!calls.some(c=>c[0]==='copy'||c[0]==='open'));
reset();await q.saveQRCode({url:'guard svg',format:'svg'});assert(calls.some(c=>c[0]==='write'&&c[1].endsWith('.svg')&&c[2].startsWith('<svg')));
assert.equal(await q.generateQRCode({}),undefined);await assert.rejects(()=>q.generateQRCode({URL:''}));
// Actual URL shortener function with mocked fetch, never user content on the network.
let outcomes=[{ok:false,text:async()=> 'error'},{ok:true,text:async()=> 'garbage'},{ok:true,text:async()=> 'https://short.test/x'}];global.fetch=async endpoint=>{fetches.push(endpoint);return outcomes.shift();};assert.equal(await url.shortenUrl('https://example.com'),'https://short.test/x');assert(fetches[0].startsWith('https://is.gd/'));assert(fetches[1].startsWith('https://da.gd/'));assert(fetches[2].startsWith('https://tinyurl.com/'));
global.fetch=async()=>{throw Error('test outage');};await assert.rejects(()=>url.shortenUrl('https://example.com'));
// Common useEffect acquisition once in source, raw input/white preview, real callbacks.
for(const from of ['clipboard','selection']){reset();pos=0;const common=load('common.tsx').default({from});for(const f of effects.splice(0))f();await new Promise(r=>setTimeout(r,80));assert.equal(calls.filter(c=>c[0]===(from==='clipboard'?'read':'selection')).length,1);pos=0;tree=load('common.tsx').default({from});const actions=flatten(tree.props.actions).filter(n=>n.props.onAction);assert.deepEqual(actions.map(a=>a.props.title),['Save to Downloads','Copy to Clipboard']);await actions[0].props.onAction();await actions[1].props.onAction();assert(calls.some(c=>c[0]==='write'));assert(calls.some(c=>c[0]==='copy'));assert(!calls.some(c=>c[0]==='open'));}
assert.equal(escapedWrites,0,'Source callbacks attempted real filesystem effects');
for(const c of calls)if(c[0]==='write'||c[0]==='mkdir')capturedPath(c[1]);
// Restore synchronously only after all source callbacks finish, for this authorized fixture.
for(const undo of restore)undo();
fs.writeFileSync(path.join(__dirname,'fixtures/oracle.json'),JSON.stringify(result,null,2)+'\n');
console.log('Own TS/TSX/config/utils + locked useForm/qrcode oracle passed; fake home/temp, guarded QR/fs callback regression and zero escaped file writes; desktop/React hooks/fetch are capture seams.');
})().catch(e=>{console.error(e);process.exit(1)});
