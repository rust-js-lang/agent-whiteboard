import {expect,test} from '@playwright/test';
import {apply,controlFrames,duck,frame,image,read,ready,record,stroke} from './helpers';

test('replay starts on blank white and reaches the exact final drawing without changing saved history',async({page})=>{
 await ready(page);for(let i=0;i<duck.length;i++)expect((await apply(page,[duck[i]])).isError).not.toBe(true);
 const before=await read(page),saved=await record(page),finalImage=await image(page);
 await controlFrames(page);
 const pending=page.webmcp.callTool('replay_drawing',{action:'start',durationMs:3000});
 await expect(page.locator('canvas')).toHaveAttribute('data-playback','replay');await expect(page.locator('canvas')).toHaveAttribute('data-element-count','0');
 await frame(page,0);await frame(page,1500);await expect.poll(()=>page.locator('canvas').getAttribute('data-element-count')).not.toBe('0');
 await frame(page,3100);expect((await pending).isError).not.toBe(true);await expect(page.locator('canvas')).toHaveAttribute('data-playback','live');
 expect(await record(page)).toBe(saved);expect(await read(page)).toEqual(before);expect(await image(page)).toEqual(finalImage);
 const second=page.webmcp.callTool('replay_drawing',{action:'start',durationMs:1000});await expect(page.locator('canvas')).toHaveAttribute('data-element-count','0');await frame(page,4000);await frame(page,5100);await second;expect(await record(page)).toBe(saved);
});

test('undo and redo are actor history operations and keyboard shortcuts work',async({page})=>{
 await ready(page);await apply(page,[stroke('a')]);await apply(page,[stroke('b')]);
 await page.locator('canvas').focus();await page.keyboard.press('Control+z');await expect(page.getByTestId('sequence')).toHaveText('Revision 3');await page.keyboard.press('Control+z');await expect(page.getByTestId('sequence')).toHaveText('Revision 4');expect((await read(page)).elements).toHaveLength(0);
 await page.keyboard.press('Control+Shift+z');await expect(page.getByTestId('sequence')).toHaveText('Revision 5');expect((await read(page)).elements.map(e=>e.id)).toEqual(['a']);await page.keyboard.press('Control+Shift+z');await expect(page.getByTestId('sequence')).toHaveText('Revision 6');expect((await read(page)).elements.map(e=>e.id)).toEqual(['a','b']);
});

test('a new accepted edit stops replay and keeps both the old art and new stroke',async({page})=>{
 await ready(page);await apply(page,[stroke('original')]);const pending=page.webmcp.callTool('replay_drawing',{action:'start',durationMs:10000});await expect(page.locator('canvas')).toHaveAttribute('data-playback','replay');expect((await apply(page,[stroke('new')])).isError).not.toBe(true);const stopped=await pending;expect(JSON.parse(stopped.content[0].text).cancelled).toBe(true);await expect(page.locator('canvas')).toHaveAttribute('data-playback','live');expect((await read(page)).elements.map(e=>e.id)).toEqual(['original','new']);
});

test('reduced motion keeps replay read-only without starting an animation',async({page})=>{
 await page.emulateMedia({reducedMotion:'reduce'});await ready(page);await apply(page,[stroke('quiet')]);const saved=await record(page);const result=await page.webmcp.callTool('replay_drawing',{action:'start'});expect(result.isError).not.toBe(true);await expect(page.locator('canvas')).toHaveAttribute('data-playback','live');expect(await record(page)).toBe(saved);
});

test('viewport changes affect the view but not saved art or history',async({page})=>{
 await ready(page);await apply(page,[stroke('art')]);const saved=await record(page),before=await read(page);expect((await page.webmcp.callTool('set_viewport',{cx:1900,cy:1000,zoom:2})).isError).not.toBe(true);expect(await record(page)).toBe(saved);expect((await read(page)).elements).toEqual(before.elements);expect((await read(page)).sequence).toBe(before.sequence);
});

test('reading or capturing committed art during replay does not replace the playback frame',async({page})=>{
 await ready(page);await apply(page,[stroke('a')]);await apply(page,[stroke('b')]);const saved=await record(page);await controlFrames(page);
 const pending=page.webmcp.callTool('replay_drawing',{action:'start',durationMs:2000});await expect(page.locator('canvas')).toHaveAttribute('data-playback','replay');await expect(page.locator('canvas')).toHaveAttribute('data-element-count','0');
 expect((await read(page)).elements).toHaveLength(2);await image(page);await expect(page.locator('canvas')).toHaveAttribute('data-playback','replay');await expect(page.locator('canvas')).toHaveAttribute('data-element-count','0');
 await frame(page,0);await frame(page,2100);await pending;expect(await record(page)).toBe(saved);
});
