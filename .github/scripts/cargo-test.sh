#!/usr/bin/env bash
# Run `cargo test` with the given arguments. On failure, turn each failing
# test (name + captured output + panic message), crashed test binary and
# compile error into an `::error` annotation: annotations show up on the PR
# and through the API, where full logs are often out of reach.
set -uo pipefail

log=$(mktemp)
cargo test --no-fail-fast "$@" 2>&1 | tee "$log"
status=${PIPESTATUS[0]}
if [ "$status" -ne 0 ]; then
  awk '
    function esc_title(s) { gsub(/%/, "%25", s); gsub(/:/, "%3A", s); gsub(/,/, "%2C", s); return s }
    function esc(s) { gsub(/%/, "%25", s); return s }
    function add(cur, line, limit) { return length(cur) >= limit ? cur : cur (cur == "" ? "" : "%0A") esc(line) }
    # The panic message first, then the start of whatever the test printed.
    function flush() {
      if (name != "") print "::error title=" esc_title(name) "::" panic (panic != "" && out != "" ? "%0A----%0A" : "") out
      name = ""; panic = ""; out = ""; in_panic = 0
    }
    { sub(/\r$/, "") }
    /^---- .+ stdout ----$/ { flush(); name = $2; next }
    /^failures:$/ { flush(); next }
    /process didn.t exit successfully/ { flush(); print "::error title=test binary crashed::" esc($0); next }
    /^error(\[E[0-9]+\])?: / && !/^error: test failed/ { print "::error title=cargo::" esc($0); next }
    name == "" || NF == 0 { next }
    /panicked at/ { in_panic = 4 }
    in_panic > 0 { panic = add(panic, $0, 1500); in_panic--; next }
    { out = add(out, $0, 600) }
    END { flush() }
  ' "$log"
fi
rm -f "$log"
exit "$status"
