import json
import psycopg2
import sys
from psycopg2.extras import execute_batch
from datetime import datetime, UTC

if len(sys.argv) != 2:
    print("Usage: import_auth.py <results.json>")
    sys.exit(1)

with open("config.json") as f:
    config = json.load(f)

DB_CONFIG = config["db_config"]
INPUT_FILE = sys.argv[1]
BATCH_SIZE = 2000
PROGRESS_EVERY = 10000

UPSERT_SQL = """
INSERT INTO minecraft_servers (
    ip,
    version,
    online_players,
    max_players,
    motd,
    protocol,
    auth_mode,
    last_seen
)
VALUES (%s,%s,%s,%s,%s,%s,%s,%s)
ON CONFLICT (ip) DO UPDATE SET
    version = EXCLUDED.version,
    online_players = EXCLUDED.online_players,
    max_players = EXCLUDED.max_players,
    motd = EXCLUDED.motd,
    protocol = EXCLUDED.protocol,
    auth_mode = EXCLUDED.auth_mode,
    last_seen = EXCLUDED.last_seen;
"""

try:
    with open("mc.txt", "r") as f:
        total_ips = len([line for line in f if line.strip()])
except:
    total_ips = 0

now = datetime.now(UTC)

conn = psycopg2.connect(**DB_CONFIG)
cur = conn.cursor()
print("Connected to database")

error_stats = {}

def handle_object(obj):
    global rows, skipped

    if not isinstance(obj, dict):
        skipped += 1
        return

    if "error" in obj:
        error_msg = obj.get("error", "unknown")
        error_stats[error_msg] = error_stats.get(error_msg, 0) + 1
        skipped += 1
        return

    ip = obj.get("ip")
    if not ip:
        skipped += 1
        return

    auth_mode = obj.get("auth_mode")
    if auth_mode is None:
        auth_mode = -1

    motd = (obj.get("motd") or "").strip()
    motd = motd.replace('\x00', '')

    version = obj.get("version") or ""
    if version:
        version = version.replace('\x00', '')

    rows.append((
        ip,
        version if version else None,
        obj.get("online_players", 0),
        obj.get("max_players", 0),
        motd,
        int(obj.get("protocol", 0)),
        auth_mode,
        now,
    ))

with open(INPUT_FILE, "r", encoding="utf-8") as f:
    data = f.read()

decoder = json.JSONDecoder()
idx = 0
length = len(data)

rows = []
inserted = 0
skipped = 0
seen = 0

while idx < length:
    try:
        value, next_idx = decoder.raw_decode(data, idx)
        idx = next_idx
        seen += 1

        if isinstance(value, list):
            for item in value:
                handle_object(item)
        else:
            handle_object(value)

        if len(rows) >= BATCH_SIZE:
            execute_batch(cur, UPSERT_SQL, rows)
            conn.commit()
            inserted += len(rows)
            rows.clear()

            if inserted % PROGRESS_EVERY == 0:
                print(
                    f"Seen: {seen:,} | "
                    f"Inserted: {inserted:,} | "
                    f"Skipped: {skipped:,}"
                )

    except json.JSONDecodeError:
        idx += 1
        skipped += 1

if rows:
    execute_batch(cur, UPSERT_SQL, rows)
    conn.commit()
    inserted += len(rows)

cur.close()
conn.close()

print("\n=== AUTH IMPORT COMPLETE ===")
print(f"JSON values read:       {seen:,}")
print(f"Inserted / Updated:     {inserted:,}")
print(f"Skipped:                {skipped:,}")

#if error_stats:
#   print("\n📊 Skip Reasons:")
#    for error, count in sorted(error_stats.items(), key=lambda x: x[1], reverse=True):
#        print(f"   {count:,}x - {error}")
# optional do print out skip reasons
# gives you data on scan timeouts and other errors that u could fix do find more servers