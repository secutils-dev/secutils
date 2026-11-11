import * as app from '../app.ts';
import * as db from '../store/db.ts';
import * as sync from '../sync/index.ts';
import { snapshotLink } from './share.ts';
import { copyRich, esc } from '../ui/ui.ts';
import { copyFeedback, hideTooltip } from '../ui/feedback.ts';
import { SyncDialog } from '../sync/dialog.ts';

export async function openShareDialog(id: string, initial: 'snapshot' | 'live' = 'snapshot') {
    const node = app.node(id); if (!node) return;
    const received = node.connection?.kind === 'live';
    let original = received ? await sync.liveLinkInfo(id) : null;
    const canCreate = node.kind === 'doc' && !db.isTemporaryStorage() && (!node.connection || node.connection.permission === 'owner');
    const canLive = canCreate || !!original;
    const ui = new SyncDialog('copyLinkDialog'); let busy = false;
    let refreshTimer: ReturnType<typeof setInterval> | undefined;
    let cachedLive: { key: string; value: { url: string; expires: number } } | null = null;
    let mode = initial === 'live' && canLive ? 'live' : 'snapshot', format = 'full';
    const lifetimes = { snapshot: 604800, live: 604800 };
    const showLink = (url = '') => {
        if (!url) hideTooltip();
        ui.element.querySelector<HTMLInputElement>('#copyLinkValue')!.value = url;
        ui.element.querySelector<HTMLElement>('#copyLinkReady')!.hidden = !url;
        ui.element.querySelector<HTMLElement>('#copyLinkHelp')!.hidden = !!url;
        const connection = app.node(id)?.connection;
        ui.element.querySelector<HTMLButtonElement>('footer .sync-footer-left')!.hidden = connection?.permission !== 'owner' || !connection.liveShared;
    };
    try {
        const answer = ui.screen({ title: 'Copy link', html: `<p><strong>${esc(node.name)}</strong></p>
            <fieldset class="copy-options"><legend class="field-label">Share</legend><label><input type="radio" name="copyMode" value="snapshot"${mode === 'snapshot' ? ' checked' : ''}> Snapshot</label><label><input type="radio" name="copyMode" value="live"${mode === 'live' ? ' checked' : ''}${canLive ? '' : ' disabled'}> Live</label></fieldset>
            <p id="copyModeHelp" class="sync-fineprint"></p>
            <fieldset id="copyFormatFields" class="copy-options"><legend class="field-label">Link format</legend><label><input type="radio" name="copyFormat" value="full" checked> Full</label><label><input type="radio" name="copyFormat" value="short"> Short</label></fieldset>
            <div id="copyExpiryFields" class="live-expiry"><label class="field-label" for="copyLifetime">Expires</label><select class="select" id="copyLifetime"></select></div>
            <fieldset id="copyAccessFields" class="live-access"><legend class="field-label">Access</legend><label><input type="radio" name="copyPermission" value="view" aria-label="Can view" checked> Can view</label><label><input type="radio" name="copyPermission" value="comment" aria-label="Can comment"> Can comment</label><label><input type="radio" name="copyPermission" value="edit" aria-label="Can edit"> Can edit</label></fieldset>
            <p id="copyForwardInfo" class="sync-fineprint" hidden></p>
            <div class="copy-link-output"><p id="copyLinkHelp" class="sync-fineprint">Click Copy link to create and copy your link.</p><div id="copyLinkReady" hidden><label class="field-label" for="copyLinkValue">Link</label><input class="field" id="copyLinkValue" type="text" readonly autocomplete="off" spellcheck="false"></div></div>`, buttons: [{ label: 'Live links', value: 'manage', empty: true, left: true }, { label: 'Cancel', value: 'close' }, { label: 'Copy link', value: 'copy', primary: true, run: async button => {
                if (busy) return; busy = true;
                const permission = ui.element.querySelector<HTMLInputElement>('input[name="copyPermission"]:checked')!.value as 'view' | 'comment' | 'edit';
                const ttl = lifetimes[mode as 'snapshot' | 'live'];
                const key = `${format}:${permission}:${ttl}`;
                ui.busy('Copying link…');
                const result = mode === 'live' ? received ? sync.forwardLiveLink(id) : cachedLive?.key === key && cachedLive.value.expires > Date.now() / 1000 + 60 ? Promise.resolve(cachedLive.value) : sync.createLiveLink(id, permission, ttl, format as 'short' | 'full') : snapshotLink(id, format as 'short' | 'full', ttl);
                // Start the clipboard write in the button gesture, before network work finishes.
                const copied = copyRich(result.then(value => value.url));
                try {
                    const [value] = await Promise.all([result, copied]);
                    if (mode === 'live' && !received && value.expires !== null) cachedLive = { key, value: { url: value.url, expires: value.expires } };
                    if (ui.open) { showLink(value.url); ui.idle(); update(); copyFeedback(button); }
                } catch (error) {
                    ui.idle();
                    const value = await result.catch(() => null);
                    if (ui.open && value) { if (mode === 'live' && !received && value.expires !== null) cachedLive = { key, value: { url: value.url, expires: value.expires } }; showLink(value.url); ui.message('Could not copy. Select the link above and copy it manually.'); }
                    else if (ui.open) { const message = (error as Error).message; ui.message(message === 'sync-pending' ? 'Your latest changes are still syncing. Try again when they finish.' : /^[a-z]+(?:-[a-z]+)+$/.test(message) ? 'Could not create the link. Your document is unchanged. Try again.' : message); }
                } finally { busy = false; if (ui.open) { if (received) { original = await sync.liveLinkInfo(id); if (!original && mode === 'live') showLink(); } update(); } }
            } }] });
        const update = () => {
            const live = mode === 'live', forwarding = live && received, expires = !forwarding && (live || format === 'short');
            ui.element.querySelector<HTMLInputElement>('input[name="copyMode"][value="live"]')!.disabled = !canCreate && !original;
            ui.element.querySelector<HTMLElement>('#copyFormatFields')!.hidden = forwarding;
            ui.element.querySelector<HTMLElement>('#copyExpiryFields')!.hidden = !expires;
            ui.element.querySelector<HTMLElement>('#copyAccessFields')!.hidden = !live || forwarding;
            const info = ui.element.querySelector<HTMLElement>('#copyForwardInfo')!; info.hidden = !forwarding || !original;
            if (original) info.textContent = `${{ view: 'View only', comment: 'Can comment', edit: 'Can edit', owner: 'Owner' }[original.permission]} · Expires ${new Date(original.expires * 1000).toLocaleDateString()}.`;
            const copy = ui.element.querySelector<HTMLButtonElement>('footer .btn-primary')!;
            if (!busy) copy.disabled = forwarding && !original;
            ui.element.querySelector<HTMLElement>('#copyLinkHelp')!.textContent = forwarding ? "Click Copy link to copy the owner's Live link." : 'Click Copy link to create and copy your link.';
            const help = ui.element.querySelector<HTMLElement>('#copyModeHelp')!;
            help.textContent = forwarding ? original ? "Forward the owner's Live link. Access and expiry stay unchanged." : 'The Live link ended. Choose Snapshot to share your local copy.' : live ? 'Receives later document changes. Choose what people can do.' : node.kind === 'folder' ? 'A copy of this folder and its documents. Later changes are not included. Live links are available for individual documents.' : 'A copy of this document. Later changes are not included.';
            if (received && !original && !forwarding) help.textContent += ' The Live link ended. Only snapshots can be shared.';
            else if (!canLive && node.kind === 'doc') help.textContent += ' Only the sharing source can create Live links.';
            if (!live && !expires) help.textContent += ' Full snapshot links do not expire.';
            const ttl = ui.element.querySelector<HTMLSelectElement>('#copyLifetime')!;
            ttl.innerHTML = [1, 7, 14, 30].map(days => `<option value="${days * 86400}"${lifetimes[mode as 'snapshot' | 'live'] === days * 86400 ? ' selected' : ''}>${days} ${days === 1 ? 'day' : 'days'}</option>`).join('');
        };
        for (const radio of ui.element.querySelectorAll<HTMLInputElement>('input[name="copyMode"], input[name="copyFormat"]')) radio.onchange = () => { if (radio.name === 'copyMode') mode = radio.value; else format = radio.value; showLink(); ui.message(''); update(); };
        ui.element.querySelector<HTMLSelectElement>('#copyLifetime')!.onchange = event => { lifetimes[mode as 'snapshot' | 'live'] = Number((event.target as HTMLSelectElement).value); showLink(); ui.message(''); };
        for (const radio of ui.element.querySelectorAll<HTMLInputElement>('input[name="copyPermission"]')) radio.onchange = () => { showLink(); ui.message(''); };
        update(); showLink();
        if (original) refreshTimer = setInterval(() => {
            if (original && (app.node(id)?.connection?.phase === 'ended' || !app.node(id)?.connection || original.expires <= Date.now() / 1000)) {
                original = null; if (mode === 'live') showLink(); update();
            }
        }, 1000);
        if ((await answer).value === 'manage') { ui.close(); if (!document.querySelector<HTMLDialogElement>('#liveLinksDialog')?.open) await sync.liveLinks(id); }
    } finally { clearInterval(refreshTimer); ui.close(); }
}
