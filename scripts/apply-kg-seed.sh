#!/usr/bin/env bash
# Apply scripts/seed-knowledge-graph.cypher to FalkorDB.
# Splits on ';' and runs each statement via GRAPH.QUERY, preserving
# quoted string literals.
set -euo pipefail

GRAPH="${OCTOPUS_KG_GRAPH:-octopus_kg}"
CONTAINER="${OCTOPUS_FALKOR_CONTAINER:-zeroclaw-falkordb}"
SEED="${1:-scripts/seed-knowledge-graph.cypher}"

if [[ ! -f "$SEED" ]]; then
  echo "ERR: seed file not found at $SEED"; exit 1
fi
if ! docker exec "$CONTAINER" redis-cli PING >/dev/null 2>&1; then
  echo "ERR: cannot reach falkordb container '$CONTAINER'"; exit 1
fi

echo "Applying $SEED → graph '$GRAPH' (container '$CONTAINER')"

# Use python: strips // comments, splits on ';', preserves quotes.
PY_SPLIT=$(cat <<'PYEOF'
import re, sys
raw = sys.stdin.read()
# strip // line comments
raw = re.sub(r'//[^\n]*', '', raw)
# split on ';' but not inside string literals
out = []
buf = ''
in_str = False
str_char = None
for ch in raw:
    if in_str:
        buf += ch
        if ch == str_char:
            in_str = False
    else:
        if ch in ('"', "'"):
            buf += ch
            in_str = True
            str_char = ch
        elif ch == ';':
            s = ' '.join(buf.split())
            if s:
                out.append(s)
            buf = ''
        else:
            buf += ch
s = ' '.join(buf.split())
if s:
    out.append(s)
for s in out:
    print(s)
PYEOF
)

ok=0; fail=0; total=0
while IFS= read -r stmt; do
  [[ -z "$stmt" ]] && continue
  total=$((total + 1))
  # Pipe statement to redis-cli stdin to avoid shell quoting hell.
  resp=$(printf 'GRAPH.QUERY %s "%s"\n' "$GRAPH" "$(echo "$stmt" | sed 's/"/\\"/g')" \
    | docker exec -i "$CONTAINER" redis-cli 2>&1 || echo "FAIL")
  if echo "$resp" | grep -qiE "^err|errMsg|FAIL"; then
    fail=$((fail + 1))
    printf '  ✗ %3d %s\n     resp: %s\n' "$total" "$(echo "$stmt" | head -c 100)" "$(echo "$resp" | head -c 250)"
  else
    ok=$((ok + 1))
    nodes=$(echo "$resp" | grep -oE "Nodes created: [0-9]+" | grep -oE "[0-9]+" || echo "0")
    edges=$(echo "$resp" | grep -oE "Relationships created: [0-9]+" | grep -oE "[0-9]+" || echo "0")
    printf '  ✓ %3d  +%s nodes  +%s edges  %s\n' "$total" "$nodes" "$edges" "$(echo "$stmt" | head -c 80)"
  fi
done < <(python -c "$PY_SPLIT" < "$SEED")

echo
echo "════════════════════════════════════"
echo "  applied: $ok / $total"
[[ $fail -gt 0 ]] && echo "  failed:  $fail"
echo "════════════════════════════════════"
[[ $fail -gt 0 ]] && exit 2 || exit 0
