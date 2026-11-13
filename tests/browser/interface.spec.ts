import {expect,test} from '@playwright/test';
import {apply,coloredPixels,read,ready,stroke} from './helpers';

test('shape and text controls create editable artwork',async({page})=>{
 await ready(page);const box=await page.locator('canvas').boundingBox();if(!box)throw Error('Canvas missing');
 for(const name of ['Line','Rectangle','Ellipse']){await page.getByRole('button',{name,exact:true}).click();await page.mouse.move(box.x+box.width*.35,box.y+box.height*.3);await page.mouse.down();await page.mouse.move(box.x+box.width*.6,box.y+box.height*.6,{steps:8});await page.mouse.up();await expect.poll(async()=>(await read(page)).elements.length).toBe(['Line','Rectangle','Ellipse'].indexOf(name)+1);}
 await page.getByRole('button',{name:'Text',exact:true}).click();await page.locator('canvas').click({position:{x:box.width*.5,y:box.height*.45}});await page.getByLabel('Board text').fill('An idea worth keeping');await page.getByRole('button',{name:'Place text',exact:true}).click();await expect.poll(async()=>(await read(page)).elements.length).toBe(4);expect(coloredPixels(await page.locator('canvas').screenshot())).toBeGreaterThan(100);
});

test('a browser without WebMCP can still draw, undo and redo',async({page})=>{
 await page.addInitScript(()=>Object.defineProperty(document,'modelContext',{value:undefined}));await page.goto('./');await expect(page.getByTestId('storage-status')).toContainText('Saved in this browser');await expect(page.getByTestId('agent-status')).toHaveText('WebMCP unavailable');
 const box=await page.locator('canvas').boundingBox();if(!box)throw Error('Canvas missing');await page.mouse.move(box.x+200,box.y+150);await page.mouse.down();await page.mouse.move(box.x+400,box.y+250,{steps:10});await page.mouse.up();await expect(page.getByTestId('sequence')).toHaveText('Revision 1');await page.getByRole('button',{name:'Undo',exact:true}).click();await expect(page.getByTestId('sequence')).toHaveText('Revision 2');await page.getByRole('button',{name:'Redo',exact:true}).click();await expect(page.getByTestId('sequence')).toHaveText('Revision 3');
});

test('mobile touch drawing and the responsive controls fit the viewport',async({browser})=>{
 const context=await browser.newContext({hasTouch:true,viewport:{width:390,height:844}});try{const page=await context.newPage();await ready(page);const box=await page.locator('canvas').boundingBox();if(!box)throw Error('Canvas missing');const cdp=await context.newCDPSession(page);await cdp.send('Input.dispatchTouchEvent',{type:'touchStart',touchPoints:[{x:190,y:box.y+180}]});await cdp.send('Input.dispatchTouchEvent',{type:'touchMove',touchPoints:[{x:280,y:box.y+280}]});await cdp.send('Input.dispatchTouchEvent',{type:'touchEnd',touchPoints:[]});await expect(page.getByTestId('sequence')).toHaveText('Revision 1');expect(coloredPixels(await page.locator('canvas').screenshot())).toBeGreaterThan(20);expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);}finally{await context.close();}
});

test('exports are real files and text is escaped in vector output',async({page})=>{
 await ready(page);await apply(page,[stroke('art'),{op:'create',id:'label',drawing:{geometry:{kind:'text',x:1000,y:1500,text:'<script>not executable</script>',size:80}}}]);
 const exported=await page.webmcp.callTool('export_board',{format:'svg'});const svg=JSON.parse(exported.content[0].text).data;expect(svg).toContain('&lt;script&gt;');expect(svg).not.toContain('<script>');
 await page.getByRole('button',{name:'Export',exact:true}).click();const event=page.waitForEvent('download');await page.getByRole('button',{name:/PNG image/}).click();const download=await event;expect(download.suggestedFilename()).toBe('whiteboard.png');expect(await download.failure()).toBeNull();
});

test('selection drag uses the release position and preserves editable geometry',async({page})=>{
 await ready(page);await apply(page,[{op:'create',id:'movable',drawing:{geometry:{kind:'path',segments:[{op:'M',x:1200,y:800},{op:'L',x:1800,y:800},{op:'L',x:1800,y:1400},{op:'L',x:1200,y:1400},{op:'Z'}]},style:{stroke:'#3d8065',fill:'#3d8065',width:10}}}]);
 const before=(await read(page)).elements[0];await page.getByRole('button',{name:'Select',exact:true}).click();const box=await page.locator('canvas').boundingBox();if(!box)throw Error('Canvas missing');const scale=Math.min(box.width/4096,box.height/2304),x=box.x+box.width/2+(1500-2048)*scale,y=box.y+box.height/2+(1100-1152)*scale;
 await page.mouse.move(x,y);await page.mouse.down();await page.mouse.move(x+110,y+55,{steps:10});await page.mouse.up();await expect(page.getByTestId('sequence')).toHaveText('Revision 2');const moved=(await read(page)).elements[0];expect(moved.drawing.geometry).toEqual(before.drawing.geometry);expect(moved.drawing.transform[4]).toBeCloseTo(110/scale);expect(moved.drawing.transform[5]).toBeCloseTo(55/scale);
 await page.getByRole('button',{name:'Undo',exact:true}).click();await expect(page.getByTestId('sequence')).toHaveText('Revision 3');expect((await read(page)).elements[0].drawing.transform).toEqual(before.drawing.transform);
});

test('clear is an explicit undoable document action',async({page})=>{
 await ready(page);await apply(page,[stroke('keep')]);const original=(await read(page)).elements[0].drawing;await page.getByRole('button',{name:'Clear board',exact:true}).click();await expect(page.getByRole('dialog',{name:'Clear board'})).toBeVisible();await page.getByRole('button',{name:'Clear canvas',exact:true}).click();await expect(page.getByTestId('sequence')).toHaveText('Revision 2');expect((await read(page)).elements).toHaveLength(0);await page.getByRole('button',{name:'Undo',exact:true}).click();await expect(page.getByTestId('sequence')).toHaveText('Revision 3');expect((await read(page)).elements[0].drawing).toEqual(original);
});
