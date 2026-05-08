#!/usr/bin/env bash
# ingest-library.sh — Chunk + ingest workspace library books into Qdrant
# via the daemon's /api/memory endpoint. Uses memory_store under the hood
# so the daemon does the embedding (text-embedding-3-small at the time of
# writing).
#
# Usage:
#   ./scripts/ingest-library.sh             # ingests workspace/library/**/*.txt
#   ./scripts/ingest-library.sh path1 ...   # ingests specific files
#
# Env:
#   OCTOPUS_GATEWAY_URL  default http://127.0.0.1:42617
#   OCTOPUS_TOKEN_FILE   default /tmp/octopus.token (one bearer per line)
#   OCTOPUS_CATEGORY     default "library" — Qdrant payload category
#   CHUNK_CHARS          default 1500
#   CHUNK_OVERLAP        default 200

set -euo pipefail

GATEWAY="${OCTOPUS_GATEWAY_URL:-http://127.0.0.1:42617}"
TOKEN_FILE="${OCTOPUS_TOKEN_FILE:-/tmp/octopus.token}"
CATEGORY="${OCTOPUS_CATEGORY:-library}"
CHUNK_CHARS="${CHUNK_CHARS:-1500}"
CHUNK_OVERLAP="${CHUNK_OVERLAP:-200}"

if [[ ! -f "$TOKEN_FILE" ]]; then
  echo "ERR: token file not found at $TOKEN_FILE"
  echo
  echo "Get one with:"
  echo "  CODE=\$(curl -s $GATEWAY/admin/paircode | jq -r .pairing_code)"
  echo "  curl -X POST $GATEWAY/pair -H \"X-Pairing-Code: \$CODE\" \\"
  echo "    -H 'Content-Type: application/json' \\"
  echo "    -d '{\"device_name\":\"ingest\"}' | jq -r .token > $TOKEN_FILE"
  exit 1
fi

TOKEN=$(head -n1 "$TOKEN_FILE" | tr -d '[:space:]')
if [[ -z "$TOKEN" ]]; then
  echo "ERR: token file is empty"; exit 1
fi

# Resolve files: cli args, or all .txt under workspace/library.
if [[ $# -gt 0 ]]; then
  FILES=("$@")
else
  WORKSPACE="${OCTOPUS_WORKSPACE:-$HOME/.zeroclaw/workspace}"
  mapfile -t FILES < <(find "$WORKSPACE/library" -type f \( -name '*.txt' -o -name '*.md' \) | sort)
fi

if [[ ${#FILES[@]} -eq 0 ]]; then
  echo "ERR: no library files found"; exit 1
fi

echo "Ingesting ${#FILES[@]} file(s) → $GATEWAY (category=$CATEGORY)"

# Use python for chunking — robust unicode handling, deterministic splits.
PY_CHUNK=$(cat <<'PYEOF'
import os, sys, json, re, hashlib

path = sys.argv[1]
chunk_chars = int(sys.argv[2])
overlap = int(sys.argv[3])
slug = os.path.splitext(os.path.basename(path))[0]

with open(path, 'r', encoding='utf-8', errors='replace') as f:
    raw = f.read()

# Drop Project Gutenberg header/footer and form feeds
raw = re.sub(r'\f', '\n', raw)
raw = re.sub(r'^\*\*\*\s*START OF.*?\*\*\*', '', raw, flags=re.S | re.I)
raw = re.sub(r'\*\*\*\s*END OF.*?$', '', raw, flags=re.S | re.I)
# Collapse 3+ blank lines, trim trailing whitespace
raw = re.sub(r'\n{3,}', '\n\n', raw).strip()

# Split on paragraph breaks first; pack until chunk_chars; carry overlap.
paragraphs = [p.strip() for p in re.split(r'\n\s*\n', raw) if p.strip()]
chunks = []
buf = ''
for p in paragraphs:
    if len(buf) + len(p) + 2 <= chunk_chars:
        buf = (buf + '\n\n' + p) if buf else p
    else:
        if buf:
            chunks.append(buf)
        # Long single paragraph: hard-split
        if len(p) > chunk_chars:
            for i in range(0, len(p), chunk_chars - overlap):
                chunks.append(p[i:i + chunk_chars])
            buf = ''
        else:
            buf = p
if buf:
    chunks.append(buf)

# Stamp each chunk with a deterministic key
out = []
for i, c in enumerate(chunks):
    h = hashlib.sha1(c.encode('utf-8')).hexdigest()[:8]
    out.append({
        'key': f'lib_{slug}_{i:04d}_{h}',
        'content': c,
        'source': slug,
        'chunk_index': i,
        'chunk_total': len(chunks),
    })
print(json.dumps(out))
PYEOF
)

ok=0
fail=0
total=0
for src in "${FILES[@]}"; do
  if [[ ! -f "$src" ]]; then
    echo "SKIP missing: $src"; continue
  fi
  echo
  echo "═══ $(basename "$src") ═══"
  CHUNKS_JSON=$(python -c "$PY_CHUNK" "$src" "$CHUNK_CHARS" "$CHUNK_OVERLAP")
  count=$(echo "$CHUNKS_JSON" | python -c 'import json,sys; print(len(json.load(sys.stdin)))')
  echo "  chunks: $count"

  # Iterate chunks and POST one by one — slow but visible. The
  # daemon enqueues embeddings per request.
  i=0
  while IFS= read -r chunk_obj; do
    i=$((i + 1))
    total=$((total + 1))
    body=$(echo "$chunk_obj" | python -c 'import json, sys; o = json.load(sys.stdin); print(json.dumps({"key": o["key"], "content": o["content"], "category": "'"$CATEGORY"'"}))')
    resp=$(curl -s -o /tmp/octopus.last.json -w '%{http_code}' \
      -X POST "$GATEWAY/api/memory" \
      -H "Authorization: Bearer $TOKEN" \
      -H "Content-Type: application/json" \
      -d "$body")
    if [[ "$resp" == "200" || "$resp" == "201" ]]; then
      ok=$((ok + 1))
      printf '  %4d/%d ✓\r' "$i" "$count"
    else
      fail=$((fail + 1))
      printf '\n  %4d/%d ✗ HTTP %s — %s\n' "$i" "$count" "$resp" "$(head -c 200 /tmp/octopus.last.json)"
    fi
  done < <(echo "$CHUNKS_JSON" | python -c 'import json,sys; [print(json.dumps(c)) for c in json.load(sys.stdin)]')
  echo
done

echo
echo "════════════════════════════════════"
echo "  total chunks: $total"
echo "  ingested:     $ok"
echo "  failed:       $fail"
echo "════════════════════════════════════"
