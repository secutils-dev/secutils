import assert from 'node:assert/strict';
import { test } from 'node:test';
import { rebalance, unbalancedTags } from '../src/doc/preview-safety.ts';

test('the tag scan reports start tags without an end tag and end tags without a start tag', () => {
    assert.deepEqual(unbalancedTags('<sub>'), { open: ['sub'], orphans: [] });
    assert.deepEqual(unbalancedTags('</sub>'), { open: [], orphans: ['sub'] });
    assert.deepEqual(unbalancedTags('<b>bold</b> and <i>italic</i>'), { open: [], orphans: [] });
    assert.deepEqual(unbalancedTags('<details>\n<summary>More</summary>'), { open: ['details'], orphans: [] });
    assert.deepEqual(unbalancedTags('<DIV align="center"><Span title="a > b">'), {
        open: ['div', 'span'],
        orphans: [],
    });
    // Void elements and comments never stay open.
    assert.deepEqual(unbalancedTags('<br><img src="x"><hr><!-- <div> -->'), { open: [], orphans: [] });
    // An end tag closes the elements that are still open inside its element.
    assert.deepEqual(unbalancedTags('<div><span></div>'), { open: [], orphans: [] });
});

test('rebalancing keeps inline tags open across Markdown tokens', () => {
    // The sanitizer returns `<sub></sub>` for the token `<sub>` and nothing for `</sub>`.
    assert.equal(rebalance('<sub></sub>', ['sub'], []), '<sub>');
    assert.equal(rebalance('', [], ['sub']), '</sub>');
    assert.equal(
        rebalance('<details>\n<summary>More</summary></details>', ['details'], []),
        '<details>\n<summary>More</summary>',
    );
    assert.equal(
        rebalance('<div align="center"><span></span></div>', ['div', 'span'], []),
        '<div align="center"><span>',
    );
});

test('rebalancing never restores an element that the sanitizer removed', () => {
    // A forbidden start tag leaves no end tag to drop, so the output stays as sanitized.
    assert.equal(rebalance('', ['script'], []), '');
    assert.equal(rebalance('kept text', ['form'], []), 'kept text');
    // Only the outermost end tags that match the open list are dropped.
    assert.equal(rebalance('<b>x</b>', ['i'], []), '<b>x</b>');
});
