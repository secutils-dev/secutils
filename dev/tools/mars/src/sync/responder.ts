// The server side of Sync and Live links. esbuild compiles this module to an IIFE, and
// `sync-assets.ts` splices it into the MARS webhook responder script, which runs once for each
// HTTP request in a small V8 sandbox. There is no other backend: no waits, no Rust route, no SQL.
//
// The sandbox shapes the design:
// - Storage is one key-value store with `get`, `set` (with `ifAbsent` and a time to live),
//   `delete`, and a prefix `list`. It has no transactions and no compare-and-swap. Records are
//   therefore immutable or append-only: a writer creates a key with `ifAbsent` and reads it back
//   to learn which writer won (`first`). No record is a mutable shared head.
// - One request can use 200 KV operations and a 10 MiB heap. Each action stays inside both
//   limits, and long work is paged across requests.
// - Web Crypto is not available. `@noble/curves` verifies P-256 signatures, and the host
//   supplies SHA-256.
//
// Trust: the service never holds a document key. It stores ciphertext and signed headers, checks
// signatures and grants, and enforces quotas, expiry, and revocation. Every request is untrusted
// input. Validate its shape and size before a write, and scope each key to the collection of
// the caller.
//
// Every response sets `skipRequest`, so request history never records user data. An error is a
// fixed code without request content.
//
// This file is the entry: it checks the request, selects the authorization for the action, and
// maps errors to responses. The actions live in `responder/`:
// - `invitations.ts`, `operator.ts`, `collections.ts`: actions with their own authorization.
// - `session.ts`: authentication of a member request.
// - `membership.ts`, `keys.ts`, `documents.ts`, `checkpoints.ts`, `renewal.ts`: member actions.
// - `store.ts`, `quota.ts`, `types.ts`: storage primitives, capacity accounting, and types.
import { fail, LIMITS, SyncError } from './protocol.ts';
import type { Context, Runtime } from './responder/types.ts';
import { type SyncResponse, json, signatureValid, allocate, first } from './responder/store.ts';
import { reserve, release } from './responder/quota.ts';
import { invitationAction } from './responder/invitations.ts';
import { operatorAction } from './responder/operator.ts';
import { createCollection } from './responder/collections.ts';
import { type MemberSession, authenticate } from './responder/session.ts';
import {
    bootstrap,
    listMembers,
    createInvitation,
    pairApproval,
    revokeInvitation,
    grantAccess,
    revokeMember,
} from './responder/membership.ts';
import { writeKeys, readKeys, readEpoch, rotateDocument, writeLiveKey } from './responder/keys.ts';
import { directory, deleteDocument, purgeLiveDocument, registerDocument, documentInfo } from './responder/documents.ts';
import { listCheckpoints, readCheckpoint, appendCheckpoint } from './responder/checkpoints.ts';
import { renewFolder, renewCheckpoint, renewDocument } from './responder/renewal.ts';

/** Actions that the bearer token of an invitation authorizes. */
const INVITATION_ACTIONS = new Set(['invite-read', 'pair-request', 'pair-result', 'live-join']);
/** Actions that the operator secret authorizes. */
const OPERATOR_ACTIONS = new Set(['telemetry', 'maintenance', 'storage-audit']);
/**
 * Member actions by name. A `Map` has no inherited keys, so a path such as `/sync/constructor`
 * finds no handler.
 */
const MEMBER_ACTIONS = new Map<string, (session: MemberSession) => Promise<SyncResponse>>([
    ['bootstrap', bootstrap],
    ['members', listMembers],
    ['key-write', writeKeys],
    ['key-read', readKeys],
    ['epoch', readEpoch],
    ['rotate-document', rotateDocument],
    ['live-key-write', writeLiveKey],
    ['invite', createInvitation],
    ['pair-pending', pairApproval],
    ['pair-approve', pairApproval],
    ['invite-revoke', revokeInvitation],
    ['directory', directory],
    ['delete-document', deleteDocument],
    ['purge-live-document', purgeLiveDocument],
    ['grant', grantAccess],
    ['revoke', revokeMember],
    ['document', registerDocument],
    ['document-info', documentInfo],
    ['index', listCheckpoints],
    ['get', readCheckpoint],
    ['append', appendCheckpoint],
    ['touch-folder', renewFolder],
    ['touch-record', renewCheckpoint],
    ['touch', renewDocument],
]);
/**
 * Handles one `/sync/<action>` request, and returns `null` for any other path.
 *
 * Actions fall into four groups with separate authorization:
 * 1. Invitation actions: the bearer token of the invitation (`invitationAction`).
 * 2. Operator actions: the operator secret of the responder (`operatorAction`).
 * 3. `create`: a self-signed owner grant, limited for each client address (`createCollection`).
 * 4. Member actions: the bearer token of a valid grant of the collection (`authenticate`).
 *
 * A failure is a `SyncError` code with its HTTP status. Any other error becomes a generic 503.
 */
export async function handle(context: Context, runtime: Runtime) {
    if (!context.path.startsWith('/sync/')) return null;
    try {
        const action = context.path.slice('/sync/'.length),
            method = context.method.toUpperCase();
        if (action === 'limits' && method === 'GET') return json(200, { v: 1, ...LIMITS });
        if (method !== 'POST') return json(405, { error: 'method-not-allowed' });
        if (context.body.length > LIMITS.updateBytes) return json(413, { error: 'update-too-large' });
        const request = JSON.parse(new TextDecoder().decode(context.body));
        if (!request || typeof request !== 'object' || Array.isArray(request)) return fail('invalid-request');
        if (INVITATION_ACTIONS.has(action)) return await invitationAction(context, runtime, request, action);
        if (OPERATOR_ACTIONS.has(action)) return await operatorAction(context, runtime, request, action);
        if (action === 'create') return await createCollection(context, runtime, request);
        const run = MEMBER_ACTIONS.get(action);
        const session = await authenticate(context, runtime, request, action);
        return run ? await run(session) : json(404, { error: 'unknown-sync-action' });
    } catch (error) {
        // Never expose runtime errors, request bodies, credentials, or cryptographic material.
        if (error instanceof SyncError) return json(error.status, { error: error.code });
        if (error instanceof SyntaxError) return json(400, { error: 'invalid-json' });
        return json(503, { error: 'sync-service-unavailable' });
    }
}
/** Internals for unit tests. */
export const responderInternals = { signatureValid, reserve, release, allocate, first };
export { prepareTestScope } from './testing.ts';
export type { Kv, Runtime, Context } from './responder/types.ts';
