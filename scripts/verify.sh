#!/usr/bin/env bash
# Runs this crate's verification recipe (CLAUDE.md's "Verification" section) as one command
# instead of six, filtering routine output and only widening it for a step that actually failed.
#
# A warning counts as a failure here even though cargo's own exit code ignores it, because
# CLAUDE.md's convention is that this tree is warning-free in every configuration below.
#
# Usage: scripts/verify.sh [--full] [--doc]
#   --full   also builds all eight device-feature combinations. Only needed when a `cfg` group
#            changed — see CLAUDE.md's "Context, and what not to economize on" — so it is not
#            part of the default run.
#   --doc    also runs the doctests. Out of the default run because the doc examples are stable
#            and the step pays for a separate compile of the merged doctest binary.

set -uo pipefail
cd "$(dirname "$0")/.."

full=0
doc=0
for arg in "$@"; do
    case "${arg}" in
        --full) full=1 ;;
        --doc) doc=1 ;;
        *)
            echo "usage: $0 [--full] [--doc]" >&2
            exit 2
            ;;
    esac
done

failed=0
declare -a fail_names=()

# Runs one step, capturing combined output. Passes if the command exits 0 *and* prints no
# "warning:" line; on failure, prints the full output rather than staying filtered, since a
# filtered failure is useless for debugging.
run_step() {
    local name="$1"
    shift
    echo "== ${name} =="
    local output status warnings
    output=$("$@" 2>&1)
    status=$?
    warnings=$(printf '%s\n' "${output}" | grep -c '^warning' || true)

    if [[ ${status} -ne 0 ]]; then
        printf '%s\n' "${output}"
        echo "FAILED (exit ${status}): ${name}"
        failed=1
        fail_names+=("${name}")
    elif [[ ${warnings} -gt 0 ]]; then
        printf '%s\n' "${output}" | grep '^warning'
        echo "FAILED (warnings): ${name}"
        failed=1
        fail_names+=("${name} (warnings)")
    else
        printf '%s\n' "${output}" | grep -E 'test result|FAILED' || echo "ok"
        echo "pass: ${name}"
    fi
    echo
}

# `dynamic_linking` on the `bevy` dev-dependency leaves the merged doctest binary without an rpath
# to the toolchain's own libstd, so it builds and then dies in dyld. Pointing dyld at the directory
# rather than the hashed filename keeps this correct across toolchain updates, and FALLBACK is
# consulted only after normal resolution, so it cannot shadow a real linking failure.
#
# This repairs the run, not the crate — a plain `cargo test --doc` still dies, and on anything but
# macOS so does this. The portable fix is chunk 28's.
run_doc_step() {
    local name="cargo test --workspace --all-features --doc"
    if [[ "$(uname -s)" != "Darwin" ]]; then
        echo "== ${name} =="
        echo "skipped: the dyld workaround is macOS-only — chunk 28 owns the portable fix"
        echo
        return
    fi
    local libdir
    libdir="$(rustc --print target-libdir)"
    # Appended rather than assigned: setting this variable at all discards dyld's own fallback list.
    export DYLD_FALLBACK_LIBRARY_PATH="${libdir}:${HOME}/lib:/usr/local/lib:/usr/lib"
    # `--workspace`: a bare `cargo test` takes the root package, which leaves the macros crate's
    # own doctest unreached — it had never been compiled.
    run_step "${name}" cargo test --workspace --all-features --doc
    unset DYLD_FALLBACK_LIBRARY_PATH
}

# First because it costs milliseconds, and because a reference that stopped resolving is the one
# kind of breakage nothing else here would ever notice.
run_step "scripts/xref.py" python3 scripts/xref.py --quiet
run_step "cargo fmt --check" cargo fmt --check
run_step "cargo check --all-features --tests --examples" \
    cargo check --all-features --tests --examples
# The examples are what a reader runs, and they run them with the default features. Checking them
# only under --all-features hid a settings group whose reflect type data was behind `serialize`:
# it compiled either way and panicked at runtime in the build anyone would actually start.
run_step "cargo check --examples" cargo check --examples
run_step "cargo clippy --all-features --all-targets" cargo clippy --all-features --all-targets
run_step "cargo clippy --no-default-features --features libm" \
    cargo clippy --no-default-features --features libm
run_step "cargo test --all-features --lib --tests" cargo test --all-features --lib --tests
[[ ${doc} -eq 1 ]] && run_doc_step
run_step "cargo test --no-default-features --features libm --test no_devices" \
    cargo test --no-default-features --features libm --test no_devices
run_step "cargo test --no-default-features --features std,mouse,gamepad --test focus_loss_without_keyboard" \
    cargo test --no-default-features --features std,mouse,gamepad --test focus_loss_without_keyboard

if [[ ${full} -eq 1 ]]; then
    echo "== device-feature matrix =="
    for combo in "" keyboard mouse gamepad keyboard,mouse keyboard,gamepad mouse,gamepad \
        keyboard,mouse,gamepad; do
        out=$(cargo check --no-default-features --features "std,bevy_reflect,${combo}" 2>&1)
        if [[ $? -ne 0 ]]; then
            printf '%s\n' "${out}"
            echo "FAILED: device matrix [${combo}]"
            failed=1
            fail_names+=("device matrix [${combo}]")
        else
            echo "pass: [${combo}]"
        fi
    done
    echo
fi

echo "=================================="
if [[ ${failed} -eq 0 ]]; then
    echo "All checks passed."
else
    echo "FAILED:"
    printf '  - %s\n' "${fail_names[@]}"
fi
exit ${failed}
