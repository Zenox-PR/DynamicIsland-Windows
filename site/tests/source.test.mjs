import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
const entries=JSON.parse(await readFile(new URL('../generated/source.json',import.meta.url),'utf8'));
test('all excerpts exactly match real source lines and hashes',async()=>{
 assert.equal(entries.length,10);assert.equal(new Set(entries.map(e=>e.id)).size,10);
 for(const e of entries){assert.match(e.file,/^windows\/crates\/island-(core|ui|windows)\/src\/[a-z_]+\.rs$/);const raw=(await readFile(new URL('../../'+e.file,import.meta.url),'utf8')).replace(/\r\n/g,'\n');assert.equal(createHash('sha256').update(raw).digest('hex'),e.sha256);assert.equal(raw.split('\n').slice(e.line-1,e.line-1+e.code.split('\n').length).join('\n').trimEnd(),e.code);}
});

