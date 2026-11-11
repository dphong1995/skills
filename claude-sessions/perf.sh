#!/bin/bash
# Measures the real widget (release build) on a fixture: footprint, CPU and idle wakeups in an idle phase and a load phase.
# Run from anywhere on an otherwise quiet machine; prints a PASS/FAIL table and exits 1 on any FAIL.
# Env: PERF_PHASE_SECONDS (default 60). The fixture lives in .build/perf-fixture (delete it to regenerate).
# Never touches ~/.claude, the Keychain or the installed app: the binary gets CLAUDE_SESSIONS_ROOT.
#
# --ab [--gate]: A/B run of the Swift build against the Rust build (`cargo xtask bundle`) on the same fixture. Each metric is
# the median of AB_ROUNDS (5) interleaved rounds (the first binary alternates per round); prints
# `metric | swift (median) | rust (median) | rule | PASS/FAIL` plus every round's raw numbers, exits 1 on any FAIL and
# refuses to run while another ClaudeSessions process runs. --gate uses .build/perf-fixture-gate (3 tiny sessions:
# waiting, busy, idle) and runs settle + idle only. Rules: Rust footprint <= Swift; CPU <= Swift x 1.1 or within 0.1
# percentage point; idle wakeups <= Swift x 1.1 or within 0.5/s; footprint growth in a phase < 5 MB.

cd "$(dirname "$0")" || exit 1

AB=0
GATE=0
for arg in "$@"; do
    case "$arg" in
        --ab) AB=1 ;;
        --gate) GATE=1 ;;
        *) echo "usage: perf.sh [--ab [--gate]]"; exit 2 ;;
    esac
done
[ "$GATE" -eq 1 ] && [ "$AB" -eq 0 ] && { echo "--gate needs --ab"; exit 2; }

PHASE=${PERF_PHASE_SECONDS:-60}
FOOTPRINT_MAX_MB=100
GROWTH_MAX_MB=5
CPU_IDLE_MAX=1
CPU_LOAD_MAX=2
WAKEUPS_MAX=5
SETTLE_CAP=300
SESSIONS=10
SMALL_MB=50
BIG_MB=230

ROOT="$PWD/.build/perf-fixture"
BIN="$PWD/.build/release/ClaudeSessions"
SWIFT_BIN="$BIN"
RUST_BIN="$PWD/target/bundle/ClaudeSessions.app/Contents/MacOS/ClaudeSessions"
AB_ROUNDS=${AB_ROUNDS:-5}
if [ "$GATE" -eq 1 ]; then
    ROOT="$PWD/.build/perf-fixture-gate"
    SESSIONS=3
fi
APP_PID=""
WRITER_PID=""
SLEEP_PIDS=()

