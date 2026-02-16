/**
 * Score-based fuzzy matching that returns a match score (higher = better match).
 * Returns 0 if no match, higher values for better matches.
 */
export function fuzzyMatchScore(query: string, target: string): number {
  const lowerQuery = query.toLowerCase();
  const lowerTarget = target.toLowerCase();

  // The exact match gets the highest score.
  if (lowerTarget === lowerQuery) {
    return 1000;
  }

  // Starts with the query gets the high score
  if (lowerTarget.startsWith(lowerQuery)) {
    return 500;
  }

  // Contains the exact substring gets a good score.
  if (lowerTarget.includes(lowerQuery)) {
    return 200;
  }

  // Word boundary match
  const words = lowerTarget.split(/[\s_-]+/);
  for (const word of words) {
    if (word.startsWith(lowerQuery)) {
      return 300;
    }
  }

  // Fuzzy match: all characters must appear in order
  let queryIndex = 0;
  let consecutiveMatches = 0;
  let maxConsecutive = 0;

  for (let i = 0; i < lowerTarget.length && queryIndex < lowerQuery.length; i++) {
    if (lowerTarget[i] === lowerQuery[queryIndex]) {
      queryIndex++;
      consecutiveMatches++;
      maxConsecutive = Math.max(maxConsecutive, consecutiveMatches);
    } else {
      consecutiveMatches = 0;
    }
  }

  // Return score based on how many characters matched and consecutive matches
  if (queryIndex === lowerQuery.length) {
    return 50 + maxConsecutive * 10;
  }

  return 0;
}

/**
 * Filter and sort items using fuzzy search across multiple fields.
 */
export function filterItemsFuzzy<T>(items: T[], query: string, getSearchFields: (item: T) => string[]): T[] {
  if (!query.trim()) {
    return items;
  }

  const matches: { item: T; score: number }[] = [];

  for (const item of items) {
    let maxScore = 0;
    for (const field of getSearchFields(item)) {
      const score = fuzzyMatchScore(query, field);
      if (score > maxScore) {
        maxScore = score;
      }
    }

    if (maxScore > 0) {
      matches.push({ item, score: maxScore });
    }
  }

  return matches.sort((a, b) => b.score - a.score).map(({ item }) => item);
}
