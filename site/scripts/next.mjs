import {spawn} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const mode=process.argv[2]||'dev';
await import('./extract.mjs');
const args=[fileURLToPath(new URL('../node_modules/next/dist/bin/next',import.meta.url)),mode];
if(mode==='dev')args.push('--hostname','127.0.0.1','--port','3011');
const child=spawn(process.execPath,args,{stdio:'inherit',env:{...process.env,NEXT_TELEMETRY_DISABLED:'1'}});
child.on('exit',code=>{process.exitCode=code??1;});
