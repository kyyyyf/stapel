#!/bin/sh
# A fake `cargo` for the tests of `stapel check` (STP-35): it compiles nothing. It answers
# `cargo --version` and `cargo test` from markers in the fixture's sources, and prints the lines
# that libtest prints. The working directory is the fixture's workspace: `crates/<dir>/src` and
# `crates/<dir>/tests/<file>.rs`.
#
# Markers, as a comment line in the body of a test function:
#   // fake: needs <text>   the test passes when a file under `crates/*/src` has <text>, else fails
#   // fake: item <text>    the test file compiles when a file under `crates/*/src` has <text>
# A line `// fake: sleeps <n>` before the tests makes a run of the file sleep <n> seconds first
# (STP-45).
# A test with no marker passes. A file under `crates/*/src` with the text `this does not compile`
# breaks the library: nothing compiles.

if [ "$1" = --version ]; then
    echo "cargo 0.0.0 (fake)"
    exit 0
fi
if [ "$1" != test ]; then
    echo "error: the fake cargo runs only test and --version" >&2
    exit 1
fi
shift

TAB=$(printf '\t')

in_src() {
    grep -rqF -- "$1" crates/*/src 2>/dev/null
}

# One line per test of a file: name, kind, text.
markers() {
    awk '
        /^fn [A-Za-z0-9_]+\(\)/ { name = $2; sub(/\(.*/, "", name); n++; names[n] = name; kind[n] = "ok"; text[n] = "-"; next }
        /\/\/ fake: / && n > 0 {
            s = $0
            sub(/.*\/\/ fake: /, "", s)
            k = index(s, " ")
            kind[n] = substr(s, 1, k - 1)
            text[n] = substr(s, k + 1)
        }
        END { for (i = 1; i <= n; i++) print names[i] "\t" kind[i] "\t" text[i] }
    ' "$1"
}

# Exit status 0 when every `item` marker of the file is found.
compiles() {
    markers "$1" | while IFS=$TAB read -r _ kind text; do
        if [ "$kind" = item ] && ! in_src "$text"; then
            exit 1
        fi
    done
}

# The state of one test: ok or FAILED.
state() {
    if [ "$2" = needs ] && ! in_src "$3"; then
        echo FAILED
    else
        echo ok
    fi
}

# Runs the tests of a file named in "$2" (all when empty) and prints the libtest lines; the
# exit status says whether one failed.
run_file() {
    file=$1
    wanted=$2
    selected=$(markers "$file" | while IFS=$TAB read -r name kind text; do
        if [ -z "$wanted" ] || printf '%s\n' "$wanted" | grep -qxF -- "$name"; then
            echo "$name$TAB$(state "$name" "$kind" "$text")"
        fi
    done)
    # A line `// fake: finished <x>` before the tests sets the time libtest prints (default 0.00);
    # `// fake: finished none` leaves the `finished in` part out (STP-45).
    fin=$(sed -n 's|^// fake: finished ||p' "$file" | head -1)
    [ -z "$fin" ] && fin=0.00
    if [ "$fin" = none ]; then
        finished=""
    else
        finished="; finished in ${fin}s"
    fi
    nap=$(sed -n 's|^// fake: sleeps ||p' "$file" | head -1)
    if [ -n "$nap" ]; then
        sleep "$nap"
    fi
    total=0
    passed=0
    failed=0
    list=""
    if [ -n "$selected" ]; then
        total=$(printf '%s\n' "$selected" | wc -l | tr -d ' ')
        passed=$(printf '%s\n' "$selected" | grep -c "${TAB}ok\$")
        failed=$((total - passed))
        list=$(printf '%s\n' "$selected" | while IFS=$TAB read -r name st; do
            echo "test $name ... $st"
        done)
    fi
    if [ "$total" = 1 ]; then
        echo "running 1 test"
    else
        echo "running $total tests"
    fi
    [ -n "$list" ] && printf '%s\n' "$list"
    if [ "$failed" -gt 0 ]; then
        echo
        echo "failures:"
        printf '%s\n' "$selected" | while IFS=$TAB read -r name st; do
            [ "$st" = FAILED ] && echo "    $name"
        done
        echo
        echo "test result: FAILED. $passed passed; $failed failed; 0 ignored; 0 measured; 0 filtered out$finished"
        echo
        return 1
    fi
    echo
    echo "test result: ok. $passed passed; 0 failed; 0 ignored; 0 measured; 0 filtered out$finished"
    echo
    return 0
}

if in_src "this does not compile"; then
    echo "error: could not compile \`fake\` (lib) due to 1 previous error" >&2
    exit 101
fi

if [ "$1" = --workspace ]; then
    # A `Cargo.lock` with the text `fake: stale lock` is out of date for `--locked` (STP-45).
    if grep -qF -- "fake: stale lock" Cargo.lock 2>/dev/null; then
        echo "error: the lock file needs to be updated but --locked was passed to prevent this" >&2
        echo "because --locked was passed" >&2
        exit 101
    fi
    # The whole workspace: every test file; one that does not compile stops the run.
    for file in crates/*/tests/*.rs; do
        [ -f "$file" ] || continue
        if ! compiles "$file"; then
            name=$(basename "$file" .rs)
            echo "error: could not compile \`fake\` (test \"$name\") due to 1 previous error" >&2
            exit 101
        fi
    done
    # `--no-run`: the build only. A file under `crates/*/src` with the text `fake: build sleeps`
    # makes it sleep a minute first (STP-45).
    for arg in "$@"; do
        if [ "$arg" = --no-run ]; then
            if in_src "fake: build sleeps"; then
                sleep 60
            fi
            echo "    Finished \`test\` profile [unoptimized + debuginfo] target(s) in 0.00s" >&2
            exit 0
        fi
    done
    # Like cargo, stop after the first test file with a failure.
    # Like cargo, the `Running` line of each test binary goes to standard error (STP-42 XR1-1).
    for file in crates/*/tests/*.rs; do
        [ -f "$file" ] || continue
        name=$(basename "$file" .rs)
        echo "     Running tests/$name.rs (target/debug/deps/$name-0000000000000000)" >&2
        run_file "$file" "" || exit 101
    done
    exit 0
fi

# `--locked -p <package> --test <file> -- --exact <names>...`
package=""
target=""
while [ $# -gt 0 ]; do
    case "$1" in
        -p) package=$2; shift 2 ;;
        --test) target=$2; shift 2 ;;
        --) shift; break ;;
        *) shift ;;
    esac
done
case "$1" in --test-threads=*) shift ;; esac
[ "$1" = --exact ] && shift
names=$(printf '%s\n' "$@")

file=""
for manifest in crates/*/Cargo.toml; do
    if grep -qx "name = \"$package\"" "$manifest" 2>/dev/null; then
        file="$(dirname "$manifest")/tests/$target.rs"
    fi
done
if [ ! -f "$file" ]; then
    echo "error: no test target named \`$target\` in package \`$package\`" >&2
    exit 101
fi
if ! compiles "$file"; then
    echo "error: could not compile \`$package\` (test \"$target\") due to 1 previous error" >&2
    exit 101
fi
echo "     Running tests/$target.rs (target/debug/deps/$target-0000000000000000)" >&2
if run_file "$file" "$names"; then
    exit 0
fi
exit 101
