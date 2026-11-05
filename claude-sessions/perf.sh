#!/bin/bash
# Measures the real widget (release build) on a fixture: footprint, CPU and idle wakeups in an idle phase and a load phase.
# Run from anywhere on an otherwise quiet machine; prints a PASS/FAIL table and exits 1 on any FAIL.
# Env: PERF_PHASE_SECONDS (default 60). The fixture lives in .build/perf-fixture (delete it to regenerate).
# Never touches ~/.claude, the Keychain or the installed app: the binary gets CLAUDE_SESSIONS_ROOT.

cd "$(dirname "$0")" || exit 1

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
APP_PID=""
WRITER_PID=""
SLEEP_PIDS=()

cleanup() {
    trap - EXIT INT TERM
    [ -n "$WRITER_PID" ] && { pkill -P "$WRITER_PID" 2>/dev/null; kill "$WRITER_PID" 2>/dev/null; }
    [ -n "$APP_PID" ] && kill "$APP_PID" 2>/dev/null
    [ ${#SLEEP_PIDS[@]} -gt 0 ] && kill "${SLEEP_PIDS[@]}" 2>/dev/null
    wait 2>/dev/null
}
trap cleanup EXIT
trap 'exit 130' INT TERM

echo "== build"
swift build -c release --product ClaudeSessions 2>&1 | grep -v "ld: warning" | tail -2
[ -x "$BIN" ] || { echo "build failed"; exit 1; }

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
if [ ! -s "$UNIT" ]; then
    # 1 unit ~1 MB: 100 realistic turns, then lines the scanner skips without a JSON parse.
    FILLER='{"type":"progress","data":"'$(head -c 900 /dev/zero | tr '\0' 'x')'"}'
    { for _ in $(seq 100); do echo "$REAL"; done; for _ in $(seq 1000); do echo "$FILLER"; done; } > "$UNIT"
fi
UNIT_SIZE=$(stat -f %z "$UNIT")

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
    if [ "$(stat -f %z "$t" 2>/dev/null)" != $((units * UNIT_SIZE)) ]; then
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
    if [ "$i" -ge 7 ]; then write_session "$i" waiting; else write_session "$i" idle; fi
done

# ---- readings ------------------------------------------------------------------------------------------------------
cpu_seconds() { ps -o cputime= -p "$APP_PID" | tr -d ' ' | awk -F: '{ s = 0; for (i = 1; i <= NF; i++) s = s * 60 + $i; printf "%.2f", s }'; }
footprint_mb() { footprint -f bytes "$APP_PID" 2>/dev/null | awk '{ for (i = 1; i < NF; i++) if ($i == "Footprint:") { printf "%.1f", $(i + 1) / 1048576; exit } }'; }
idle_wakeups() { top -l 1 -pid "$APP_PID" -stats pid,idlew 2>/dev/null | tail -1 | awk '{ gsub(/[^0-9]/, "", $2); print $2 + 0 }'; }

RESULTS=()
FAILED=0
# $1 metric, $2 measured, $3 threshold, $4 comparison direction text ("<")
record() {
    local verdict=FAIL
    awk -v a="$2" -v t="$3" 'BEGIN{exit !(a<t)}' && verdict=PASS
    [ "$verdict" = FAIL ] && FAILED=1
    RESULTS+=("$(printf '%-34s | %10s | < %-8s | %s' "$1" "$2" "$3" "$verdict")")
}

# $1 = phase name, $2 = CPU threshold, $3 = "wakeups" to also check idle wakeups
measure_phase() {
    local name=$1 cpu_max=$2 cpu0 cpu1 fp0 fp1 w0 w1
    cpu0=$(cpu_seconds); fp0=$(footprint_mb); w0=$(idle_wakeups)
    sleep "$PHASE"
    cpu1=$(cpu_seconds); fp1=$(footprint_mb); w1=$(idle_wakeups)
    record "$name footprint end (MB)" "$fp1" "$FOOTPRINT_MAX_MB"
    record "$name footprint growth (MB)" "$(awk -v a="$fp0" -v b="$fp1" 'BEGIN{printf "%.1f", b - a}')" "$GROWTH_MAX_MB"
    record "$name CPU (%)" "$(awk -v a="$cpu0" -v b="$cpu1" -v s="$PHASE" 'BEGIN{printf "%.2f", (b - a) / s * 100}')" "$cpu_max"
    [ "$3" = wakeups ] && record "$name idle wakeups (/s)" "$(awk -v a="$w0" -v b="$w1" -v s="$PHASE" 'BEGIN{printf "%.2f", (b - a) / s}')" "$WAKEUPS_MAX"
}

# ---- run -----------------------------------------------------------------------------------------------------------
echo "== start app"
CLAUDE_SESSIONS_ROOT="$ROOT" "$BIN" > /dev/null 2>&1 &
APP_PID=$!
echo "app pid: $APP_PID (sample it with: sample $APP_PID 10)"

echo "== settle (cputime rising < 0.05 s for 5 consecutive 1 s samples, cap ${SETTLE_CAP} s)"
settle_start=$SECONDS
quiet=0
prev=$(cpu_seconds)
while [ "$quiet" -lt 5 ] && [ $((SECONDS - settle_start)) -lt "$SETTLE_CAP" ]; do
    sleep 1
    cur=$(cpu_seconds)
    if awk -v a="$prev" -v b="$cur" 'BEGIN{exit !(b - a < 0.05)}'; then quiet=$((quiet + 1)); else quiet=0; fi
    prev=$cur
done
settle=$((SECONDS - settle_start))
if [ "$quiet" -lt 5 ]; then
    FAILED=1
    RESULTS+=("$(printf '%-34s | %10s | < %-8s | FAIL (not stable)' "settle (s)" "$settle" "$SETTLE_CAP")")
fi

echo "== idle phase (${PHASE} s)"
measure_phase idle "$CPU_IDLE_MAX" wakeups

echo "== load phase (${PHASE} s)"
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
