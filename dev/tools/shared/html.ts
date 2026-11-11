// The HTML tokenizer sees script text before the JavaScript parser does.
// A literal comment opener followed by <script can enter double-escaped mode,
// consuming the closing tag and preventing the entire script from running.
export function assertInlineScriptSafety(html: string, label = 'HTML'): void {
    let index = 0;
    for (const [, code] of html.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script\s*>/gi)) {
        index++;
        if (code.includes('<!--')) {
            throw new Error(`${label}: script #${index} contains a literal HTML comment opener. Escape < in script data before embedding it.`);
        }
    }
}
