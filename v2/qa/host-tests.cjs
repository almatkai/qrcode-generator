// Execute captured COMMITTED actual FormRuntime/DetailRuntime/Surface through React.
// Only unrelated primitives, list/grid, hooks and overlay presentation are stubs.
const fs=require('fs'),path=require('path'),vm=require('vm'),assert=require('assert/strict');
const React=require('react'),TR=require('react-test-renderer'),{JSDOM}=require('jsdom'),esbuild=require('esbuild');
const {act}=TR;const dom=new JSDOM('<!doctype html><html><body></body></html>');global.window=dom.window;global.document=dom.window.document;
window.tezbar={setSuppressBlurHide:async()=>{}};
const empty=()=>null;
function loadTree(which){const root=path.resolve(__dirname,'../integration',which,'host/src/renderer');const cache={};
 function load(relative){if(cache[relative])return cache[relative];const file=path.join(root,relative);let source=fs.readFileSync(file,'utf8');if(relative.endsWith('detail-runtime.tsx'))source+='\nexport {parseImageOnlyMarkdown,resolveMarkdownImageSrc};';
 const code=esbuild.transformSync(source,{loader:'tsx',format:'cjs',jsx:'automatic'}).code;const m={exports:{}};cache[relative]=m.exports;
 const req=p=>{if(p==='react')return React;if(p==='react/jsx-runtime')return require('react/jsx-runtime');if(p.endsWith('primitives'))return new Proxy({cx:(...v)=>v.join(' ')},{get(t,p){return t[p]||empty;}});if(p.endsWith('colorConverter'))return{buildColorFormatRows:()=>[]};
 if(p==='./hooks')return{NavigationContext:React.createContext({}),setRuntimeContext(){}};
 if(p==='./action-runtime-registry')return{ActionRegistryContext:React.createContext({})};
 if(p==='./action-runtime')return{Action:empty,ActionPanel:empty,ActionPanelOverlay:props=>React.createElement('overlay-seam',props)};
 if(p==='./grid-runtime')return{GridRuntime:empty};if(p==='./list-runtime')return{ListRuntime:empty};
 if(p.endsWith('/Markdown'))return{Markdown:empty};
 if(p==='./form-runtime'||p==='./detail-runtime')return load('src/raycast-api/'+p.slice(2)+'.tsx');throw Error('Unexpected import '+p);};
 vm.runInThisContext('(function(require,module,exports){'+code+'\n})',{filename:file})(req,m,m.exports);cache[relative]=m.exports;return m.exports;}
 return {form:load('src/raycast-api/form-runtime.tsx'),detail:load('src/raycast-api/detail-runtime.tsx'),surface:load('src/raycast-api/index.tsx')};}
