#!/usr/bin/env bash
set -euo pipefail

MODE="full"
while [ "$#" -gt 0 ]; do
    case "$1" in
        --mode)
            MODE="${2:?mode value is required}"
            shift 2
            ;;
        *)
            printf 'unknown argument: %s\n' "$1" >&2
            exit 1
            ;;
    esac
done

print_step() {
    printf '\n==> %s\n' "$1"
}

allow_dirty_flag=()
if [ -z "${CI:-}" ]; then
    allow_dirty_flag=(--allow-dirty)
fi

list_publishable_crates() {
    cargo metadata --no-deps --format-version 1 | python3 -c '
import json
import os
import sys

metadata = json.load(sys.stdin)
packages = {package["id"]: package for package in metadata["packages"]}
members = metadata.get("workspace_default_members") or metadata["workspace_members"]

for package_id in members:
    package = packages[package_id]
    publish = package.get("publish")
    if publish == [] or publish is False:
        continue
    print(package["name"])
'
}

run_package_checks() {
    local crate_name

    print_step "Packaging and crates.io dry-run checks"
    for crate_name in $(list_publishable_crates); do
        printf 'checking crate: %s\n' "${crate_name}"
        cargo package --no-verify -p "${crate_name}" "${allow_dirty_flag[@]}"
        cargo package -p "${crate_name}" "${allow_dirty_flag[@]}"
        bash ci/check_package_contents.sh "${crate_name}" "${allow_dirty_flag[@]}"
        cargo publish -p "${crate_name}" --dry-run "${allow_dirty_flag[@]}"
    done
}

case "${MODE}" in
    full)
        print_step "cargo fmt"
        cargo fmt --all -- --check

        print_step "cargo clippy"
        cargo clippy --workspace --all-targets --all-features -- -D warnings

        print_step "cargo test"
        cargo test --workspace --all-features

        print_step "cargo build"
        cargo build --workspace --all-features

        print_step "cargo doc"
        cargo doc --workspace --no-deps --all-features

        run_package_checks
        ;;
    package)
        run_package_checks
        ;;
    *)
        printf 'unsupported mode: %s\n' "${MODE}" >&2
        exit 1
        ;;
esac
