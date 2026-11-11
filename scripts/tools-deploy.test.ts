import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { promisify } from 'node:util';
import { runInNewContext } from 'node:vm';
import { discoverTools } from '../dev/tools/shared/sources.ts';

const exec = promisify(execFile);
const aggregates = ['LLMS_TXT', 'ROBOTS_TXT', 'SITEMAP_XML', 'AGENT_SKILLS_INDEX'];
const sources = discoverTools();
const keys = [...sources.flatMap(({ slug, skill }) => [slug.replaceAll('-', '_').toUpperCase(),
    ...(skill ? [`${slug.replaceAll('-', '_').toUpperCase()}_MD`] : [])]), ...aggregates];
const ids = Object.fromEntries(keys.map((key) => {
    const hex = createHash('sha256').update(key).digest('hex');
    return [key, `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`];
}));

async function deploy(t: { after(fn: () => void): void }, selectors: string[] = []) {
    const root = mkdtempSync(join(tmpdir(), 'secutils-deploy-'));
    t.after(() => rmSync(root, { recursive: true, force: true }));
    const output = join(root, 'responders.jsonl');
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('SECUTILS_')));
    const result = await exec(process.execPath, ['--input-type=module', '-e', `
        import { appendFileSync } from 'node:fs';
        import { main } from './dev/tools/shared/deploy.ts';
        globalThis.fetch = async (url, init) => {
            if (!url.startsWith('https://deployment.invalid/api/webhooks/responders/')) throw new Error('Unexpected network request');
            appendFileSync(${JSON.stringify(output)}, JSON.stringify({ id: url.split('/').at(-1), ...JSON.parse(init.body) }) + '\\n');
            return new Response(null, { status: 204 });
        };
        process.argv = [process.execPath, 'deploy.ts', ...${JSON.stringify(selectors)}];
        await main();
    `], { env: { ...env, SECUTILS_HTML_APP_API_DOMAIN: 'https://deployment.invalid', SECUTILS_HTML_APP_API_KEY: 'synthetic-key',
        SECUTILS_MARS_STATS_VIEW_TOKEN: Buffer.alloc(32, 7).toString('base64url'),
        ...Object.fromEntries(keys.map((key) => [`SECUTILS_HTML_APP_RESPONDER_ID_${key}`, ids[key]])) }, maxBuffer: 1024 * 1024 });
    const records = readFileSync(output, 'utf8').trim().split('\n').map((line) => JSON.parse(line));
    return { ...result, records, byKey: (key: string) => records.find((record) => record.id === ids[key]).settings };
}

test('all tools assemble from the new layout without changing responder identities, public routes, or storage settings', async (t) => {
    const result = await deploy(t);
    assert.equal(result.records.length, keys.length);
    assert.match(result.stdout, /27 deployed/);
    for (const record of result.records) {
        assert.deepEqual(Object.keys(record), ['id', 'settings']);
        assert.equal(record.settings.requestsToTrack, 0);
        assert.equal(record.settings.statusCode, 200);
    }
    for (const source of sources) {
        const key = source.slug.replaceAll('-', '_').toUpperCase();
        const settings = result.byKey(key);
        assert.doesNotMatch(settings.body, /href="\.\.\/shared\/|src="\.\/assets\/|from ['"]\.\.\/shared\//);
        assert.doesNotMatch(settings.body, /\{\{(?:TOOLS_HOST|MARS_HOST)\}\}/);
        const mdPath = source.slug === 'index' || source.slug === 'mars' ? '/llms.txt'
            : settings.body.match(/name="su-tool-path" content="([^"]+)"/)![1] + '.md';
        const response = runInNewContext(settings.script, { context: { headers: { accept: 'text/markdown' } } });
        assert.equal(response.statusCode, 302);
        assert.equal(response.headers.Location, mdPath);
        if (source.skill) assert.match(result.byKey(`${key}_MD`).body, /^---\nname:/);
    }
    assert.match(result.byKey('MARS').script, /MARS_SYNC/);
    assert.ok(result.byKey('MARS').headers.some(([name, value]: string[]) => name === 'Cross-Origin-Opener-Policy' && value === 'same-origin'));
    assert.match(result.byKey('MARS_STATS').script, new RegExp(ids.MARS));
    assert.match(result.byKey('LLMS_TXT').body, /dev\/tools\/shared\/guides\/url-state\.md/);
    assert.doesNotMatch(result.byKey('LLMS_TXT').body, /mars-stats|mock-saml-idp|og-template/);
});

for (const selector of ['echo', 'echo.html', 'echo.skill.md', 'echo/index.html', 'echo/SKILL.md']) {
    test(`${selector} selects the same responder pair and discovery indexes`, async (t) => {
        const result = await deploy(t, [selector, selector]);
        assert.deepEqual(result.records.map((record) => record.id), [ids.ECHO, ids.ECHO_MD, ...aggregates.map((key) => ids[key])]);
    });
}
