import { rmSync } from 'node:fs';
const database = `/private/tmp/agent-whiteboard-e2e-${process.pid}.db`;
const child = Bun.spawn(['./target/debug/whiteboard-server'], { cwd: new URL('..',import.meta.url).pathname, env: {...process.env,WHITEBOARD_TOKEN:'whiteboard-e2e-key-only',WHITEBOARD_DB:database,WHITEBOARD_BIND:'127.0.0.1:4081',WHITEBOARD_SECURE_COOKIE:'false'},stdout:'inherit',stderr:'inherit' });
const close=()=>{child.kill();};process.on('SIGINT',close);process.on('SIGTERM',close);
const status=await child.exited;
for(const suffix of ['', '-wal','-shm'])rmSync(database+suffix,{force:true});
process.exit(status??0);
