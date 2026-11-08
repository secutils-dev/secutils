// Durable, exact-byte key distribution. No remote mutation starts before the plan commits.
import type { WrappedKey } from './protocol.ts';

export interface RotationDocument {
    document: string; epoch: number; key: string;
    values: { wrapped: WrappedKey; signature: string }[];
    links: { invitation: string; value: { document: string; epoch: number; nonce: string; ciphertext: string }; signature: string }[];
    delivered?: boolean; activated?: boolean; applied?: boolean;
}
export interface RotationPlan {
    v: 1; id: string; revision: number; collection: string; member: string;
    target: { member: string } | { invitation: string };
    documents: RotationDocument[]; fenced?: boolean;
}
export interface RotationRepository {
    get(): Promise<RotationPlan | null | undefined>;
    commit(expected: number | null, value: RotationPlan | null): Promise<void>;
}
export async function resumeRotation(repository: RotationRepository,
    request: (action: string, values: Record<string, unknown>) => Promise<unknown>,
    apply: (document: RotationDocument) => Promise<void>) {
    let plan = await repository.get(); if (!plan) return;
    const save = async () => { const previous = plan!.revision; plan!.revision++; await repository.commit(previous, plan!); };
    if (!plan.fenced) {
        await request('member' in plan.target ? 'revoke' : 'invite-revoke', 'member' in plan.target ? { target: plan.target.member } : { invitation: plan.target.invitation });
        plan.fenced = true; await save();
    }
    for (const document of plan.documents) {
        if (!document.delivered) {
            for (let offset = 0; offset < document.values.length; offset += 25) await request('key-write', { values: document.values.slice(offset, offset + 25) });
            for (const link of document.links) await request('live-key-write', link);
            document.delivered = true; await save();
        }
        if (!document.activated) {
            await request('rotate-document', { document: document.document, epoch: document.epoch });
            document.activated = true; await save();
        }
        if (!document.applied) {
            await apply(document); document.applied = true; await save();
        }
    }
    await repository.commit(plan.revision, null);
}
