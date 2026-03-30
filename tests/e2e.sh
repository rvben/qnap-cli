#!/usr/bin/env bash
# End-to-end tests against a live QNAP instance.
# Requires saved credentials (run `qnap login` first).
# Uses /Public/e2e-<timestamp> as a scratch directory, cleaned up on exit.

set -euo pipefail

QNAP="${QNAP_BIN:-qnap}"
SCRATCH="/Public/e2e-$(date +%s)"
UPLOAD_SRC="$(mktemp)"
DOWNLOAD_DST="$(mktemp)"
PASS=0
FAIL=0

cleanup() {
    rm -f "$UPLOAD_SRC" "$DOWNLOAD_DST"
    "$QNAP" files rm "$SCRATCH" 2>/dev/null || true
}
trap cleanup EXIT

ok() {
    echo "  PASS  $1"
    PASS=$((PASS + 1))
}

fail() {
    echo "  FAIL  $1: $2"
    FAIL=$((FAIL + 1))
}

run() {
    local label="$1"; shift
    if output=$("$@" 2>&1); then
        ok "$label"
    else
        fail "$label" "$output"
    fi
}

run_match() {
    local label="$1"
    local pattern="$2"; shift 2
    if output=$("$@" 2>&1) && echo "$output" | grep -q "$pattern"; then
        ok "$label"
    else
        fail "$label" "expected pattern '$pattern', got: $output"
    fi
}

SCRATCH_NAME="${SCRATCH##*/}"

echo "e2e: scratch dir $SCRATCH"
echo ""
echo "--- System commands ---"
run         "info"              "$QNAP" info
run         "info --json"       "$QNAP" info --json
run         "status"            "$QNAP" status
run         "status --json"     "$QNAP" status --json
run         "volumes"           "$QNAP" volumes
run         "volumes --json"    "$QNAP" volumes --json
run         "shares"            "$QNAP" shares
run         "shares --json"     "$QNAP" shares --json

echo ""
echo "--- File operations ---"

run         "files mkdir"               "$QNAP" files mkdir "$SCRATCH"
run_match   "files ls (scratch)"        "$SCRATCH_NAME" "$QNAP" files ls /Public

echo "hello from qnap e2e test" > "$UPLOAD_SRC"
run         "files upload"              "$QNAP" files upload "$UPLOAD_SRC" "$SCRATCH"

REMOTE_FILE="$SCRATCH/$(basename "$UPLOAD_SRC")"

run_match   "files stat (file)"         "size"   "$QNAP" files stat "$REMOTE_FILE"
run_match   "files stat --json"         "\"size_bytes\"" "$QNAP" files stat "$REMOTE_FILE" --json
run         "files ls --all"            "$QNAP" files ls "$SCRATCH" --all

run         "files mkdir (subdir)"      "$QNAP" files mkdir "$SCRATCH/subdir"

# cp: same name, different dir
run         "files cp (same name)"      "$QNAP" files cp "$REMOTE_FILE" "$SCRATCH/subdir/$(basename "$UPLOAD_SRC")"

# cp: different name, different dir (copy + rename internally)
run         "files cp (rename)"         "$QNAP" files cp "$REMOTE_FILE" "$SCRATCH/subdir/renamed_copy.txt"

# mv: rename within same dir
run         "files mv (rename)"         "$QNAP" files mv "$REMOTE_FILE" "$SCRATCH/original_renamed.txt"

# mv: cross-dir with rename
run         "files mv (cross-dir)"      "$QNAP" files mv "$SCRATCH/subdir/renamed_copy.txt" "$SCRATCH/moved_out.txt"

# download and verify content
run         "files download"            "$QNAP" files download "$SCRATCH/original_renamed.txt" "$DOWNLOAD_DST"
if grep -q "hello from qnap e2e test" "$DOWNLOAD_DST"; then
    ok "files download (content)"
else
    fail "files download (content)" "downloaded file content mismatch"
fi

# rm individual files
run         "files rm (file)"           "$QNAP" files rm "$SCRATCH/original_renamed.txt"
run         "files rm (file 2)"         "$QNAP" files rm "$SCRATCH/moved_out.txt"

# rm subdir (contains the same-name copy)
run         "files rm (dir)"            "$QNAP" files rm "$SCRATCH/subdir"

echo ""
echo "--- Results ---"
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
