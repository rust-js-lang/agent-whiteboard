import {test,expect} from '@playwright/test';
import {apply,coloredPixels,duck,image,read,ready,record,stroke} from './helpers';

test('native WebMCP draws real curved artwork, sees a PNG, and persists exact geometry',async({page},testInfo)=>{
 const errors:string[]=[];page.on('pageerror',error=>errors.push(error.message));await ready(page);
 const tools=await page.webmcp.tools();expect(tools.map(t=>t.name).sort()).toEqual(['apply_operations','export_board','get_board','get_elements','get_view','replay_drawing','set_viewport']);
 for(let i=0;i<duck.length;i++){const result=await apply(page,[duck[i]],'duck-'+i);expect(result.isError).not.toBe(true);}
 const snapshot=await read(page);expect(snapshot.sequence).toBe(duck.length);expect(snapshot.elements.length).toBe(duck.length);
 const png=await image(page);expect(coloredPixels(png),'Actual rendered duck pixels are required').toBeGreaterThan(15000);
 expect(coloredPixels(await page.locator('canvas').screenshot())).toBeGreaterThan(5000);
 await page.screenshot({path:testInfo.outputPath('duck.png')});
 await page.reload();await expect(page.getByTestId('agent-status')).toHaveText('Agent ready · 7 tools');expect(await read(page)).toEqual(snapshot);expect(await image(page)).toEqual(png);expect(errors).toEqual([]);
});

test('human freehand drawing saves its points, and undo/redo remain after reload',async({page})=>{
 await ready(page);const bounds=await page.locator('canvas').boundingBox();if(!bounds)throw Error('Missing canvas');
 await page.mouse.move(bounds.x+bounds.width*.3,bounds.y+bounds.height*.4);await page.mouse.down();await page.mouse.move(bounds.x+bounds.width*.65,bounds.y+bounds.height*.6,{steps:20});await page.mouse.up();
 await expect(page.getByTestId('sequence')).toHaveText('Revision 1');const state=await read(page);expect(state.elements[0].drawing.geometry.kind).toBe('stroke');expect(coloredPixels(await image(page))).toBeGreaterThan(100);
 await page.getByRole('button',{name:'Undo',exact:true}).click();await expect(page.getByTestId('sequence')).toHaveText('Revision 2');expect((await read(page)).elements).toHaveLength(0);
 await page.reload();await expect(page.getByTestId('agent-status')).toHaveText('Agent ready · 7 tools');await page.getByRole('button',{name:'Redo',exact:true}).click();await expect(page.getByTestId('sequence')).toHaveText('Revision 3');expect((await read(page)).elements[0].drawing).toEqual(state.elements[0].drawing);
});

test('invalid batches, stale edits, duplicate requests and collisions preserve accepted artwork',async({page})=>{
 await ready(page);const command={commandId:'once',operations:[stroke('a')]};expect((await page.webmcp.callTool('apply_operations',command)).isError).not.toBe(true);const saved=await record(page);
 expect((await page.webmcp.callTool('apply_operations',command)).isError).not.toBe(true);expect(await record(page)).toBe(saved);
 expect((await page.webmcp.callTool('apply_operations',{...command,operations:[stroke('other')]})).isError).toBe(true);
 expect((await apply(page,[stroke('new'),stroke('a')])).isError).toBe(true);expect((await read(page)).elements.map(e=>e.id)).toEqual(['a']);
 expect((await apply(page,[{op:'delete',id:'a',expectedVersion:0}])).isError).toBe(true);expect(await record(page)).toBe(saved);
});
