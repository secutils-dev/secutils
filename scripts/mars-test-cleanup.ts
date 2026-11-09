// Retry one private test manifest; never clear the user or complete Sync namespace.
import { resolve } from 'node:path';
import { closeTestScope, scopeRequest } from '../e2e/mars-test-scopes.ts';

const [path, origin] = process.argv.slice(2);
if (!path || !origin) throw new Error('Usage: node --env-file=.env scripts/mars-test-cleanup.ts <manifest.json|--list|--expired> <expected-origin>');
if (path === '--list' || path === '--expired') {
  let after: string | null = null;
  for (let page = 0; page < 1000; page++) {
    const result = await scopeRequest(origin, { operation: 'list', after });
    for (const scope of result.scopes ?? []) {
      if (path === '--list') { console.log(JSON.stringify(scope)); continue; }
      if (scope.state === 'closed' || scope.state === 'active' && scope.expiresAt > Date.now() / 1000) continue;
      let closed = false;
      for (let batch = 0; batch < 2000; batch++) {
        const cleanup = await scopeRequest(origin, { operation: 'close', scope: scope.scope, run: scope.run, test: scope.test });
        if (cleanup.complete) { console.log(JSON.stringify({ scope: scope.scope, run: scope.run, test: scope.test, removed: cleanup.removed, state: 'closed' })); closed = true; break; }
      }
      if (!closed) throw new Error(`Cleanup did not finish for test scope ${scope.scope}. Retry --expired.`);
    }
    if (result.complete) break;
    if (!result.cursor || result.cursor === after) throw new Error('Test registry cursor did not advance.');
    after = result.cursor;
    if (page === 999) throw new Error('Test registry scan reached its page limit.');
  }
} else {
  const result = await closeTestScope(resolve(path), origin);
  console.log(JSON.stringify({ scope: result.scope, run: result.run, test: result.test, state: result.state, removed: result.removed }));
}
