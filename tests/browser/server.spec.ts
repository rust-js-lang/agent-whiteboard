import {expect,test,type Page} from '@playwright/test';
import {spawn,type ChildProcess} from 'node:child_process';
import {mkdirSync} from 'node:fs';
import {dirname,resolve} from 'node:path';
import {apply,read,stroke,image} from './helpers';
const token='whiteboard-e2e-key-only';
async function login(page:Page,room:string,origin='http://127.0.0.1:4081'){
 await page.goto(origin+'/#room='+room);await page.getByLabel('Server access key').fill(token);await page.getByRole('button',{name:'Join room',exact:true}).click();await expect(page.getByTestId('agent-status')).toHaveText('Agent ready · 7 tools');await expect(page.getByTestId('storage-status')).toContainText('server storage');
}
test('isolated agents share a real SQLite room, undo their own work, and reject stale conflicts',async({browser})=>{
 const a=await browser.newContext(),b=await browser.newContext();
 try{
  const alice=await a.newPage(),bob=await b.newPage(),room='shared-'+crypto.randomUUID();await Promise.all([login(alice,room),login(bob,room)]);
  const results=await Promise.all([apply(alice,[stroke('alice','#3d8065')]),apply(bob,[stroke('bob','#4083bf')])]);expect(results.every(r=>!r.isError)).toBe(true);
  await expect.poll(async()=>(await read(alice)).elements.length).toBe(2);expect((await read(bob)).elements).toHaveLength(2);expect(await image(alice)).toEqual(await image(bob));
  expect((await apply(alice,[{op:'undo'}])).isError).not.toBe(true);expect((await read(alice)).elements.map(e=>e.id)).toEqual(['bob']);await expect.poll(async()=>(await read(bob)).elements.length).toBe(1);
  await apply(alice,[{op:'redo'}]);const target=(await read(bob)).elements.find(e=>e.id==='alice')!;
  const drawing={...target.drawing,style:{...target.drawing.style,stroke:'#d97783'}};expect((await apply(bob,[{op:'replace',id:'alice',expectedVersion:target.version,drawing}])).isError).not.toBe(true);
  const undo=await apply(alice,[{op:'undo'}]);expect(undo.isError).toBe(true);expect((await read(alice)).elements.find(e=>e.id==='alice')?.drawing.style.stroke).toBe('#d97783');
 }finally{await a.close();await b.close();}
});

test('server requires authentication and enforces same-origin mutation requests',async({request})=>{
 const unauthenticated=await request.get('http://127.0.0.1:4081/api/rooms/private');expect(unauthenticated.status()).toBe(401);
 const foreign=await request.post('http://127.0.0.1:4081/api/session',{headers:{Origin:'https://example.invalid'},data:{token}});expect(foreign.status()).toBe(403);
 const wrong=await request.post('http://127.0.0.1:4081/api/session',{headers:{Origin:'http://127.0.0.1:4081'},data:{token:'wrong'}});expect(wrong.status()).toBe(401);
});

test('server storage and actor history survive a native process restart',async({page,request},testInfo)=>{
 const database=testInfo.outputPath('persistent.db');mkdirSync(dirname(database),{recursive:true});
 let server:ChildProcess|undefined;
 const start=async()=>{server=spawn(resolve('target/debug/whiteboard-server'),[],{cwd:process.cwd(),env:{...process.env,WHITEBOARD_DB:database,WHITEBOARD_BIND:'127.0.0.1:4082',WHITEBOARD_TOKEN:token,WHITEBOARD_SECURE_COOKIE:'false'},stdio:'ignore'});await expect.poll(async()=>{try{return(await request.get('http://127.0.0.1:4082/api/health')).ok();}catch{return false;}},{timeout:7000}).toBe(true);};
 const stop=async()=>{if(server&&server.exitCode===null){const child=server;await new Promise<void>(resolve=>{child.once('exit',()=>resolve());child.kill('SIGTERM');});}server=undefined;};
 try{await start();await login(page,'restart','http://127.0.0.1:4082');await apply(page,[stroke('persisted')]);const before=await read(page);await stop();await start();await page.reload();await expect(page.getByTestId('agent-status')).toHaveText('Agent ready · 7 tools');expect(await read(page)).toEqual(before);await apply(page,[{op:'undo'}]);expect((await read(page)).elements).toHaveLength(0);}finally{await stop();}
});
