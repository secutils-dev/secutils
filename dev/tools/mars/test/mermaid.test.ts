// Renderer repairs use copies of the live helpers, not a second implementation. Browser tests
// exercise Mermaid itself; these tests bound the repairs and preserve the original error contract.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';

const viewer = readFileSync(new URL('../src/doc/viewer.js', import.meta.url), 'utf8');
const source = viewer.slice(viewer.indexOf('function autoQuoteMermaidLabels('), viewer.indexOf('const _mermaidCache'));
const { escapeMermaidGanttTimes, sanitizeMermaid } = runInNewContext(
    `${source}\n({ escapeMermaidGanttTimes, sanitizeMermaid })`,
);

test('Gantt repairs only clock labels before an explicit task separator', () => {
    const lines = [
        'gantt',
        '  dateFormat HH:mm:ss',
        '  axisFormat %H:%M',
        '  title Incident 13:00 : timeline',
        '  section SLO 13:00 : bad slices',
        '  accTitle: Incident 13:00 : timeline',
        '  accDescr: Incident 13:00 : timeline',
        '  %% slice 13:00 : commented out',
        '  click s1 href "https://example.test/13:00 : details"',
        '  slice 13:00 BAD (p99 13,448)    :s1, 13:00:00, 10m',
        '  slice 14:10:52 BAD            :crit, s2, 14:10:52, 65s',
        '  slice 9:05 BAD\t :s3, 09:05:00, 10m',
        '  alert fires                   :milestone, 13:23:51, 0s',
        '  valid task :s4, 13:00:00, 10m %% note : unchanged',
        '  escaped 13#58;00 :s5, 13:00:00, 10m',
        '  ambiguous 13:00:s6, 13:00:00, 10m',
    ];
    const original = lines.join('\r\n');
    const expected = original
        .replace('slice 13:00 BAD', 'slice 13#58;00 BAD')
        .replace('slice 14:10:52 BAD', 'slice 14#58;10#58;52 BAD')
        .replace('slice 9:05 BAD', 'slice 9#58;05 BAD');
    assert.equal(escapeMermaidGanttTimes(original), expected);
    assert.equal(escapeMermaidGanttTimes(expected), expected);
});

test('Gantt repair runs even when Mermaid syntax parsing succeeds, but not for other diagrams', async () => {
    const original = 'gantt\n  slice 13:00 BAD (p99 13,448) :s1, 13:00:00, 10m';
    assert.equal(
        await sanitizeMermaid({ parse: async () => ({ diagramType: 'gantt' }) }, original),
        original.replace('slice 13:00', 'slice 13#58;00'),
    );
    const flowchart = 'flowchart LR\n A[Time 13:00 : unchanged] --> B[Done]';
    assert.equal(await sanitizeMermaid({ parse: async () => ({ diagramType: 'flowchart-v2' }) }, flowchart), flowchart);
});

test('flowchart label quoting remains a parse-error fallback and preserves the first error', async () => {
    const original = 'flowchart LR\n A[Start (here)] --> B[Done]';
    const firstError = new Error('original parse error');
    const parsed: string[] = [];
    const fixed = await sanitizeMermaid(
        {
            parse: async (text: string) => {
                parsed.push(text);
                if (text === original) throw firstError;
                return { diagramType: 'flowchart-v2' };
            },
        },
        original,
    );
    assert.equal(fixed, 'flowchart LR\n A["Start (here)"] --> B[Done]');
    assert.deepEqual(parsed, [original, fixed]);
    await assert.rejects(
        sanitizeMermaid(
            {
                parse: async (text: string) => {
                    throw text === original ? firstError : new Error('retry parse error');
                },
            },
            original,
        ),
        (error) => error === firstError,
    );
});
