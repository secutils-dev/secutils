import assert from 'node:assert/strict';
import test from 'node:test';
import { buttonClass } from '../src/ui/ui.ts';

test('button variants keep primary, secondary, empty, neutral, and flush choices explicit', () => {
    assert.equal(buttonClass(), 'btn');
    assert.equal(buttonClass({ primary: true }), 'btn btn-primary');
    assert.equal(buttonClass({ danger: true }), 'btn btn-danger');
    assert.equal(buttonClass({ empty: true }), 'btn btn-empty');
    assert.equal(buttonClass({ empty: true, text: true, flush: 'left' }), 'btn btn-empty btn-text btn-flush-left');
    assert.equal(buttonClass({ empty: true, flush: 'right' }), 'btn btn-empty btn-flush-right');
});
