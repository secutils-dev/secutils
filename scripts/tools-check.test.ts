import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

function fixture(t: { after(fn: () => void): void }, missingSkill = false) {
    const root = mkdtempSync(join(tmpdir(), 'secutils-tools-check-'));
    t.after(() => rmSync(root, { recursive: true, force: true }));
    const file = (name: string, value: string) => {
        const path = join(root, name);
        mkdirSync(join(path, '..'), { recursive: true });
        writeFileSync(path, value);
        return path;
    };
    file('scripts/tools-check.ts', readFileSync(new URL('./tools-check.ts', import.meta.url), 'utf8'));
    file('dev/tools/shared/sources.ts', readFileSync(new URL('../dev/tools/shared/sources.ts', import.meta.url), 'utf8'));
    const html = (name: string, path: string, promote: boolean, host = '') =>
        `<meta name="su-tool-name" content="${name}"><meta name="su-tool-path" content="${path}">`
        + `<meta name="su-tool-promote" content="${promote}">`
        + (host ? `<meta name="su-tool-host" content="${host}">` : '');
    file('dev/tools/index.html', html('Index', '/', false));
    file('dev/tools/jwt/index.html', html('JWT Debugger', '/jwt', true));
    if (!missingSkill) file('dev/tools/jwt/SKILL.md', '# JWT Debugger');
    file('dev/tools/mars/index.html', html('MARS', '/', true, '{{MARS_HOST}}'));
    file('dev/tools/mars/SKILL.md', '# MARS');
    file('dev/tools/private/index.html', html('Private tool', '/private', false));
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('SECUTILS_')));
    return {
        run(promo?: string, extraEnv: Record<string, string> = {}) {
            return spawnSync(process.execPath, [join(root, 'scripts/tools-check.ts')], {
                encoding: 'utf8',
                env: { ...env, ...extraEnv,
                    ...(promo === undefined ? {} : { SECUTILS_TOOLS_PROMO_HOME_INDEX: file('promo.html', promo) }) },
            });
        },
    };
}

const promotedLinks = '<a href="https://{{TOOLS_HOST}}/jwt">JWT</a><a href="https://mars.secutils.dev/">MARS</a>';

test('promotion checks need no root README or marketing checkout', (t) => {
    const result = fixture(t).run();
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /OK \(4 tools, 2 promoted, 2 index-only\)/);
    assert.match(result.stderr, /skipping promo home page sync check/);
});

test('promotion checks accept linked tools and the index without a root README', (t) => {
    const result = fixture(t).run(promotedLinks + '<a href="https://{{TOOLS_HOST}}/">All tools</a>');
    assert.equal(result.status, 0, result.stderr);
});

test('own-host promotion checks honor the configured MARS host', (t) => {
    const result = fixture(t).run(promotedLinks.replace('mars.secutils.dev', 'mars.example.com'), {
        SECUTILS_MARS_PUBLIC_HOST: 'mars.example.com',
    });
    assert.equal(result.status, 0, result.stderr);
});

test('promotion checks still reject missing marketing links', (t) => {
    const result = fixture(t).run('<a href="https://mars.secutils.dev/">MARS</a>');
    assert.equal(result.status, 1);
    assert.match(result.stderr, /Promoted tool "JWT Debugger".*NOT linked from the promo home page/);
});

test('promotion checks still reject private tools on the marketing page', (t) => {
    const result = fixture(t).run(promotedLinks + '<a href="https://{{TOOLS_HOST}}/private">Private</a>');
    assert.equal(result.status, 1);
    assert.match(result.stderr, /Non-promoted tool "Private tool".*leaked into the promo home page/);
});

test('promotion checks still require companion skills', (t) => {
    const result = fixture(t, true).run();
    assert.equal(result.status, 1);
    assert.match(result.stderr, /Promoted tool "JWT Debugger".*missing its companion AI-agent skill/);
});
