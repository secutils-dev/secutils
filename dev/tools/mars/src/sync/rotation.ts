// Durable, exact-byte key distribution. No remote mutation starts before the plan commits.
// A plan holds the fresh document keys and each wrapped copy, already encrypted and signed.
// `resumeRotation` replays the plan step by step and saves after each step.
// A reload or a lost response therefore repeats a step with identical bytes.
import type { WrappedKey } from './protocol.ts';
import { openJson } from './crypto.ts';
import { linkWrappingKey } from './live-link.ts';

/**
 * Key update of one document in a plan.
 * - `epoch`, `key`: the next key epoch and its document key in base64url.
 * - `values`: the key wrapped for each remaining member, each with the signature of the owner.
 * - `links`: the key sealed for each remaining Live link under the wrapping key of that link, each with the signature
 *   of the owner.
 * - `delivered`: the service stored the wrapped keys. `activated`: the service switched the epoch. `applied`: the local
 *   session uses the key.
 */
export interface RotationDocument {
    document: string;
    epoch: number;
    key: string;
    values: { wrapped: WrappedKey; signature: string }[];
    links: {
        invitation: string;
        value: { document: string; epoch: number; nonce: string; ciphertext: string };
        signature: string;
    }[];
    delivered?: boolean;
    activated?: boolean;
    applied?: boolean;
}
/**
 * A saved revocation with its key rotation. `target` is the member or the Live invitation that loses access. `fenced`
 * means that the service recorded the revocation. `revision` increments with each save. The plan holds document keys in
 * plaintext, so it is a secret.
 */
export interface RotationPlan {
    v: 1;
    id: string;
    revision: number;
    collection: string;
    member: string;
    target: { member: string } | { invitation: string };
    documents: RotationDocument[];
    fenced?: boolean;
}
/** Storage of one plan. `commit` replaces the plan at revision `expected`. A `null` value deletes it. */
export interface RotationRepository {
    get(): Promise<RotationPlan | null | undefined>;
    commit(expected: number | null, value: RotationPlan | null): Promise<void>;
}
/**
 * Checks that each saved Live-link key update still decrypts, under the wrapping key of its link, to the epoch and key
 * of its document. It runs before a remote step, and when a recovery file supplies the plan.
 * @throws `SyncError` `unsupported-link-key-format` for an unknown link or a link without a separate wrapping key. A
 *   failed decryption rejects with a Web Crypto error, and a mismatch with an `Error`.
 */
export async function validateRotationLinks(
    plan: RotationPlan,
    links: { id: string; document: string; secret: string; wrappingKey: string }[],
) {
    for (const document of plan.documents)
        for (const update of document.links) {
            const link = links.find((link) => link.id === update.invitation && link.document === document.document);
            const value = await openJson<{ epoch: number; key: string }>(update.value, linkWrappingKey(link), {
                invitation: update.invitation,
            });
            if (value.epoch !== document.epoch || value.key !== document.key)
                throw new Error('Invalid saved link key update');
        }
}
/**
 * Runs the saved plan to its end. Without a plan it does nothing. It saves the plan after each step:
 * 1. Revoke the target on the service.
 * 2. For each document: deliver the wrapped keys and the Live-link updates, activate the epoch, then call `apply`.
 * 3. Delete the plan.
 *
 * A failure rejects and keeps the plan. Call again to resume. The caller prevents concurrent runs of one plan.
 * @param request Sends one authenticated service action.
 * @param apply Installs the key of a document in the local session. A resumed run can call it again.
 * @param validate Receives the stored plan before the first remote step.
 */
export async function resumeRotation(
    repository: RotationRepository,
    request: (action: string, values: Record<string, unknown>) => Promise<unknown>,
    apply: (document: RotationDocument) => Promise<void>,
    validate?: (plan: RotationPlan) => Promise<void>,
) {
    let plan = await repository.get();
    if (!plan) return;
    await validate?.(plan);
    const save = async () => {
        const previous = plan!.revision;
        plan!.revision++;
        await repository.commit(previous, plan!);
    };
    if (!plan.fenced) {
        await request(
            'member' in plan.target ? 'revoke' : 'invite-revoke',
            'member' in plan.target ? { target: plan.target.member } : { invitation: plan.target.invitation },
        );
        plan.fenced = true;
        await save();
    }
    for (const document of plan.documents) {
        if (!document.delivered) {
            // 25 wrapped keys for each request keep the responder inside its KV operation budget.
            for (let offset = 0; offset < document.values.length; offset += 25)
                await request('key-write', { values: document.values.slice(offset, offset + 25) });
            for (const link of document.links) await request('live-key-write', link);
            document.delivered = true;
            await save();
        }
        if (!document.activated) {
            await request('rotate-document', { document: document.document, epoch: document.epoch });
            document.activated = true;
            await save();
        }
        if (!document.applied) {
            await apply(document);
            document.applied = true;
            await save();
        }
    }
    await repository.commit(plan.revision, null);
}
