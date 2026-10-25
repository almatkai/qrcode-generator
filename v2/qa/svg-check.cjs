// Independent SVG rasterization with resvg. Compares decoded pixels/module geometry to Rust PNG.
const fs=require('fs'),path=require('path'),assert=require('assert/strict'),{Resvg}=require('@resvg/resvg-js');
const PNG=require(path.resolve(__dirname,'../.work/v1/node_modules/pngjs')).PNG;
const root=path.resolve(__dirname,'../.work/qr-output');
for(let i=0;i<8;i++){
 const svg=fs.readFileSync(path.join(root,i+'.svg'));const raster=new Resvg(svg,{fitTo:{mode:'width',value:512}}).render().asPng();const png=PNG.sync.read(fs.readFileSync(path.join(root,i+'.png'))),vector=PNG.sync.read(raster);
 assert.equal(vector.width,512);assert.equal(vector.height,512);
 // resvg anti-aliases edges even with crispEdges on path; sample module centres and quiet-zone.
 const matrix=JSON.parse(fs.readFileSync(path.join(root,'comparison.json')))[i];const size=matrix.rustSize,extent=size+8;
 for(let y=0;y<extent;y++)for(let x=0;x<extent;x++){const px=Math.floor((x+.5)*512/extent),py=Math.floor((y+.5)*512/extent),n=(py*512+px)*4;assert.equal(vector.data[n+3],png.data[n+3]);if(png.data[n+3])assert.deepEqual([...vector.data.subarray(n,n+3)],[...png.data.subarray(n,n+3)]);}
 fs.writeFileSync(path.join(root,i+'-svg.png'),raster);
}
console.log('Independent resvg SVG rasterization: dimensions, transparent background, palette, module centres and quiet-zone match eight QR outputs.');
