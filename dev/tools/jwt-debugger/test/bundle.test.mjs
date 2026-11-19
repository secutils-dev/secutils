import assert from 'node:assert/strict';
import { createHmac } from 'node:crypto';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import { SignJWT, jwtVerify } from '../dist/jose.js';

const encoder = new TextEncoder();
const secret = encoder.encode('your-256-bit-secret');
const payload = { sub: '1234567890', name: 'John Doe', iat: 1516239022 };
const unsigned = 'eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ';
const sample = `${unsigned}.${createHmac('sha256', secret).update(unsigned).digest('base64url')}`;

test('HS256 matches an independent HMAC implementation', async () => {
    assert.equal(await new SignJWT(payload).setProtectedHeader({ alg: 'HS256' }).sign(secret), sample);
    assert.deepEqual((await jwtVerify(sample, secret, { algorithms: ['HS256'] })).payload, payload);
});

for (const alg of ['HS256', 'HS384', 'HS512']) {
    test(`${alg} round-trips and rejects the wrong secret`, async () => {
        const token = await new SignJWT(payload).setProtectedHeader({ alg }).sign(secret);
        assert.deepEqual((await jwtVerify(token, secret, { algorithms: [alg] })).payload, payload);
        await assert.rejects(jwtVerify(token, encoder.encode('wrong-secret'), { algorithms: [alg] }));
    });
}

test('the self-contained bundle includes its license and has no external imports', () => {
    const source = readFileSync(new URL('../dist/jose.js', import.meta.url), 'utf8');
    assert.match(source, /MIT License/);
    assert.doesNotMatch(source, /\bimport\s*(?:\(|['"]|.*?\bfrom\s*['"])/);
});
