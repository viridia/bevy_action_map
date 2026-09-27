#!/usr/bin/env bash
# A dependency's source at the version `Cargo.lock` holds, grepped or located in one call, so that
# settling how Bevy or gilrs behaves never needs a glob over the registry or a `cd` into it.
#
# With only a crate, prints its source directory. With grep arguments as well, greps that directory
# recursively as extended regular expressions: paths come out relative to the directory, which is
# printed first on stderr, and each line is cut at WIDTH characters as `mentions.sh` does.
#
# Usage: scripts/depsrc.sh <crate>[@<version>] [grep arguments]
#   A crate locked at two versions lists both and exits 2; name one with `@`.
#   Exits 1 when the crate is not a registry package in the lock, or grep finds nothing.

set -uo pipefail
cd "$(dirname "$0")/.."

WIDTH=160

if [[ $# -lt 1 ]]; then
    echo "usage: $0 <crate>[@<version>] [grep arguments]" >&2
    exit 2
fi

crate="${1%%@*}"
wanted=""
[[ "$1" == *@* ]] && wanted="${1#*@}"
shift

# Registry packages only: a path or git dependency has no directory under `registry/src`.
versions="$(awk -v name="${crate}" '
    /^\[\[package\]\]/ { n = ""; v = "" }
    /^name = /    { n = $3; gsub(/"/, "", n) }
    /^version = / { v = $3; gsub(/"/, "", v) }
    /^source = "registry\+/ && n == name { print v }
' Cargo.lock | sort -u)"

if [[ -n "${wanted}" ]]; then
    versions="$(printf '%s\n' "${versions}" | grep -Fx -- "${wanted}")"
fi
if [[ -z "${versions}" ]]; then
    echo "${crate}${wanted:+@${wanted}}: not a registry package in Cargo.lock" >&2
    exit 1
fi
if [[ $(printf '%s\n' "${versions}" | wc -l) -gt 1 ]]; then
    echo "${crate} is locked at more than one version; name one as ${crate}@<version>:" >&2
    printf '  %s\n' ${versions} >&2
    exit 2
fi

registry="${CARGO_HOME:-${HOME}/.cargo}/registry/src"
dir=""
for index in "${registry}"/*/; do
    if [[ -d "${index}${crate}-${versions}" ]]; then
        dir="${index}${crate}-${versions}"
        break
    fi
done
if [[ -z "${dir}" ]]; then
    echo "${crate} ${versions} is locked but not unpacked under ${registry}; run cargo fetch" >&2
    exit 1
fi

if [[ $# -eq 0 ]]; then
    echo "${dir}"
    exit 0
fi

echo "${dir}" >&2
cd "${dir}" || exit 1
out="$(grep -rnIE --color=never "$@" .)"
status=$?
if [[ ${status} -ne 0 ]]; then
    exit "${status}"
fi
printf '%s\n' "${out}" | sed 's|^\./||' | LC_ALL=en_US.UTF-8 cut -c "1-${WIDTH}"
