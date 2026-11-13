import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, relative } from 'node:path';
const root = join(import.meta.dir,'../dist');
function files(directory:string):string[]{return readdirSync(directory,{withFileTypes:true}).flatMap(entry=>entry.isDirectory()?files(join(directory,entry.name)):[relative(root,join(directory,entry.name)).replaceAll('\\','/')]);}
const assets = files(root).filter(file=>file!=='sw.js');
const version = Bun.hash(assets.map(file=>file+':'+Bun.hash(readFileSync(join(root,file)))).join('|')).toString(16);
writeFileSync(join(root,'sw.js'),`const CACHE='agent-whiteboard-${version}';
const ROOT=new URL('./',self.location.href);const ASSETS=${JSON.stringify(['./',...assets])};
self.addEventListener('install',event=>event.waitUntil(caches.open(CACHE).then(cache=>cache.addAll(ASSETS))));
self.addEventListener('activate',event=>event.waitUntil(self.clients.claim()));
self.addEventListener('fetch',event=>{const request=event.request,url=new URL(request.url);if(request.method!=='GET'||url.origin!==ROOT.origin||!url.pathname.startsWith(ROOT.pathname)||url.pathname.startsWith(new URL('api/',ROOT).pathname))return;
if(request.mode==='navigate'){event.respondWith(fetch(request).catch(()=>caches.open(CACHE).then(cache=>cache.match(new URL('index.html',ROOT).href))));return;}
event.respondWith(caches.open(CACHE).then(async cache=>(await cache.match(request,{ignoreVary:true}))||fetch(request)));});
`);
console.log(`Offline bundle: ${assets.length} assets (${version})`);
