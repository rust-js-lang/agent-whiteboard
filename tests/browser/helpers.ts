import {expect,type Page} from '@playwright/test';
import {PNG} from 'pngjs';
import type {Command,Drawing,Operation,Snapshot} from '../protocol';
export async function ready(page:Page,url='./'){await page.goto(url);await expect(page.getByTestId('agent-status')).toHaveText('Agent ready · 7 tools');}
export async function read(page:Page):Promise<Snapshot>{const result=await page.webmcp.callTool('get_board',{});expect(result.isError).not.toBe(true);return JSON.parse(result.content[0].text);}
export async function apply(page:Page,operations:Operation[],commandId:string=crypto.randomUUID()){return page.webmcp.callTool('apply_operations',{commandId,operations});}
export function stroke(id:string,color='#243b35',points=[{x:1000,y:800},{x:2000,y:1200},{x:2800,y:700}]):Operation{return {op:'create',id,drawing:{geometry:{kind:'stroke',points},style:{stroke:color,width:28}}};}
export function rectangle(id:string,color:string,x=1000,y=700,w=1000,h=800):Operation{return {op:'create',id,drawing:{geometry:{kind:'path',segments:[{op:'M',x,y},{op:'L',x:x+w,y},{op:'L',x:x+w,y:y+h},{op:'L',x,y:y+h},{op:'Z'}]},style:{stroke:color,fill:color,width:8}}};}
export function coloredPixels(buffer:Buffer){const png=PNG.sync.read(buffer);let pixels=0;for(let i=0;i<png.data.length;i+=4){const r=png.data[i],g=png.data[i+1],b=png.data[i+2];if(Math.max(r,g,b)-Math.min(r,g,b)>20&&Math.min(r,g,b)<180)pixels++;}return pixels;}
export function changedPixels(first:Buffer,second:Buffer){const a=PNG.sync.read(first),b=PNG.sync.read(second);expect([a.width,a.height]).toEqual([b.width,b.height]);let count=0;for(let i=0;i<a.data.length;i+=4)if(Math.abs(a.data[i]-b.data[i])+Math.abs(a.data[i+1]-b.data[i+1])+Math.abs(a.data[i+2]-b.data[i+2])>40)count++;return count;}
export async function image(page:Page,bounds?:object){const result=await page.webmcp.callTool('get_view',{...(bounds?{bounds}:{}),maxSize:1024});expect(result.isError).not.toBe(true);const content=result.content.find((item:{type:string})=>item.type==='image');expect(content).toBeTruthy();return Buffer.from(content.data,'base64');}
export async function record(page:Page):Promise<string>{const result=await page.webmcp.callTool('export_board',{format:'json'});expect(result.isError).not.toBe(true);return JSON.parse(result.content[0].text).data;}
export async function controlFrames(page:Page){await page.evaluate(()=>{let next=0;const frames=new Map<number,FrameRequestCallback>();window.requestAnimationFrame=callback=>{frames.set(++next,callback);return next;};window.cancelAnimationFrame=id=>{frames.delete(id);};Object.assign(window,{__stepFrame(time:number){const callbacks=[...frames.values()];frames.clear();callbacks.forEach(callback=>callback(time));}});});}
export async function frame(page:Page,time:number){await page.evaluate(time=>Reflect.get(window,'__stepFrame')(time),time);}
export const duck:Operation[]=[
 {op:'create',id:'duck-water',drawing:{geometry:{kind:'path',segments:[{op:'M',x:1160,y:1620},{op:'C',cx1:1450,cy1:1540,cx2:1800,cy2:1720,x:2050,y:1620},{op:'C',cx1:2260,cy1:1540,cx2:2500,cy2:1700,x:2760,y:1600}]},style:{stroke:'#b5d9db',width:22}}},
 {op:'create',id:'duck-body',drawing:{geometry:{kind:'path',segments:[{op:'M',x:1290,y:1210},{op:'C',cx1:1510,cy1:1080,cx2:1660,cy2:1210,x:1930,y:1080},{op:'C',cx1:1990,cy1:900,cx2:1880,cy2:640,x:2140,y:630},{op:'C',cx1:2410,cy1:620,cx2:2460,cy2:990,x:2250,y:1110},{op:'C',cx1:2460,cy1:1460,cx2:2080,cy2:1650,x:1700,y:1530},{op:'C',cx1:1450,cy1:1460,cx2:1350,cy2:1350,x:1290,y:1210},{op:'Z'}]},style:{stroke:'#826834',fill:'#f4cc66',width:14}}},
 {op:'create',id:'duck-beak',drawing:{geometry:{kind:'path',segments:[{op:'M',x:2330,y:830},{op:'Q',cx:2510,cy:830,x:2580,y:940},{op:'Q',cx:2430,cy:1000,x:2320,y:960},{op:'Z'}]},style:{stroke:'#aa6639',fill:'#e99d50',width:13}}},
 {op:'create',id:'duck-wing',drawing:{geometry:{kind:'path',segments:[{op:'M',x:1590,y:1270},{op:'C',cx1:1750,cy1:1190,cx2:1940,cy2:1290,x:2060,y:1240},{op:'C',cx1:1970,cy1:1450,cx2:1720,cy2:1470,x:1590,y:1270},{op:'Z'}]},style:{stroke:'#aa823e',fill:'#edba51',width:12}}},
 {op:'create',id:'duck-eye',drawing:{geometry:{kind:'stroke',points:[{x:2240,y:807}]},style:{stroke:'#313b32',width:40}}},
 {op:'create',id:'duck-eye-shine',drawing:{geometry:{kind:'stroke',points:[{x:2247,y:799}]},style:{stroke:'#ffffff',width:11}}},
 {op:'create',id:'duck-label',drawing:{geometry:{kind:'text',x:1530,y:1850,text:'a little room to wonder.',size:66},style:{stroke:'#7d9380'}}},
];
export type {Command,Drawing};
