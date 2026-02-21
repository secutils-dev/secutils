export type ContentSecurityPolicyDirectives = Map<string, string[]>;
export type SerializedContentSecurityPolicyDirectives = Array<{ name: string; value: string[] }>;

export interface ContentSecurityPolicy<Directives = ContentSecurityPolicyDirectives> {
  id: string;
  name: string;
  directives: Directives;
  createdAt: number;
  updatedAt: number;
}

export function getContentSecurityPolicyString(policy: ContentSecurityPolicy) {
  return Array.from(policy.directives.entries())
    .map(([directiveName, directiveValues]) =>
      directiveValues.length > 0 ? `${directiveName} ${[...directiveValues].sort().join(' ')}` : `${directiveName}`,
    )
    .join('; ');
}

export function deserializeContentSecurityPolicyDirectives(
  serializedDirectives: SerializedContentSecurityPolicyDirectives,
): ContentSecurityPolicyDirectives {
  return new Map(
    serializedDirectives.map((directive) => {
      return [directive.name, directive.value ?? []];
    }),
  );
}

export function serializeContentSecurityPolicyDirectives(
  directives: ContentSecurityPolicyDirectives,
): SerializedContentSecurityPolicyDirectives {
  return Array.from(directives).map(([directiveName, directiveValues]) => ({
    name: directiveName,
    value: directiveValues,
  }));
}
