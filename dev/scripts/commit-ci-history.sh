#!/usr/bin/env bash
#
# Commit a single auto-generated history file and push it to the current branch.
#
# Multiple CI jobs (the webui bundle-size job and the JS runtime perf job) run in
# parallel on every push to `main` and each commit a different history file back
# to the branch. They all check out the same triggering SHA, so the first one to
# push advances `origin/main` and the others become non-fast-forward and get
# rejected. Because every job touches a distinct, append-only history file, we can
# safely fetch + rebase our single commit onto the latest branch tip and retry the
# push until it lands.
#
# Usage: commit-ci-history.sh <file> <commit-message>

set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "Usage: $0 <file> <commit-message>" >&2
  exit 2
fi

FILE="$1"
MESSAGE="$2"
BRANCH="${GITHUB_REF_NAME:-$(git rev-parse --abbrev-ref HEAD)}"
MAX_ATTEMPTS="${PUSH_MAX_ATTEMPTS:-5}"

git config user.name "github-actions[bot]"
git config user.email "github-actions[bot]@users.noreply.github.com"

git add "$FILE"
if git diff --cached --quiet; then
  echo "No changes to ${FILE}; nothing to commit."
  exit 0
fi

git commit -m "$MESSAGE"

for attempt in $(seq 1 "$MAX_ATTEMPTS"); do
  if git push origin "HEAD:${BRANCH}"; then
    echo "Pushed ${FILE} on attempt ${attempt}/${MAX_ATTEMPTS}."
    exit 0
  fi

  echo "Push rejected (attempt ${attempt}/${MAX_ATTEMPTS}); rebasing onto origin/${BRANCH} and retrying..."
  git fetch origin "$BRANCH"
  # The racing job only ever touches a different history file, so replaying our
  # single commit onto the new tip never conflicts.
  git rebase "origin/${BRANCH}"
done

echo "Failed to push ${FILE} after ${MAX_ATTEMPTS} attempts." >&2
exit 1
