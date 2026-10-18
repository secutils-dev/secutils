import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { zipSync, strToU8 } from 'fflate';
import { import3mf, parseMesh } from './import.js';

const core = 'http://schemas.microsoft.com/3dmanufacturing/core/2015/02';
const material = 'http://schemas.microsoft.com/3dmanufacturing/material/2015/02';
const geometry = '<mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh>';
const root = (resources, build = '<item objectid="1"/>', attrs = '') => `<model xmlns="${core}" xmlns:m="${material}" ${attrs}><resources>${resources}</resources><build>${build}</build></model>`;
const pack = (model, rels = '') => zipSync({
    '_rels/.rels': strToU8(`<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Target="/custom/model.xml" Id="r" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel" ${rels}/></Relationships>`),
    'custom/model.xml': strToU8(model),
});
const object = `<object id="1">${geometry}</object>`;

test('published exporter fixtures: uncolored, vertex colors, face colors, and component instances', () => {
    for (const [name, triangles, instances] of [['box', 12, 1], ['vertex-colors', 4, 1], ['face-colors', 44, 1], ['multiple-cylinders', 528, 6], ['prusaslicer', 72, 2]]) {
        const bytes = readFileSync(new URL(`../../../../../e2e/tools/fixtures/3mf/${name}.3mf`, import.meta.url));
        const result = import3mf(bytes);
        assert.equal(result.mesh.count, triangles);
        assert.equal(result.mesh.instances.length, instances);
        assert.deepEqual(parseMesh(result.source).positions, result.positions);
    }
});

test('object defaults, triangle overrides, and independent sRGB vertex assignments', () => {
    const group = '<m:colorgroup id="2"><m:color color="#FF0000"/><m:color color="#00FF00"/><m:color color="#0000FF"/></m:colorgroup>';
    const shape = `<object id="1" pid="2" pindex="1">${geometry.replace('/></triangles>', '/><triangle v1="0" v2="1" v3="2" p1="0" p2="1" p3="2"/></triangles>')}</object>`;
    const result = import3mf(pack(root(group + shape)));
    assert.deepEqual([...result.colors], [0, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1]);
});

test('Core base materials preserve defaults and per-triangle overrides', () => {
    const group = '<basematerials id="2"><base name="red" displaycolor="#FF0000"/><base name="blue" displaycolor="#0000FF"/></basematerials>';
    const shape = `<object id="1" pid="2" pindex="0">${geometry.replace('/></triangles>', '/><triangle v1="0" v2="1" v3="2" pid="2" p1="1"/></triangles>')}</object>`;
    const result = import3mf(pack(root(group + shape)));
    assert.deepEqual([...result.colors], [1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1]);
});

test('nested noncommuting transforms, units, Z-up conversion, and repeated instances', () => {
    const component = '<object id="2"><components><component objectid="1" transform="1 0 0 0 1 0 0 0 1 2 0 0"/></components></object>';
    const parent = '<object id="3"><components><component objectid="2" transform="0 1 0 -1 0 0 0 0 1 0 0 0"/></components></object>';
    const result = import3mf(pack(root(object + component + parent, '<item objectid="3" transform="1 0 0 0 1 0 0 0 1 10 0 0"/><item objectid="1"/>', 'unit="inch"')));
    assert.equal(result.mesh.instances.length, 2);
    assert.deepEqual([...result.positions.slice(0, 3)], [254, 0, Math.fround(-50.8)]);
    assert.equal(result.mesh.unit, 'inch');
    assert.deepEqual(result.mesh.min, [0, 0, Math.fround(-76.2)]);
});

test('Core prefixes are independent of literal namespace names', () => {
    const text = root(object).replace(/<(\/?)(model|resources|build|item|object|mesh|vertices|vertex|triangles|triangle)(?=[\s>\/])/g, '<$1c:$2').replace(`xmlns="${core}"`, `xmlns:c="${core}"`);
    assert.equal(import3mf(pack(text)).mesh.count, 1);
});

