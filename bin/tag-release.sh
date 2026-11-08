#!/bin/bash

# Create a git tag and GitHub release for every publishable workspace crate
# whose CURRENT version is not yet tagged and released on the remote.
#
# Idempotent: a crate with both a tag and a release is skipped, and a crate with
# one but not the other gets only the missing half, so re-running after a run
# that published but died before tagging finishes the job. Only the version in
# each Cargo.toml now is considered; historical versions are not backfilled.
#
# Tag naming:
#   - root crate `sirene`  -> vX.Y.Z (the macOS app is attached to this release)
#   - other crates         -> <crate-name>-vX.Y.Z
#
# Requires: git, jq, cargo, and the gh CLI authenticated (GH_TOKEN in CI).

set -euo pipefail

ROOT_CRATE="sirene"

# Tag as the bot in CI; leave a local user's identity alone otherwise.
if [ "${GITHUB_ACTIONS:-}" = "true" ]; then
    git config --local user.name "github-actions[bot]"
    git config --local user.email "github-actions[bot]@users.noreply.github.com"
fi

tag_exists_on_remote() {
    # Match the tab-prefixed full ref as a fixed string, so `v0.1.0` does not
    # match `v0.1.0-rc1` and dots are not regex metacharacters.
    git ls-remote --tags origin "refs/tags/$1" | grep -qF -- "	refs/tags/$1"
}

release_exists() {
    gh release view "$1" >/dev/null 2>&1
}

tag_and_release() {
    local tag=$1
    # $2: whether the release may be marked "Latest" (true or false).
    local tag_remote release_remote
    tag_exists_on_remote "$tag" && tag_remote=true || tag_remote=false
    release_exists "$tag"       && release_remote=true || release_remote=false

    if [ "$tag_remote" = true ] && [ "$release_remote" = true ]; then
        echo "Tag $tag and its GitHub release already exist - skipping."
        return 0
    fi
    echo "Creating tag $tag and GitHub release..."
    if ! git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
        git tag -a "$tag" -m "Release $tag"
    fi
    if [ "$tag_remote" = false ]; then
        git push origin "$tag"
    fi
    if [ "$release_remote" = false ]; then
        # Only the root crate's release may become "Latest", where the macOS app
        # is attached; otherwise whichever crate is tagged last would take it.
        gh release create "$tag" --generate-notes --title "$tag" --latest="$2"
    fi
}

members=$(cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[] | select(.publish != []) | "\(.name) \(.version)"')

while read -r name version; do
    [ -z "$name" ] && continue
    if [ "$name" = "$ROOT_CRATE" ]; then
        tag_and_release "v$version" true
    else
        tag_and_release "$name-v$version" false
    fi
done <<< "$members"