const b=loadTree('baseline'),a=loadTree('proposed');
const node=(type,props)=>({type,props,children:[]});
function formRoot(revision=0,one='',two=''){return{type:'Form',props:{__tezbarDraftRevision:revision},children:[node('Form.TextField',{id:'url',title:'URL',value:one,actionId:'field:url'}),node('Form.TextField',{id:'customColor',title:'Custom',value:two,actionId:'field:customColor'}),node('Form.Checkbox',{id:'utmEnabled',title:'UTM',value:false,actionId:'field:utmEnabled'})]};}
for(const [label,tree] of [['before',b],['after',a]]){
 let changes=[];const ref=React.createRef();let root;act(()=>{root=TR.create(React.createElement(tree.form.FormRuntime,{root:formRoot(),title:'QR',ref,onBack(){},onSubmitForm(){},onChangeField:(...args)=>changes.push(args),onInvokeAction(){},onOpenActions(){}}));});
 act(()=>root.root.findAllByType('input')[0].props.onChange({target:{value:'new url'}}));
 act(()=>root.root.findAllByType('input')[1].props.onChange({target:{value:'#abc'}}));
 act(()=>root.root.findAllByType('input')[2].props.onChange({target:{checked:true}}));
 assert.equal(changes.length,3);
 act(()=>root.update(React.createElement(tree.form.FormRuntime,{root:formRoot(1,'new url',''),title:'QR',ref,onBack(){},onSubmitForm(){},onChangeField:(...args)=>changes.push(args),onInvokeAction(){},onOpenActions(){}})));
 assert.equal(ref.current.getValues().customColor,label==='before'?'':'#abc');
 if(label==='after'){assert.equal(changes[1][2].url,'new url');assert.equal(changes[1][2].customColor,'#abc');assert.equal(changes[1][2].__tezbarDraftRevision,2);assert.equal(ref.current.getValues().__tezbarDraftRevision,3);assert.equal(changes[2][2].utmEnabled,true);assert.equal(ref.current.getValues().utmEnabled,true);}
 act(()=>root.unmount());
 // Detail real DOM-handler boundary: browser MouseEvent is not an action ID after patch.
 let id;act(()=>{root=TR.create(React.createElement(tree.detail.DetailRuntime,{root:node('Detail',{markdown:'hello'}),title:'QR',onBack(){},onRunPrimaryAction:x=>{id=x;},onOpenActions(){}}));});
 const buttons=root.root.findAllByType('button');const button=buttons.find(x=>x.children.includes('Enter'));assert(button);const event=new window.MouseEvent('click');act(()=>button.props.onClick(event));assert.equal(id,label==='before'?event:undefined);act(()=>root.unmount());
 // Every one of three real Surface overlay submit routes must receive current fields.
 let sent=[];let surface;act(()=>{surface=TR.create(React.createElement(tree.surface.ExtensionRuntimeSurface,{sessionId:'x',title:'QR',extensionId:'x',commandName:'index',root:formRoot(),actions:['show','save','copy'].map(id=>({id,title:id,kind:'submit-form'})),onBack(){},onSearchTextChanged(){},onLoadMore(){},onInvokeAction:(...v)=>sent.push(v)}));});
 act(()=>surface.root.findAllByType('input')[0].props.onChange({target:{value:'current'}}));
 for(const id of ['show','save','copy']){act(()=>window.dispatchEvent(new window.KeyboardEvent('keydown',{key:'k',metaKey:true})));const overlay=surface.root.findByType('overlay-seam');act(()=>overlay.props.onExecute({id,title:id,kind:'submit-form'}));const last=sent.at(-1);assert.equal(last[0],id);assert.equal(last[1]?.url,label==='after'||id==='show'?'current':undefined);}
 act(()=>surface.unmount());
}
const png='data:image/png;base64,iVBORw0KGgo=';
assert.equal(a.detail.parseImageOnlyMarkdown('![qrcode]('+png+')','').src,png);
assert.equal(a.detail.parseImageOnlyMarkdown('![qrcode]('+png+'?raycast-height=350)','').src,png+'?raycast-height=350');
assert.equal(a.detail.parseImageOnlyMarkdown('![qrcode](assets/qr.png?raycast-height=350)','/package/resources').src,'file:///package/resources/assets/qr.png');
assert.equal(a.detail.parseImageOnlyMarkdown('![qrcode](data:image/png;base64,x) user injection',''),null);
// Registry resource routing: extract and compile actual Rust registry separately in runtime-host test.
console.log('Actual committed renderer before/after: drafts, all three submit actions, Detail MouseEvent, image path/data-URL handling passed.');
// Actual outer response gate: stale preview/Form views cannot replace a newer queued draft.
const outer=fs.readFileSync(path.resolve(__dirname,'../integration/proposed/host/src/renderer/ExtensionRuntimeView.tsx'),'utf8');
const gateSource=outer.match(/export function isStaleDraftResponse[\s\S]+?^}/m)?.[0];assert(gateSource,'Missing actual response gate');const moduleGate={exports:{}};vm.runInThisContext('(function(module,exports){'+esbuild.transformSync(gateSource,{loader:'ts',format:'cjs'}).code+'\n})')(moduleGate,moduleGate.exports);
assert(moduleGate.exports.isStaleDraftResponse(3,{root:{type:'Detail',props:{__tezbarDraftRevision:2}}}));assert(moduleGate.exports.isStaleDraftResponse(3,{root:{type:'Form',props:{__tezbarDraftRevision:1}}}));assert(!moduleGate.exports.isStaleDraftResponse(3,{root:{props:{__tezbarDraftRevision:3}}}));assert(!moduleGate.exports.isStaleDraftResponse(3,{root:{props:{}}}));
