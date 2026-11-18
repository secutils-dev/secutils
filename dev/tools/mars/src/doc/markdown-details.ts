// Markdown disclosures are one balanced block, so source mapping cannot close them between
// paragraphs. Only the body and summary are parsed as Markdown; all supplied HTML still goes
// through the caller's HTML renderer and the final preview safety policy.
import { Lexer, type Token, type TokenizerExtension, type RendererExtension, type Tokens } from 'marked';

/** A quoted HTML attribute can contain `>` without ending its tag. */
const ATTRIBUTES = String.raw`(?:"[^"]*"|'[^']*'|[^'">])*`;
/** Fence lines can carry the surrounding list indentation or blockquote markers. */
const FENCE_PREFIX = String.raw`(?: {0,3}>[ \t]?)*[ \t]*(?:(?:[-*+]|\d{1,9}[.)])[ \t]+)?`;
/** A disclosure starts with its native summary, with or without blank lines around it. */
const HEADER = new RegExp(
    String.raw`^ {0,3}(<details\b${ATTRIBUTES}>\s*<summary\b${ATTRIBUTES}>)([\s\S]*?)<\/summary\s*>[ \t]*(?:\n)?`,
    'i',
);
/** Ignore literal HTML and code when looking for the matching disclosure end tag. */
const BOUNDARY = new RegExp(
    String.raw`<!--[\s\S]*?(?:-->|(?![\s\S]))|<(pre|code|script|style|textarea)\b${ATTRIBUTES}>[\s\S]*?(?:<\/\1\s*>|(?![\s\S]))|^${FENCE_PREFIX}(${'`'}{3,}|~{3,})[^\n]*(?:\n|$)|^(?: {4}|\t)[^\n]*(?:\n|$)|${'`'}+|<\/?[a-z][a-z0-9-]*\b${ATTRIBUTES}>`,
    'gim',
);

/** The complete source block and its separately parsed summary and body. */
interface DetailsToken extends Tokens.Generic {
    type: 'details';
    opening: string;
    summaryTokens: Token[];
    tokens: Token[];
}

/**
 * A marked extension for balanced `<details><summary>` blocks. It leaves unmatched disclosures
 * to the ordinary HTML tokenizer and does not change source bytes or the caller's HTML policy.
 */
export const markdownDetails: TokenizerExtension & RendererExtension = {
    name: 'details',
    level: 'block',
    childTokens: ['summaryTokens', 'tokens'],
    start(src) {
        return src.search(/^ {0,3}<details\b/im);
    },
    tokenizer(src) {
        const header = HEADER.exec(src);
        if (!header) return;
        const scan = new RegExp(BOUNDARY);
        scan.lastIndex = header[0].length;
        let depth = 1;
        for (let match; (match = scan.exec(src));) {
            if (match[2]) {
                const fence = match[2];
                const end = new RegExp(
                    `^(?: {0,3}>[ \\t]?)*[ \\t]*${fence[0]}{${fence.length},}[ \\t]*(?:\\n|$)`,
                    'gm',
                );
                end.lastIndex = scan.lastIndex;
                if (!end.exec(src)) return;
                scan.lastIndex = end.lastIndex;
                continue;
            }
            if (match[0].startsWith('`')) {
                const code = Lexer.rules.inline.normal.code.exec(src.slice(match.index));
                if (code) scan.lastIndex = match.index + code[0].length;
                continue;
            }
            if (!/^<\/?details\b/i.test(match[0])) continue;
            depth += match[0].startsWith('</') ? -1 : 1;
            if (depth) continue;
            const raw = src.slice(0, scan.lastIndex);
            return {
                type: 'details',
                raw,
                opening: header[1],
                summaryTokens: this.lexer.inlineTokens(header[2]),
                tokens: this.lexer.blockTokens(src.slice(header[0].length, match.index)),
            } satisfies DetailsToken;
        }
    },
    renderer(token) {
        const details = token as DetailsToken;
        // These shell fragments span tokens like inline HTML. The host must keep their balance,
        // but still remove supplied attributes before it adds its own source-range annotations.
        const html = (text: string) =>
            this.parser.renderer.html({
                type: 'html',
                raw: text,
                text,
                block: false,
                inLink: false,
                inRawBlock: false,
            });
        return (
            html(details.opening) +
            this.parser.parseInline(details.summaryTokens) +
            html('</summary>') +
            this.parser.parse(details.tokens) +
            html('</details>')
        );
    },
};