test('invalid input fails before geometry reaches the renderer', () => {
    const cases = [
        [pack(root(object), 'TargetMode="External"'), /External/],
        [pack(root(object, undefined, 'xmlns:p="urn:production" requiredextensions="p"')), /required.*extension/],
        [pack(root(object.replace('v3="2"', 'v3="9"'))), /index is out of range/],
        [pack(root(object.replace('x="1"', 'x="NaN"'))), /vertex/],
        [pack(root('<object id="1"><components><component objectid="1"/></components></object>')), /Cyclic/],
        [pack(root(object, '<item objectid="99"/>')), /Missing.*object/],
        [pack('<!DOCTYPE model [<!ENTITY x "boom">]>' + root(object)), /DTD/],
        [pack(root(object).replace('</model>', '')), /unclosed/],
        [new Uint8Array([1, 2, 3]), /archive|zip/i],
        [pack(root(object)).subarray(0, -22), /truncated/],
    ];
    for (const [bytes, message] of cases) assert.throws(() => import3mf(bytes), message);
});

test('declared archive expansion and geometry instance limits fail before large allocations', () => {
    const bytes = pack(root(object));
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const end = bytes.length - 22, directory = view.getUint32(end + 16, true);
    view.setUint32(directory + 24, 129 * 1024 * 1024, true);
    assert.throws(() => import3mf(bytes), /Expanded/);
    const objects = [object];
    for (let i = 2; i <= 22; i++) objects.push(`<object id="${i}"><components><component objectid="${i - 1}"/><component objectid="${i - 1}"/></components></object>`);
    assert.throws(() => import3mf(pack(root(objects.join(''), '<item objectid="22"/>'))), /limit/);
});

test('ZIP64 directory and entry extra fields preserve standard blue and red colors', () => {
    const bytes = readFileSync(new URL('../../../../../e2e/tools/fixtures/3mf/zip64-colors.3mf', import.meta.url));
    const result = import3mf(bytes);
    assert.equal(result.mesh.count, 2);
    assert.deepEqual(result.mesh.warnings, []);
    assert.deepEqual([...result.colors.slice(0, 3)].map((n) => Math.round(n * 255)), [48, 59, 150]);
    assert.deepEqual([...result.colors.slice(9, 12)].map((n) => Math.round(n * 255)), [164, 39, 39]);
    const end = bytes.length - 22, locator = end - 20;
    const record = Number(bytes.readBigUInt64LE(locator + 8));
    const directory = Number(bytes.readBigUInt64LE(record + 48));
    const extra = directory + 46 + bytes.readUInt16LE(directory + 28);
    for (const [modify, error] of [
        [(b) => b.writeUInt32LE(0, locator), /Missing.*locator/],
        [(b) => b.writeBigUInt64LE(BigInt(bytes.length), locator + 8), /directory record/],
        [(b) => b.writeBigUInt64LE(0xffffffffffffffffn, record + 48), /supported range/],
        [(b) => b.writeUInt32LE(2, locator + 16), /Multi-disk/],
        [(b) => b.writeBigUInt64LE(4097n, record + 24) && b.writeBigUInt64LE(4097n, record + 32) && b.writeUInt16LE(65535, end + 10), /oversized/],
        [(b) => b.writeBigUInt64LE(129n * 1024n * 1024n, extra + 4), /Expanded/],
        [(b) => b.writeUInt16LE(9, extra), /Missing.*entry/],
        [(b) => b.writeUInt16LE(8, extra + 2), /Truncated/],
    ]) {
        const malformed = Buffer.from(bytes);
        modify(malformed);
        assert.throws(() => import3mf(malformed), error);
    }
});

test('optional unsupported appearance uses a documented default', () => {
    const group = '<m:texture2dgroup id="2" texid="3"/>';
    const result = import3mf(pack(root(group + object.replace('id="1"', 'id="1" pid="2" pindex="0"'))));
    assert.match(result.mesh.warnings[0], /default blue/);
});

test('su-mesh rejects unknown versions, wrong lengths, non-finite geometry, and invalid colors', () => {
    const { mesh } = import3mf(pack(root(object)));
    assert.throws(() => parseMesh(JSON.stringify({ ...mesh, version: 2 })), /version/);
    assert.throws(() => parseMesh(JSON.stringify({ ...mesh, positions: '' })), /buffer/);
    const invalid = new Float32Array(9); invalid[0] = NaN;
    assert.throws(() => parseMesh(JSON.stringify({ ...mesh, positions: Buffer.from(invalid.buffer).toString('base64') })), /Non-finite/);
    invalid[0] = 2;
    assert.throws(() => parseMesh(JSON.stringify({ ...mesh, colors: Buffer.from(invalid.buffer).toString('base64') })), /color/);
});
