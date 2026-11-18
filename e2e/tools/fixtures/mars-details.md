# TODO

## Review notes

1. Explore the report in https://example.com/review?thread=123&context=456

<details>
<summary>Section Title</summary>
**A reviewer found a reproducible formatting issue. This first paragraph must render as Markdown without an extra blank line after the summary.**

The distinction is between **reading the source** and **rendering the document**:

- The source keeps the author’s formatting markers and HTML tags.
- The preview shows `NOT (id=A OR id=B OR id=C …)` as code. A relative [source.ts:123](/reference/source.ts:123) link remains a link.

Later paragraphs must remain inside the disclosure. [A reference](https://example.com/reference) must remain clickable.

**This sentence starts another formatted paragraph.** The following plain text belongs to the same paragraph.

Nested inline formatting also works: a **`terms` query** combines bold text with inline code. [Terms query](https://www.elastic.co/docs/reference/query-languages/query-dsl/query-dsl-terms-query).

Collapsing the disclosure hides **all its body content**, not only the first paragraph.

I’d adjust the reply to:

> Yeah, good catch. Keep the entire body inside the disclosure, including this final blockquote.
</details>
