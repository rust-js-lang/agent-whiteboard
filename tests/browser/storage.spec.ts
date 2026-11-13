import {expect,test} from '@playwright/test';
import {PNG} from 'pngjs';
import {apply,image,read,ready,record,rectangle,stroke} from './helpers';

function center(buffer:Buffer){const png=PNG.sync.read(buffer);const offset=(Math.floor(png.height/2)*png.width+Math.floor(png.width/2))*4;return [...png.data.slice(offset,offset+3)];}
test('partial erasing reveals lower ink, leaves new ink intact, and supports undo and SVG export',async({page})=>{
 await ready(page);await apply(page,[rectangle('bottom','#4083bf'),rectangle('top','#d97783')]);
 const region={x:1000,y:700,width:1000,height:800};expect(center(await image(page,region))).toEqual([217,119,131]);
 const state=await read(page);const top=state.elements.find(e=>e.id==='top')!;
 expect((await apply(page,[{op:'erase',targets:[{id:top.id,expectedVersion:top.version}],points:[{x:1500,y:1100}],radius:150}])).isError).not.toBe(true);
 expect(center(await image(page,region))).toEqual([64,131,191]);
 const erasedImage=await image(page,region);await apply(page,[stroke('new','#3d8065',[{x:1500,y:1100}])]);expect(center(await image(page,region))).toEqual([61,128,101]);
 await apply(page,[{op:'undo'}]);expect(await image(page,region)).toEqual(erasedImage);await apply(page,[{op:'undo'}]);expect(center(await image(page,region))).toEqual([217,119,131]);await apply(page,[{op:'redo'}]);expect(center(await image(page,region))).toEqual([64,131,191]);
 const exported=await page.webmcp.callTool('export_board',{format:'svg'});expect(JSON.parse(exported.content[0].text).data).toContain('<mask');
 await page.reload();await expect(page.getByTestId('agent-status')).toHaveText('Agent ready · 7 tools');expect(center(await image(page,region))).toEqual([64,131,191]);
});

test('simultaneous tabs commit independently without losing strokes',async({page,context})=>{
 await ready(page);const second=await context.newPage();await ready(second);
 const results=await Promise.all([apply(page,[stroke('a')]),apply(second,[stroke('b')])]);expect(results.every(r=>!r.isError)).toBe(true);
 expect((await read(page)).elements.map(e=>e.id).sort()).toEqual(['a','b']);expect((await read(second)).sequence).toBe(2);
 await expect(page.locator('canvas')).toHaveAttribute('data-live-sequence','2');
});

test('an aborted storage transaction is not acknowledged or shown as saved',async({page})=>{
 await page.addInitScript(()=>{const original=IDBObjectStore.prototype.put;IDBObjectStore.prototype.put=function(...args:Parameters<IDBObjectStore['put']>){const request=Reflect.apply(original,this,args);const tx=this.transaction;request.addEventListener('success',()=>tx.abort());return request;};});
 await ready(page);const result=await apply(page,[stroke('lost')]);expect(result.isError).toBe(true);expect((await read(page)).sequence).toBe(0);expect((await read(page)).elements).toHaveLength(0);await expect(page.locator('canvas')).toHaveAttribute('data-element-count','0');
});

test('denied browser storage is explicitly temporary and reload never pretends to retain it',async({page})=>{
 await page.addInitScript(()=>Object.defineProperty(window,'indexedDB',{get(){throw new DOMException('Blocked','SecurityError');}}));
 await ready(page);await expect(page.getByTestId('storage-status')).toContainText('Temporary');await apply(page,[stroke('temporary')]);expect((await read(page)).elements).toHaveLength(1);await page.reload();await expect(page.getByTestId('agent-status')).toHaveText('Agent ready · 7 tools');expect((await read(page)).elements).toHaveLength(0);
});

test('future or corrupt saved data is preserved and blocks editing',async({page})=>{
 await ready(page);
 const future=JSON.stringify({...JSON.parse(await record(page)),version:999});
 await page.evaluate(async future=>{const db=await new Promise<IDBDatabase>((resolve,reject)=>{const request=indexedDB.open('agent-whiteboard',1);request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);});await new Promise<void>((resolve,reject)=>{const tx=db.transaction('boards','readwrite');tx.objectStore('boards').put(future,'local');tx.oncomplete=()=>resolve();tx.onerror=()=>reject(tx.error);});db.close();},future);
 await page.reload();await expect(page.getByRole('alert')).toContainText('unsupported format');await expect(page.getByRole('button',{name:'Undo',exact:true})).toBeDisabled();
 const retained=await page.evaluate(async()=>{const db=await new Promise<IDBDatabase>(resolve=>{const r=indexedDB.open('agent-whiteboard',1);r.onsuccess=()=>resolve(r.result);});const result=await new Promise(resolve=>{const r=db.transaction('boards').objectStore('boards').get('local');r.onsuccess=()=>resolve(r.result);});db.close();return result;});expect(retained).toBe(future);
});

test('installed app reopens with network disabled and keeps the drawing',async({page,context})=>{
 await ready(page);await apply(page,[stroke('offline')]);const state=await read(page);
 await page.evaluate(async()=>{await navigator.serviceWorker.ready;if(!navigator.serviceWorker.controller)await new Promise<void>(resolve=>navigator.serviceWorker.addEventListener('controllerchange',()=>resolve(),{once:true}));});
 await context.setOffline(true);await page.reload();await expect(page.getByTestId('agent-status')).toHaveText('Agent ready · 7 tools');expect(await read(page)).toEqual(state);await apply(page,[stroke('also-offline')]);expect((await read(page)).elements).toHaveLength(2);
});

test('eraser masks move with their selected artwork',async({page})=>{
 await ready(page);await apply(page,[rectangle('background','#4083bf',500,300,2500,1700),rectangle('moving','#d97783')]);
 await apply(page,[{op:'erase',targets:[{id:'moving',expectedVersion:1}],points:[{x:1500,y:1100}],radius:150}]);const element=(await read(page)).elements.find(e=>e.id==='moving')!;
 await apply(page,[{op:'replace',id:'moving',expectedVersion:element.version,drawing:{...element.drawing,transform:[1,0,0,1,300,200]}}]);
 expect(center(await image(page,{x:1700,y:1200,width:200,height:200}))).toEqual([64,131,191]);
});
