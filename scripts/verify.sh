#!/usr/bin/env bash
# Runs this crate's verification recipe (CLAUDE.md's "Verification" section) as one command
# instead of six, filtering routine output and only widening it for a step that actually failed.
#
# A warning counts as a failure here even though cargo's own exit code ignores it, because
# CLAUDE.md's convention is that this tree is warning-free in every configuration below.
#
# The doctest run is split out from `cargo test --all-features`: it is known to *compile* but
# fail to *run* here, because of `dynamic_linking` on the `bevy` dev-dependency (chunk 28 owns
# the fix). That known failure is reported, not treated as a regression; anything else from the
# doctest step is.
#
# Usage: scripts/verify.sh [--full]
#   --full   also builds all eight device-feature combinations. Only needed when a `cfg` group
#            changed — see CLAUDE.md's "Context, and what not to economize on" — so it is not
#            part of the default run.

set -uo pipefail
cd "$(dirname "$0")/.."

full=0
[[ "${1:-}" == "--full" ]] && full=1

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

# The doctest step's known failure mode: compiles, then crashes at run time on a missing dylib.
# Anything else — a genuine compile error, a different runtime failure — is a real regression.
run_doctest_step() {
    local name="cargo test --all-features --doc"
    echo "== ${name} =="
    local output status
    output=$(cargo test --all-features --doc 2>&1)
    status=$?

    if [[ ${status} -eq 0 ]]; then
        echo "pass: ${name} (better than expected — dynamic_linking issue may be fixed; check chunk 28)"
    elif printf '%s\n' "${output}" | grep -q 'Library not loaded:.*libstd-'; then
        echo "known failure (dynamic_linking, chunk 28), not a regression: ${name}"
    else
        printf '%s\n' "${output}"
        echo "FAILED (exit ${status}, not the known dynamic_linking failure): ${name}"
        failed=1
        fail_names+=("${name}")
    fi
    echo
}

run_step "cargo fmt --check" cargo fmt --check
run_step "cargo check --all-features --tests --examples" \
    cargo check --all-features --tests --examples
run_step "cargo clippy --all-features --all-targets" cargo clippy --all-features --all-targets
run_step "cargo clippy --no-default-features --features libm" \
    cargo clippy --no-default-features --features libm
run_step "cargo test --all-features --lib --tests" cargo test --all-features --lib --tests
run_doctest_step
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