cleanup() {
    trap - EXIT INT TERM
    if [ -n "$WRITER_PID" ]; then
        # Collect the writer's `sleep` first: once the writer is dead its child is reparented and pkill -P cannot find it.
        local kids
        kids=$(pgrep -P "$WRITER_PID")
        kill "$WRITER_PID" 2>/dev/null
        [ -n "$kids" ] && kill $kids 2>/dev/null
    fi
    if [ -n "$APP_PID" ]; then
        kill "$APP_PID" 2>/dev/null
        # `wait` below must not block forever on an app that ignores TERM.
        for _ in $(seq 30); do kill -0 "$APP_PID" 2>/dev/null || break; sleep 0.1; done
        app_alive && kill -9 "$APP_PID" 2>/dev/null
    fi
    [ ${#SLEEP_PIDS[@]} -gt 0 ] && kill "${SLEEP_PIDS[@]}" 2>/dev/null
    wait 2>/dev/null
}
trap cleanup EXIT
trap 'exit 130' INT TERM

if [ "$AB" -eq 1 ]; then
    running=$(pgrep -x ClaudeSessions)
    [ -n "$running" ] && { echo "another ClaudeSessions process is running (pid $running); quit it first"; exit 1; }
fi

echo "== build"
swift build -c release --product ClaudeSessions 2>&1 | grep -v "ld: warning" | tail -2
[ "${PIPESTATUS[0]}" -eq 0 ] && [ -x "$BIN" ] || { echo "build failed"; exit 1; }
if [ "$AB" -eq 1 ]; then
    echo "== build (rust)"
    export PATH="$HOME/.cargo/bin:$PATH"
    cargo xtask bundle > /dev/null && [ -x "$RUST_BIN" ] || { echo "rust build failed"; exit 1; }
fi

load=$(sysctl -n vm.loadavg | awk '{print $2}')
cores=$(sysctl -n hw.ncpu)
awk -v l="$load" -v c="$cores" 'BEGIN{exit !(l>c)}' && echo "WARNING: load average $load > $cores cores; numbers will be noisy"

# ---- fixture -------------------------------------------------------------------------------------------------------
REAL='{"type":"user","message":{"content":"Please fix the failing test in the parser module. See https://acme.atlassian.net/browse/ABC-1 for the details."}}
{"type":"assistant","isSidechain":false,"message":{"content":[{"type":"text","text":"I will look at the parser first and then run the tests. Reading the code. Reading the code. Reading the code."}]}}
{"type":"assistant","isSidechain":false,"message":{"content":[{"type":"tool_use","id":"toolu_01","name":"Bash","input":{"command":"swift test --filter ParserTests"}}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_01"}]}}
{"type":"user","message":{"content":"The PR is https://bitbucket.org/t6engineering/talo6-dct-backend/pull-requests/12 now."}}'
TOOL_LINE='{"type":"assistant","isSidechain":false,"message":{"content":[{"type":"tool_use","id":"toolu_99","name":"Bash","input":{"command":"echo perf"}}]}}'

mkdir -p "$ROOT/projects" "$ROOT/sessions" "$ROOT/tasks" "$ROOT/cwd"
UNIT="$ROOT/unit.jsonl"
if [ "$GATE" -eq 0 ] && [ ! -s "$UNIT" ]; then
    # 1 unit ~1 MB: 100 realistic turns, then lines the scanner skips without a JSON parse.
    FILLER='{"type":"progress","data":"'$(head -c 900 /dev/zero | tr '\0' 'x')'"}'
    { for _ in $(seq 100); do echo "$REAL"; done; for _ in $(seq 1000); do echo "$FILLER"; done; } > "$UNIT.tmp" && mv "$UNIT.tmp" "$UNIT"
fi
[ "$GATE" -eq 0 ] && UNIT_SIZE=$(stat -f %z "$UNIT")

name_of() { echo "perf-$1"; }
transcript_of() { echo "$ROOT/projects/$(echo -n "$ROOT/cwd/$1" | sed 's/[^A-Za-z0-9]/-/g')/$(name_of "$1").jsonl"; }

echo "== fixture ($ROOT)"
for i in $(seq 0 $((SESSIONS - 1))); do
    units=$SMALL_MB; [ "$i" -eq $((SESSIONS - 1)) ] && units=$BIG_MB
    t=$(transcript_of "$i")
    mkdir -p "$(dirname "$t")" "$ROOT/cwd/$i/.git" "$ROOT/tasks/$(name_of "$i")"
    echo "ref: refs/heads/main" > "$ROOT/cwd/$i/.git/HEAD"
    for n in 1 2 3 4 5; do
        echo "{\"id\":\"$n\",\"subject\":\"Task $n\",\"status\":\"pending\",\"blocks\":[],\"blockedBy\":[]}" > "$ROOT/tasks/$(name_of "$i")/$n.json"
    done
    if [ "$GATE" -eq 1 ]; then
        echo "$REAL" > "$t"
    elif [ "$(stat -f %z "$t" 2>/dev/null)" != $((units * UNIT_SIZE)) ]; then
        echo "generating $(basename "$t") ($units MB)"
        for _ in $(seq "$units"); do cat "$UNIT"; done > "$t"
    fi
done

for _ in $(seq "$SESSIONS"); do sleep 100000 & SLEEP_PIDS+=($!); done

# $1 = session index, $2 = status
write_session() {
    local i=$1 status=$2 extra=""
    [ "$status" = waiting ] && [ "$i" -eq $((SESSIONS - 1)) ] && extra=',"waitingFor":"permission prompt"'
    echo "{\"pid\":${SLEEP_PIDS[$i]},\"sessionId\":\"$(name_of "$i")\",\"name\":\"$(name_of "$i")\",\"cwd\":\"$ROOT/cwd/$i\",\"status\":\"$status\",\"statusUpdatedAt\":$(($(date +%s) * 1000))$extra}" \
        > "$ROOT/sessions/$(name_of "$i").json.tmp"
    mv "$ROOT/sessions/$(name_of "$i").json.tmp" "$ROOT/sessions/$(name_of "$i").json"
}
rm -f "$ROOT"/sessions/*.json
for i in $(seq 0 $((SESSIONS - 1))); do
    if [ "$GATE" -eq 1 ]; then
        case "$i" in 0) write_session "$i" waiting ;; 1) write_session "$i" busy ;; *) write_session "$i" idle ;; esac
    elif [ "$i" -ge 7 ]; then write_session "$i" waiting; else write_session "$i" idle; fi
done

# ---- readings ------------------------------------------------------------------------------------------------------
cpu_seconds() { ps -o cputime= -p "$APP_PID" | tr -d ' ' | awk -F: '{ s = 0; for (i = 1; i <= NF; i++) s = s * 60 + $i; printf "%.2f", s }'; }
is_num() { [[ $1 =~ ^-?[0-9]+(\.[0-9]+)?$ ]]; }
app_alive() { kill -0 "$APP_PID" 2>/dev/null; }
footprint_mb() { footprint -f bytes "$APP_PID" 2>/dev/null | awk '{ for (i = 1; i < NF; i++) if ($i == "Footprint:") { printf "%.1f", $(i + 1) / 1048576; exit } }'; }
# top's IDLEW column is cumulative since the process started, so the phase rate is the difference of two `-l 1` readings.
# Prints nothing when the app is gone (the last line is then the header, not a number).
idle_wakeups() { top -l 1 -pid "$APP_PID" -stats pid,idlew 2>/dev/null | tail -1 | awk '$1 ~ /^[0-9]+$/ && $2 ~ /^[0-9]/ { gsub(/[^0-9]/, "", $2); print $2 }'; }

RESULTS=()
FAILED=0
# $1 metric, $2 measured, $3 threshold; a measured value that is not a number (app gone, failed reading) is a FAIL.
record() {
    local verdict=FAIL
    is_num "$2" && awk -v a="$2" -v t="$3" 'BEGIN{exit !(a<t)}' && verdict=PASS
    [ "$verdict" = FAIL ] && FAILED=1
    RESULTS+=("$(printf '%-34s | %10s | < %-8s | %s' "$1" "$2" "$3" "$verdict")")
}

# $1 a, $2 b, $3 scale, $4 printf format: (b - a) * scale, or n/a when a reading is missing.
delta_of() { is_num "$1" && is_num "$2" && awk -v a="$1" -v b="$2" -v k="$3" "BEGIN{printf \"$4\", (b - a) * k}" || echo n/a; }

# $1 = event text; FAILs and returns 1 when the app is gone.
check_alive() {
    app_alive && return 0
    FAILED=1
    RESULTS+=("$(printf '%-34s | %10s | %-10s | FAIL' "app exited ($1)" "-" "-")")
    return 1
}

# $1 = phase name, $2 = CPU threshold, $3 = "wakeups" to also check idle wakeups
measure_phase() {
    local name=$1 cpu_max=$2 cpu0 cpu1 fp0 fp1 w0 w1 a b t0=$SECONDS
    check_alive "$name start" || return
    # Wait for the footprint to stop moving (two readings within 1 MB, cap 30 s) so growth is measured from a settled value.
    a=$(footprint_mb)
    while [ $((SECONDS - t0)) -lt 30 ] && app_alive; do
        sleep 1
        b=$(footprint_mb)
        is_num "$a" && is_num "$b" && awk -v a="$a" -v b="$b" 'BEGIN{d = b - a; exit !(d < 1 && d > -1)}' && break
        a=$b
    done
    cpu0=$(cpu_seconds); fp0=$(footprint_mb); w0=$(idle_wakeups)
    # Background sleep + wait so a signal is handled at once instead of after the sleep.
    sleep "$PHASE" & SLEEP_PIDS+=($!)
    wait $!
    cpu1=$(cpu_seconds); fp1=$(footprint_mb); w1=$(idle_wakeups)
    check_alive "$name end"
    RAW_FP=$fp1
    RAW_GROWTH=$(delta_of "$fp0" "$fp1" 1 %.1f)
    RAW_CPU=$(delta_of "$cpu0" "$cpu1" "$(awk -v s="$PHASE" 'BEGIN{print 100 / s}')" %.2f)
    RAW_WAKE=n/a
    [ "$3" = wakeups ] && RAW_WAKE=$(delta_of "$w0" "$w1" "$(awk -v s="$PHASE" 'BEGIN{print 1 / s}')" %.2f)
    record "$name footprint end (MB)" "$fp1" "$FOOTPRINT_MAX_MB"
    record "$name footprint growth (MB)" "$(delta_of "$fp0" "$fp1" 1 %.1f)" "$GROWTH_MAX_MB"
    record "$name CPU (%)" "$(delta_of "$cpu0" "$cpu1" "$(awk -v s="$PHASE" 'BEGIN{print 100 / s}')" %.2f)" "$cpu_max"
    [ "$3" = wakeups ] && record "$name idle wakeups (/s)" "$(delta_of "$w0" "$w1" "$(awk -v s="$PHASE" 'BEGIN{print 1 / s}')" %.2f)" "$WAKEUPS_MAX"
}

# ---- run -----------------------------------------------------------------------------------------------------------
# Starts $BIN on the fixture and waits until its CPU time stops rising; sets SETTLE_SECONDS, FAILs when it never calms down.
start_and_settle() {
    echo "== start app"
    CLAUDE_SESSIONS_ROOT="$ROOT" "$BIN" > /dev/null 2>&1 &
    APP_PID=$!
    echo "app pid: $APP_PID (sample it with: sample $APP_PID 10)"

    echo "== settle (cputime rising < 0.05 s for 5 consecutive 1 s samples, cap ${SETTLE_CAP} s)"
    local settle_start=$SECONDS cur
    quiet=0
    local prev
    prev=$(cpu_seconds)
    while [ "$quiet" -lt 5 ] && [ $((SECONDS - settle_start)) -lt "$SETTLE_CAP" ]; do
        sleep 1
        app_alive || break
        cur=$(cpu_seconds)
        if is_num "$prev" && is_num "$cur" && awk -v a="$prev" -v b="$cur" 'BEGIN{exit !(b - a < 0.05)}'; then quiet=$((quiet + 1)); else quiet=0; fi
        prev=$cur
    done
    settle=$((SECONDS - settle_start))
    check_alive settle
    if [ "$quiet" -lt 5 ]; then
        FAILED=1
        RESULTS+=("$(printf '%-34s | %10s | < %-8s | FAIL (not stable)' "settle (s)" "$settle" "$SETTLE_CAP")")
    fi
}

stop_app() {
    kill "$APP_PID" 2>/dev/null
    for _ in $(seq 30); do kill -0 "$APP_PID" 2>/dev/null || break; sleep 0.1; done
    app_alive && kill -9 "$APP_PID" 2>/dev/null
    wait "$APP_PID" 2>/dev/null
    APP_PID=""
}

# $1 = a list of numbers on separate lines; prints the median.
median() { sort -n | awk '{ v[NR] = $1 } END { if (NR % 2) print v[(NR + 1) / 2]; else printf "%.2f", (v[NR / 2] + v[NR / 2 + 1]) / 2 }'; }

# $1 swift median, $2 rust median, $3 mode (<= | x1.1+0.1 | x1.1+0.5 | lt5); prints PASS or FAIL.
ab_verdict() {
    is_num "$1" && is_num "$2" || { echo FAIL; return; }
    awk -v s="$1" -v r="$2" -v m="$3" 'BEGIN {
        if (m == "<=") ok = (r <= s)
        else if (m == "x1.1+0.1") ok = (r <= s * 1.1 || r <= s + 0.1)
        else if (m == "x1.1+0.5") ok = (r <= s * 1.1 || r <= s + 0.5)
        else ok = (r < 5)
        print ok ? "PASS" : "FAIL" }'
}

# A/B: AB_ROUNDS interleaved rounds, the first binary alternating; raw numbers go to AB_<app>_<metric> arrays.
ab_main() {
    local round app first
    AB_ROWS=()
    AB_SETTLE=()
    AB_UNSETTLED=0
    for round in $(seq 1 "$AB_ROUNDS"); do
        if [ $((round % 2)) -eq 1 ]; then first=swift; else first=rust; fi
        for app in "$first" "$([ "$first" = swift ] && echo rust || echo swift)"; do
            echo "=== round $round: $app"
            if [ "$app" = swift ]; then BIN=$SWIFT_BIN; else BIN=$RUST_BIN; fi
            RESULTS=(); FAILED=0
            start_and_settle
            AB_SETTLE+=("$round $app ${settle}")
            if [ "$FAILED" -ne 0 ]; then
                # An unsettled or dead app must not feed the medians: fail the verdict and skip this round's numbers.
                AB_UNSETTLED=1
                printf '%s\n' "${RESULTS[@]}"
                stop_app
                continue
            fi
            RAW_FP=""; RAW_GROWTH=""; RAW_CPU=""; RAW_WAKE=""
            measure_phase idle "$CPU_IDLE_MAX" wakeups
            # An app that died mid-phase leaves no numbers: fail the verdict instead of shifting columns in the medians.
            if ! app_alive || ! is_num "$RAW_FP" || ! is_num "$RAW_GROWTH" || ! is_num "$RAW_CPU" || ! is_num "$RAW_WAKE"; then
                AB_UNSETTLED=1
                printf '%s\n' "${RESULTS[@]}"
                stop_app
                continue
            fi
            AB_ROWS+=("$round $app idle $RAW_FP $RAW_GROWTH $RAW_CPU $RAW_WAKE")
            if [ "$GATE" -eq 0 ]; then
                for i in 0 1 2; do write_session "$i" busy; done
                ( while true; do for i in 0 1 2; do echo "$TOOL_LINE" >> "$(transcript_of "$i")"; done; sleep 0.5; done ) 2>/dev/null &
                WRITER_PID=$!
                RAW_FP=""; RAW_GROWTH=""; RAW_CPU=""
                measure_phase load "$CPU_LOAD_MAX"
                if ! app_alive || ! is_num "$RAW_FP" || ! is_num "$RAW_GROWTH" || ! is_num "$RAW_CPU"; then
                    AB_UNSETTLED=1
                else
                    AB_ROWS+=("$round $app load $RAW_FP $RAW_GROWTH $RAW_CPU n/a")
                fi
                kill "$WRITER_PID" 2>/dev/null; WRITER_PID=""
                for i in 0 1 2; do write_session "$i" idle; done
            fi
            stop_app
        done
    done
    local phases="idle"; [ "$GATE" -eq 0 ] && phases="idle load"
    local ab_failed=$AB_UNSETTLED phase metric col mode swift_m rust_m verdict
    echo
    printf '%-26s | %14s | %14s | %-18s | %s\n' metric "swift (median)" "rust (median)" rule result
    for phase in $phases; do
        for metric in "footprint end (MB):4:<=:rust <= swift" "footprint growth (MB):5:lt5:rust < 5" "CPU (%):6:x1.1+0.1:rust <= x1.1 or +0.1" "idle wakeups (/s):7:x1.1+0.5:rust <= x1.1 or +0.5"; do
            IFS=: read -r label col mode rule <<< "$metric"
            [ "$col" = 7 ] && [ "$phase" = load ] && continue
            swift_m=$(printf '%s\n' "${AB_ROWS[@]}" | awk -v p="$phase" -v c="$col" '$2 == "swift" && $3 == p { print $c }' | median)
            rust_m=$(printf '%s\n' "${AB_ROWS[@]}" | awk -v p="$phase" -v c="$col" '$2 == "rust" && $3 == p { print $c }' | median)
            verdict=$(ab_verdict "$swift_m" "$rust_m" "$mode")
            [ "$verdict" = FAIL ] && ab_failed=1
            printf '%-26s | %14s | %14s | %-18s | %s\n' "$phase $label" "$swift_m" "$rust_m" "$rule" "$verdict"
        done
    done
    echo
    echo "raw rounds: round app phase footprint_mb growth_mb cpu_pct wakeups_per_s"
    printf '%s\n' "${AB_ROWS[@]}"
    echo
    echo "settle time (s, no rule): round app seconds"
    printf '%s\n' "${AB_SETTLE[@]}"
    for app in swift rust; do
        echo "settle median $app: $(printf '%s\n' "${AB_SETTLE[@]}" | awk -v a="$app" '$2 == a { print $3 }' | median) s"
    done
    return "$ab_failed"
}

if [ "$AB" -eq 1 ]; then
    ab_main
    exit $?
fi

start_and_settle

echo "== idle phase (${PHASE} s)"
measure_phase idle "$CPU_IDLE_MAX" wakeups

echo "== load phase (${PHASE} s)"
# The writer appends to transcripts 0-2, so their size no longer matches and the next run regenerates them.
for i in 0 1 2; do write_session "$i" busy; done
(
    while true; do
        for i in 0 1 2; do echo "$TOOL_LINE" >> "$(transcript_of "$i")"; done
        sleep 0.5
    done
) 2>/dev/null &
WRITER_PID=$!
measure_phase load "$CPU_LOAD_MAX"

echo
echo "settle time: ${settle} s (no threshold)"
printf '%-34s | %10s | %-10s | %s\n' metric measured threshold result
for r in "${RESULTS[@]}"; do echo "$r"; done
exit "$FAILED"
