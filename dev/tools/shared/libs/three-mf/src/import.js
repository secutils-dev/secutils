import { Unzip, UnzipInflate, strFromU8 } from 'fflate';
import { SaxesParser } from 'saxes';

const CORE = 'http://schemas.microsoft.com/3dmanufacturing/core/2015/02';
const MATERIAL = 'http://schemas.microsoft.com/3dmanufacturing/material/2015/02';
const RELS = 'http://schemas.openxmlformats.org/package/2006/relationships';
const START = 'http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel';
const MAX_BYTES = 128 * 1024 * 1024;
const MAX_TRIANGLES = 1000000;
const MAX_VERTICES = 3000000;
const IDENTITY = [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0];
const UNITS = { micron: 0.001, millimeter: 1, centimeter: 10, inch: 25.4, foot: 304.8, meter: 1000 };
const fail = (message) => { throw new Error(message); };

function xml(text) {
    const parser = new SaxesParser({ xmlns: true });
    const stack = [];
    let root, nodes = 0;
    parser.on('doctype', () => fail('3MF XML cannot contain a DTD.'));
    parser.on('error', (error) => { throw error; });
    parser.on('opentag', (tag) => {
        if (++nodes > 6000000 || stack.length > 64) fail('3MF XML exceeds the node or depth limit.');
        const node = { name: tag.local, ns: tag.uri, attrs: {}, namespaces: { ...stack.at(-1)?.namespaces, ...tag.ns }, children: [] };
        for (const attr of Object.values(tag.attributes)) if (!attr.uri) node.attrs[attr.local] = attr.value;
        // Namespaced attributes are kept separately from Core attributes.
        node.extra = Object.values(tag.attributes).filter((attr) => attr.uri && attr.uri !== 'http://www.w3.org/2000/xmlns/');
        if (stack.length) stack.at(-1).children.push(node);
        else if (root) fail('Expected one XML root.');
        else root = node;
        stack.push(node);
    });
    parser.on('closetag', () => stack.pop());
    parser.write(text).close();
    if (!root) fail('Empty 3MF XML part.');
    return root;
}
const children = (node, name, ns = CORE) => node?.children.filter((child) => child.name === name && child.ns === ns) ?? [];
const child = (node, name, ns = CORE) => children(node, name, ns)[0];
function number(value, label) {
    if (value === undefined || !/^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?$/.test(value.trim())) fail(`Invalid ${label}.`);
    const n = Number(value);
    if (!Number.isFinite(n) || !Number.isFinite(Math.fround(n))) fail(`Non-finite ${label}.`);
    return n;
}
function index(value, label) {
    if (value === undefined || !/^\d+$/.test(value.trim())) fail(`Invalid ${label}.`);
    const n = Number(value);
    if (!Number.isSafeInteger(n) || n > 0x7fffffff) fail(`Invalid ${label}.`);
    return n;
}
function transform(value) {
    if (value === undefined) return IDENTITY;
    const values = value.trim().split(/\s+/).map((n) => number(n, 'transform'));
    if (values.length !== 12) fail('A 3MF transform needs 12 numbers.');
    return values;
}
function apply(p, m) {
    return [p[0] * m[0] + p[1] * m[3] + p[2] * m[6] + m[9], p[0] * m[1] + p[1] * m[4] + p[2] * m[7] + m[10], p[0] * m[2] + p[1] * m[5] + p[2] * m[8] + m[11]];
}
function color(value) {
    if (!/^#[\da-f]{6}(?:[\da-f]{2})?$/i.test(value ?? '')) fail('Invalid 3MF display color.');
    // A standalone surface color is opaque, including #RRGGBBAA values.
    return [1, 3, 5].map((offset) => parseInt(value.slice(offset, offset + 2), 16) / 255);
}
function partPath(value) {
    if (!value || /[\\?#]/.test(value) || /^[a-z][\w+.-]*:/i.test(value) || value.startsWith('//')) fail('External or invalid 3MF part path.');
    const parts = decodeURIComponent(value).replace(/^\//, '').split('/');
    if (parts.some((part) => part === '..' || part === '.' || !part)) fail('Invalid 3MF part path.');
    return parts.join('/');
}
function unzip(bytes) {
    if (bytes.length > MAX_BYTES) fail('3MF archive exceeds 128 MiB.');
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const uint64 = (offset, limit = bytes.length) => {
        if (offset < 0 || offset + 8 > limit) fail('Truncated 3MF ZIP64 value.');
        const value = view.getBigUint64(offset, true);
        if (value > BigInt(Number.MAX_SAFE_INTEGER)) fail('3MF ZIP64 value exceeds the supported range.');
        return Number(value);
    };
    let end = -1;
    for (let i = bytes.length - 22; i >= Math.max(0, bytes.length - 65557); i--) {
        if (view.getUint32(i, true) === 0x06054b50 && i + 22 + view.getUint16(i + 20, true) === bytes.length) { end = i; break; }
    }
    if (end < 0) fail('Invalid or truncated 3MF ZIP archive.');
    if (view.getUint16(end + 4, true) || view.getUint16(end + 6, true)) fail('Multi-disk 3MF ZIP archives are unsupported.');
    let entriesCount = view.getUint16(end + 10, true);
    let directorySize = view.getUint32(end + 12, true), directoryOffset = view.getUint32(end + 16, true);
    let directoryEnd = end;
    const locator = end - 20;
    if (locator >= 0 && view.getUint32(locator, true) === 0x07064b50) {
        if (view.getUint32(locator + 4, true) || view.getUint32(locator + 16, true) !== 1) fail('Multi-disk 3MF ZIP64 archives are unsupported.');
        const record = uint64(locator + 8);
        if (record > locator - 56 || view.getUint32(record, true) !== 0x06064b50) fail('Invalid 3MF ZIP64 directory record.');
        const recordSize = uint64(record + 4);
        if (recordSize < 44 || recordSize !== locator - record - 12) fail('Invalid 3MF ZIP64 directory record size.');
        if (view.getUint32(record + 16, true) || view.getUint32(record + 20, true)) fail('Multi-disk 3MF ZIP64 archives are unsupported.');
        const count64 = uint64(record + 32), size64 = uint64(record + 40), offset64 = uint64(record + 48);
        if (uint64(record + 24) !== count64
            || (entriesCount !== 65535 && entriesCount !== count64)
            || (directorySize !== 0xffffffff && directorySize !== size64)
            || (directoryOffset !== 0xffffffff && directoryOffset !== offset64)) fail('Inconsistent 3MF ZIP64 directory.');
        entriesCount = count64; directorySize = size64; directoryOffset = offset64; directoryEnd = record;
    } else if (entriesCount === 65535 || directoryOffset === 0xffffffff || directorySize === 0xffffffff) {
        fail('Missing 3MF ZIP64 directory locator.');
    }
    if (entriesCount > 4096 || directoryOffset > directoryEnd || directorySize > directoryEnd - directoryOffset) fail('Invalid or oversized 3MF ZIP directory.');
    directoryEnd = directoryOffset + directorySize;
    let declaredSize = 0, cursor = directoryOffset;
    for (let i = 0; i < entriesCount; i++) {
        if (cursor + 46 > directoryEnd || view.getUint32(cursor, true) !== 0x02014b50) fail('Invalid 3MF ZIP directory entry.');
        const flags = view.getUint16(cursor + 8, true), method = view.getUint16(cursor + 10, true);
        if ((flags & 1) || ![0, 8].includes(method)) fail('Encrypted or unsupported 3MF ZIP compression.');
        const extraStart = cursor + 46 + view.getUint16(cursor + 28, true);
        const extraEnd = extraStart + view.getUint16(cursor + 30, true);
        const next = extraEnd + view.getUint16(cursor + 32, true);
        if (next > directoryEnd) fail('Truncated 3MF ZIP directory entry.');
        let expandedSize = view.getUint32(cursor + 24, true), compressedSize = view.getUint32(cursor + 20, true);
        let localOffset = view.getUint32(cursor + 42, true), disk = view.getUint16(cursor + 34, true);
        const needs64 = expandedSize === 0xffffffff || compressedSize === 0xffffffff || localOffset === 0xffffffff || disk === 65535;
        let found64 = false;
        for (let extra = extraStart; extra < extraEnd;) {
            if (extra + 4 > extraEnd) fail('Truncated 3MF ZIP extra field.');
            const fieldEnd = extra + 4 + view.getUint16(extra + 2, true);
            if (fieldEnd > extraEnd) fail('Truncated 3MF ZIP extra field.');
            if (view.getUint16(extra, true) === 1) {
                if (found64) fail('Duplicate 3MF ZIP64 extra field.');
                found64 = true;
                let position = extra + 4;
                if (expandedSize === 0xffffffff) { expandedSize = uint64(position, fieldEnd); position += 8; }
                if (compressedSize === 0xffffffff) { compressedSize = uint64(position, fieldEnd); position += 8; }
                if (localOffset === 0xffffffff) { localOffset = uint64(position, fieldEnd); position += 8; }
                if (disk === 65535) {
                    if (position + 4 > fieldEnd) fail('Truncated 3MF ZIP64 disk value.');
                    disk = view.getUint32(position, true);
                }
            }
            extra = fieldEnd;
        }
        if (needs64 && !found64) fail('Missing 3MF ZIP64 entry sizes.');
        if (disk) fail('Multi-disk 3MF ZIP archives are unsupported.');
        if (localOffset > directoryOffset - 30 || compressedSize > directoryOffset - localOffset - 30
            || view.getUint32(localOffset, true) !== 0x04034b50) fail('Invalid 3MF ZIP local entry.');
        declaredSize += expandedSize;
        if (declaredSize > MAX_BYTES) fail('Expanded 3MF archive exceeds 128 MiB.');
        cursor = next;
    }
    if (cursor !== directoryOffset + directorySize) fail('Invalid 3MF ZIP directory size.');
    const files = new Map();
    let expanded = 0, entries = 0;
    const archive = new Unzip((file) => {
        if (++entries > 4096) fail('3MF archive has too many entries.');
        if (file.originalSize > MAX_BYTES) fail('Expanded 3MF archive exceeds 128 MiB.');
        if (file.name.endsWith('/')) return;
        const name = partPath(file.name);
        if (files.has(name)) fail('Duplicate 3MF package part.');
        files.set(name, null);
        const chunks = [];
        let size = 0;
        file.ondata = (error, data, final) => {
            if (error) throw error;
            size += data.length; expanded += data.length;
            if (expanded > MAX_BYTES) fail('Expanded 3MF archive exceeds 128 MiB.');
            chunks.push(data);
            if (final) {
                const result = new Uint8Array(size);
                let offset = 0;
                for (const chunk of chunks) { result.set(chunk, offset); offset += chunk.length; }
                files.set(name, result);
            }
        };
        file.start();
    });
    archive.register(UnzipInflate);
    // Bound each inflater output allocation, even for forged ZIP size headers.
    for (let i = 0; i < bytes.length; i += 4096) archive.push(bytes.subarray(i, i + 4096), i + 4096 >= bytes.length);
    if (!files.size || entries !== entriesCount || [...files.values()].some((file) => !file)) fail('Invalid or truncated 3MF archive.');
    return files;
}
function base64(array) {
    const bytes = new Uint8Array(array.buffer, array.byteOffset, array.byteLength);
    const parts = [];
    for (let i = 0; i < bytes.length; i += 24576) parts.push(btoa(String.fromCharCode(...bytes.subarray(i, i + 24576))));
    return parts.join('');
}

export function import3mf(bytes) {
    const files = unzip(bytes);
    const read = (name) => {
        if (!files.get(name)) fail(`Missing 3MF package part: ${name}`);
        return xml(strFromU8(files.get(name)));
    };
    let modelPath;
    for (const [name] of files) if (name.endsWith('.rels')) {
        const rels = read(name);
        if (rels.name !== 'Relationships' || rels.ns !== RELS) fail('Invalid package relationships.');
        for (const rel of children(rels, 'Relationship', RELS)) {
            if (rel.attrs.TargetMode === 'External') fail('External 3MF relationships are unsupported.');
            if (name === '_rels/.rels' && rel.attrs.Type === START) {
                if (modelPath) fail('Multiple 3MF start parts.');
                modelPath = partPath(rel.attrs.Target);
            }
        }
    }
    if (!modelPath) fail('Missing 3MF start-part relationship.');
    const model = read(modelPath);
    if (model.name !== 'model' || model.ns !== CORE) fail('Expected a 3MF Core model.');
    for (const prefix of (model.attrs.requiredextensions ?? '').trim().split(/\s+/).filter(Boolean)) {
        if (model.namespaces[prefix] !== MATERIAL) fail(`Unsupported required 3MF extension: ${prefix}`);
    }
    const unit = model.attrs.unit ?? 'millimeter', unitScale = UNITS[unit];
    if (!unitScale) fail('Unsupported 3MF unit.');
    const resources = new Map(), warnings = new Set();
    let vertexCount = 0, triangleCount = 0;
    for (const resource of child(model, 'resources')?.children ?? []) {
        if (resource.ns !== CORE && resource.ns !== MATERIAL && resource.attrs.id === undefined) continue;
        const id = index(resource.attrs.id, 'resource ID');
        if (resources.has(id)) fail('Duplicate 3MF resource ID.');
        let result = { kind: 'unsupported' };
        if (resource.ns === CORE && resource.name === 'basematerials') {
            result = { kind: 'base', colors: children(resource, 'base').map((base) => color(base.attrs.displaycolor)) };
        } else if (resource.ns === MATERIAL && resource.name === 'colorgroup') {
            result = { kind: 'color', colors: children(resource, 'color', MATERIAL).map((entry) => color(entry.attrs.color)) };
        } else if (resource.ns === CORE && resource.name === 'object') {
            const mesh = child(resource, 'mesh'), components = child(resource, 'components');
            if (!!mesh === !!components) fail('A 3MF object needs a mesh or components.');
            if (resource.extra.some((attr) => attr.local === 'path')) fail('Cross-part 3MF objects are unsupported.');
            result = { kind: 'object', name: resource.attrs.name ?? '', defaults: resource.attrs, components: null };
            if (mesh) {
                const vertices = children(child(mesh, 'vertices'), 'vertex');
                const triangles = children(child(mesh, 'triangles'), 'triangle');
                vertexCount += vertices.length; triangleCount += triangles.length;
                if (vertexCount > MAX_VERTICES || triangleCount > MAX_TRIANGLES) fail('3MF geometry exceeds the vertex or triangle limit.');
                result.vertices = new Float32Array(vertices.length * 3);
                vertices.forEach((vertex, i) => ['x', 'y', 'z'].forEach((axis, j) => { result.vertices[i * 3 + j] = number(vertex.attrs[axis], 'vertex'); }));
                result.indices = new Uint32Array(triangles.length * 3);
                result.properties = triangles.map((triangle, i) => {
                    if (triangle.extra.some((attr) => /paint|segmentation|fuzzy_skin/i.test(attr.local))) warnings.add('Slicer-specific paint metadata is unsupported; only portable 3MF colors are displayed.');
                    for (const [j, key] of ['v1', 'v2', 'v3'].entries()) {
                        const v = index(triangle.attrs[key], 'triangle vertex index');
                        if (v >= vertices.length) fail('3MF triangle vertex index is out of range.');
                        result.indices[i * 3 + j] = v;
                    }
                    return triangle.attrs;
                });
            } else result.components = children(components, 'component').map((component) => {
                if (component.extra.some((attr) => attr.local === 'path')) fail('Cross-part 3MF components are unsupported.');
                return { id: index(component.attrs.objectid, 'component object ID'), transform: transform(component.attrs.transform) };
            });
        } else if ((model.attrs.requiredextensions ?? '').split(/\s+/).some((prefix) => model.namespaces[prefix] === resource.ns)) {
            fail(`Unsupported feature in required 3MF extension: ${resource.name}`);
        }
        resources.set(id, result);
    }
    const instances = [];
    let count = 0, visits = 0;
    const visit = (id, transforms, path, active) => {
        if (++visits > 100000 || active.size > 64) fail('3MF component instances exceed the limit.');
        if (active.has(id)) fail('Cyclic 3MF component reference.');
        const object = resources.get(id);
        if (object?.kind !== 'object') fail('Missing 3MF object resource.');
        if (object.components) {
            const next = new Set(active); next.add(id);
            object.components.forEach((component, i) => visit(component.id, [component.transform, ...transforms], `${path}/${i}:${component.id}`, next));
        } else {
            const length = object.indices.length / 3;
            if (count + length > MAX_TRIANGLES) fail('Instanced 3MF geometry exceeds 1,000,000 triangles.');
            instances.push({ objectId: id, name: object.name, path, start: count, count: length, transforms });
            count += length;
        }
    };
    children(child(model, 'build'), 'item').forEach((item, i) => visit(index(item.attrs.objectid, 'build object ID'), [transform(item.attrs.transform)], `${i}:${item.attrs.objectid}`, new Set()));
    if (!count) fail('The 3MF build has no triangles.');
    const positions = new Float32Array(count * 9), colors = new Float32Array(count * 9);
    const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
    for (const instance of instances) {
        const object = resources.get(instance.objectId);
        for (let i = 0; i < instance.count; i++) {
            const props = object.properties[i], defaults = object.defaults;
            const pid = props.pid ?? defaults.pid;
            const group = pid === undefined ? null : resources.get(index(pid, 'property resource ID'));
            if (pid !== undefined && !group) fail('Missing 3MF property resource.');
            const p1 = props.p1 ?? defaults.pindex;
            if (group && ['base', 'color'].includes(group.kind) && p1 === undefined) fail('Missing 3MF property index.');
            if (group?.kind === 'unsupported') warnings.add('Unsupported optional appearance uses the default blue material.');
            if (group?.kind === 'object') fail('Invalid 3MF property resource.');
            const propertyIndices = [p1, props.p2 ?? p1, props.p3 ?? p1];
            for (let v = 0; v < 3; v++) {
                const offset = (instance.start + i) * 9 + v * 3;
                const vi = object.indices[i * 3 + v] * 3;
                let p = Array.from(object.vertices.subarray(vi, vi + 3));
                for (const matrix of instance.transforms) p = apply(p, matrix);
                // 3MF is Z-up. The viewer is Y-up. Units become millimeters.
                p = [p[0] * unitScale, p[2] * unitScale, -p[1] * unitScale];
                let c = [0.26, 0.6, 0.86];
                if (group?.colors) {
                    const pi = index(group.kind === 'base' ? p1 : propertyIndices[v], 'color index');
                    c = group.colors[pi];
                    if (!c) fail('3MF color index is out of range.');
                    if (group.kind === 'base' && propertyIndices.some((value) => value !== p1)) fail('Base materials cannot form vertex gradients.');
                }
                for (let axis = 0; axis < 3; axis++) {
                    if (!Number.isFinite(Math.fround(p[axis]))) fail('Non-finite transformed 3MF vertex.');
                    positions[offset + axis] = p[axis]; colors[offset + axis] = c[axis];
                    min[axis] = Math.min(min[axis], positions[offset + axis]); max[axis] = Math.max(max[axis], positions[offset + axis]);
                }
            }
        }
    }
    const mesh = { version: 1, positions: base64(positions), colors: base64(colors), count, min, max, unit, coordinateUnit: 'millimeter', instances, warnings: [...warnings] };
    return { mesh, positions, colors, source: JSON.stringify(mesh) };
}

export function parseMesh(source) {
    if (source.length > MAX_BYTES) fail('Mesh source exceeds 128 MiB.');
    const mesh = JSON.parse(source);
    if (mesh.version !== 1) fail('Unsupported su-mesh version.');
    if (!Number.isInteger(mesh.count) || mesh.count < 1 || mesh.count > MAX_TRIANGLES) fail('Invalid su-mesh triangle count.');
    const decode = (value) => {
        if (typeof value !== 'string' || value.length !== mesh.count * 48 || !/^[A-Za-z0-9+/]*$/.test(value)) fail('Invalid su-mesh buffer.');
        const binary = atob(value), bytes = new Uint8Array(binary.length);
        for (let i = 0; i < bytes.length; i++) bytes[i] = binary.charCodeAt(i);
        const array = new Float32Array(bytes.buffer);
        if (array.some((n) => !Number.isFinite(n))) fail('Non-finite su-mesh value.');
        return array;
    };
    const positions = decode(mesh.positions), colors = decode(mesh.colors);
    if (colors.some((n) => n < 0 || n > 1)) fail('Invalid su-mesh color.');
    const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
    for (let i = 0; i < positions.length; i++) { min[i % 3] = Math.min(min[i % 3], positions[i]); max[i % 3] = Math.max(max[i % 3], positions[i]); }
    mesh.min = min; mesh.max = max;
    if (!Array.isArray(mesh.instances) || mesh.instances.length > 100000 || !Array.isArray(mesh.warnings) || mesh.warnings.some((warning) => typeof warning !== 'string')) fail('Invalid su-mesh metadata.');
    return { mesh, positions, colors, source };
}
