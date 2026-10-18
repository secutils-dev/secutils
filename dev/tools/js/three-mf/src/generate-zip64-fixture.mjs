// Generate a redistributable fixture for exporters that always use ZIP64.
import { writeFileSync } from 'node:fs';
import { zipSync, strToU8 } from 'fflate';

const model = `<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02" xmlns:m="http://schemas.microsoft.com/3dmanufacturing/material/2015/02" unit="millimeter">
<resources><m:colorgroup id="2"><m:color color="#303B96FF"/><m:color color="#A42727FF"/></m:colorgroup>
<object id="1" pid="2" pindex="0"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="10" y="0" z="0"/><vertex x="10" y="0" z="10"/><vertex x="0" y="0" z="10"/></vertices>
<triangles><triangle v1="0" v2="1" v3="2"/><triangle v1="0" v2="2" v3="3" p1="1"/></triangles></mesh></object></resources><build><item objectid="1"/></build></model>`;
const zip = Buffer.from(zipSync({
    '_rels/.rels': strToU8('<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Target="/3D/model.model" Id="r" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/></Relationships>'),
    '3D/model.model': strToU8(model),
}, { level: 6 }));
const end = zip.length - 22, count = zip.readUInt16LE(end + 10);
let cursor = zip.readUInt32LE(end + 16), localSize = 0;
const locals = [], central = [];
for (let i = 0; i < count; i++) {
    const nameSize = zip.readUInt16LE(cursor + 28), name = zip.subarray(cursor + 46, cursor + 46 + nameSize);
    const compressed = zip.readUInt32LE(cursor + 20), expanded = zip.readUInt32LE(cursor + 24), offset = zip.readUInt32LE(cursor + 42);
    const dataOffset = offset + 30 + zip.readUInt16LE(offset + 26) + zip.readUInt16LE(offset + 28);
    const local = Buffer.from(zip.subarray(offset, offset + 30));
    local.writeUInt16LE(45, 4);
    local.writeUInt32LE(0xffffffff, 18); local.writeUInt32LE(0xffffffff, 22);
    local.writeUInt16LE(20, 28);
    const localExtra = Buffer.alloc(20);
    localExtra.writeUInt16LE(1); localExtra.writeUInt16LE(16, 2);
    localExtra.writeBigUInt64LE(BigInt(expanded), 4); localExtra.writeBigUInt64LE(BigInt(compressed), 12);
    const entry = Buffer.concat([local, name, localExtra, zip.subarray(dataOffset, dataOffset + compressed)]);
    locals.push(entry);
    const header = Buffer.from(zip.subarray(cursor, cursor + 46));
    header.writeUInt16LE(45, 4); header.writeUInt16LE(45, 6);
    header.writeUInt32LE(0xffffffff, 20); header.writeUInt32LE(0xffffffff, 24); header.writeUInt32LE(0xffffffff, 42);
    header.writeUInt16LE(28, 30);
    const extra = Buffer.alloc(28);
    extra.writeUInt16LE(1); extra.writeUInt16LE(24, 2);
    extra.writeBigUInt64LE(BigInt(expanded), 4); extra.writeBigUInt64LE(BigInt(compressed), 12); extra.writeBigUInt64LE(BigInt(localSize), 20);
    central.push(Buffer.concat([header, name, extra]));
    localSize += entry.length;
    cursor += 46 + nameSize + zip.readUInt16LE(cursor + 30) + zip.readUInt16LE(cursor + 32);
}
const directory = Buffer.concat(central), record = Buffer.alloc(56), locator = Buffer.alloc(20), eocd = Buffer.alloc(22);
record.writeUInt32LE(0x06064b50); record.writeBigUInt64LE(44n, 4); record.writeUInt16LE(45, 12); record.writeUInt16LE(45, 14);
record.writeBigUInt64LE(BigInt(count), 24); record.writeBigUInt64LE(BigInt(count), 32);
record.writeBigUInt64LE(BigInt(directory.length), 40); record.writeBigUInt64LE(BigInt(localSize), 48);
locator.writeUInt32LE(0x07064b50); locator.writeBigUInt64LE(BigInt(localSize + directory.length), 8); locator.writeUInt32LE(1, 16);
eocd.writeUInt32LE(0x06054b50); eocd.writeUInt16LE(count, 8); eocd.writeUInt16LE(count, 10);
eocd.writeUInt32LE(directory.length, 12); eocd.writeUInt32LE(0xffffffff, 16);
writeFileSync(new URL('../../../../../e2e/tools/fixtures/3mf/zip64-colors.3mf', import.meta.url), Buffer.concat([...locals, directory, record, locator, eocd]));
