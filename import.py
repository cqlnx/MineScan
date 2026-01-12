import json
import psycopg2
import sys
import signal
import requests
import ijson
from psycopg2.extras import execute_batch
from datetime import datetime, UTC

if len(sys.argv) != 2:
    print("Usage: import.py <results.json>")
    sys.exit(1)

INPUT_FILE = sys.argv[1]

with open("config.json") as f:
    config = json.load(f)

WEBHOOK_URL = config["webhook_url"]
DB_CONFIG = config["db_config"]
BATCH_SIZE = 500
PROGRESS_EVERY = 10_000
total = "input.txt"

UPSERT_SQL = """
INSERT INTO minecraft_servers (
    ip,
    version,
    online_players,
    max_players,
    motd,
    protocol,
    last_seen
)
VALUES (%s,%s,%s,%s,%s,%s,%s)
ON CONFLICT (ip) DO UPDATE SET
    version = EXCLUDED.version,
    online_players = EXCLUDED.online_players,
    max_players = EXCLUDED.max_players,
    motd = EXCLUDED.motd,
    protocol = EXCLUDED.protocol,
    last_seen = EXCLUDED.last_seen;
"""

def clean_text(value):
    if not value:
        return ""
    return value.replace("\x00", "")

def handle_sigterm(sig, frame):
    print("\nSIGTERM received — flushing and exiting safely...")
    if rows:
        execute_batch(cur, UPSERT_SQL, rows)
        conn.commit()
        print(f"Flushed {len(rows)} rows before exit")
    cur.close()
    conn.close()
    sys.exit(0)

signal.signal(signal.SIGTERM, handle_sigterm)

with open(total, "r") as f:
    ip_list = [line.strip() for line in f if line.strip()]
total_ips = len(ip_list)

now = datetime.now(UTC)

payload = {
    "embeds": [
        {
            "author": {"name": "MineScan-System"},
            "title": "🛰️ Scan Status Update",
            "description": "The status of a scan has been updated and will be processed shortly.",
            "color": 0xFFFF00,
            "fields": [
                {"name": "Scan Status", "value": "Importing Data", "inline": False},
                {"name": "Total", "value": f"{total_ips:,}", "inline": False},
                {"name": "Worker", "value": "Codespace-01", "inline": False},
                {"name": "Edition", "value": "Java", "inline": False},
            ],
            "timestamp": now.isoformat()
        }
    ]
}

requests.post(WEBHOOK_URL, json=payload)

conn = psycopg2.connect(**DB_CONFIG)
conn.autocommit = False
cur = conn.cursor()
print("Connected to database")

rows = []
inserted = 0
skipped = 0
seen = 0

def handle_object(obj):
    global skipped

    if not isinstance(obj, dict):
        skipped += 1
        return

    if "error" in obj:
        skipped += 1
        return

    ip = obj.get("ip")
    if not ip:
        skipped += 1
        return

    rows.append((
        ip,
        clean_text(obj.get("version")),
        obj.get("online_players", 0),
        obj.get("max_players", 0),
        clean_text(obj.get("motd")).strip(),
        int(obj.get("protocol", 0)),
        now,
    ))

with open(INPUT_FILE, "rb") as f:
    for obj in ijson.items(f, "item"):
        seen += 1
        handle_object(obj)

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

if rows:
    execute_batch(cur, UPSERT_SQL, rows)
    conn.commit()
    inserted += len(rows)

cur.close()
conn.close()

print("\n=== IMPORT COMPLETE ===")
print(f"JSON values read:       {seen:,}")
print(f"Inserted / Updated:     {inserted:,}")
print(f"Skipped:                {skipped:,}")

payload = {
    "embeds": [
        {
            "author": {"name": "MineScan-System"},
            "title": "🛰️ Scan Status Update",
            "description": "The status of a scan has been updated and will be processed shortly.",
            "color": 0x57F287,
            "fields": [
                {"name": "Scan Status", "value": "Done", "inline": False},
                {"name": "Total", "value": f"Updated {inserted:,} rows", "inline": False},
                {"name": "Worker", "value": "Codespace-01", "inline": False},
                {"name": "Edition", "value": "Java", "inline": False},
            ],
            "timestamp": datetime.now(UTC).isoformat()
        }
    ]
}

requests.post(WEBHOOK_URL, json=payload)
