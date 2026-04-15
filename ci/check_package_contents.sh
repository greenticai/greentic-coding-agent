#!/usr/bin/env bash
set -euo pipefail

crate_name="${1:?crate name is required}"
shift

package_listing="$(cargo package --list -p "${crate_name}" "$@")"

require_path() {
    local path="$1"
    if ! printf '%s\n' "${package_listing}" | grep -Fx "${path}" >/dev/null 2>&1; then
        printf 'missing required packaged path for %s: %s\n' "${crate_name}" "${path}" >&2
        exit 1
    fi
}

require_path "Cargo.toml"
require_path "README.md"
require_path "LICENSE"

if ! printf '%s\n' "${package_listing}" | grep -E '^src/(main|lib)\.rs$' >/dev/null 2>&1; then
    printf 'missing required packaged source entrypoint for %s\n' "${crate_name}" >&2
    exit 1
fi

printf 'package contents verified for %s\n' "${crate_name}"
