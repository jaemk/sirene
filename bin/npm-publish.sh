#!/bin/bash

# Publish the @jaemk/sirene package TARBALL as VERSION. A version already on
# the registry is a benign skip, so a re-run of a release stays idempotent.
# Authentication is npm trusted publishing (the release workflow's OIDC token);
# no npm token is stored anywhere.
#
#   bin/npm-publish.sh VERSION TARBALL

set -euo pipefail

version=${1:?version}
tarball=${2:?package tarball}

if [ "$(npm view "@jaemk/sirene@$version" version 2>/dev/null || true)" = "$version" ]; then
    echo "@jaemk/sirene@$version is already on npm - nothing to publish."
    exit 0
fi
npm publish "$tarball" --access public
echo "Published @jaemk/sirene@$version"
