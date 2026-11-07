#!/usr/bin/env bash
# Measures one desktop app on Linux under X11: time until its window is mapped, then, after it
# settles, memory (PSS summed over its whole process tree, so WebKitGTK's web and network
# processes count) and CPU while idle. Needs xwininfo (x11-utils).
#
#   measure-linux.sh <label> <window title> <settle seconds> <idle seconds> -- <command...>
#
# Prints one line: label, window_ms, pss_mib, rss_mib, idle_cpu_percent, processes.
set -euo pipefail
label=$1 title=$2 settle=$3 idle=$4
shift 4
[ "$1" = "--" ] && shift

tree() { # the pid and all its descendants
  local pid=$1
  echo "$pid"
  for child in $(cat /proc/"$pid"/task/*/children 2>/dev/null); do tree "$child"; done
}
ticks() { # user+system clock ticks of a process tree
  local sum=0
  for p in $(tree "$1"); do
    [ -r /proc/"$p"/stat ] || continue
    read -r -a f < <(sed 's/.*) //' /proc/"$p"/stat)
    sum=$((sum + f[11] + f[12]))
  done
  echo "$sum"
}
memory() { # PSS and RSS in KiB over a process tree
  local pss=0 rss=0
  for p in $(tree "$1"); do
    [ -r /proc/"$p"/smaps_rollup ] || continue
    pss=$((pss + $(awk '/^Pss:/ {print $2}' /proc/"$p"/smaps_rollup)))
    rss=$((rss + $(awk '/^Rss:/ {print $2}' /proc/"$p"/smaps_rollup)))
  done
  echo "$pss $rss"
}

start=$(date +%s%N)
"$@" >/tmp/measure-"$label".log 2>&1 &
pid=$!
window_ms=timeout
for _ in $(seq 1 3000); do # up to 60 s
  if xwininfo -root -tree 2>/dev/null | grep -qF "\"$title\""; then
    window_ms=$((($(date +%s%N) - start) / 1000000))
    break
  fi
  kill -0 "$pid" 2>/dev/null || break
  sleep 0.02
done
sleep "$settle"
hz=$(getconf CLK_TCK)
t0=$(ticks "$pid")
sleep "$idle"
t1=$(ticks "$pid")
cpu=$(awk -v d=$((t1 - t0)) -v hz="$hz" -v s="$idle" 'BEGIN { printf "%.1f", d / hz / s * 100 }')
read -r pss rss < <(memory "$pid")
procs=$(tree "$pid" | wc -l)
kill "$pid" 2>/dev/null || true
wait "$pid" 2>/dev/null || true
printf '%s window_ms=%s pss_mib=%.1f rss_mib=%.1f idle_cpu=%s%% processes=%s\n' \
  "$label" "$window_ms" "$(echo "$pss / 1024" | bc -l)" "$(echo "$rss / 1024" | bc -l)" "$cpu" "$procs"
