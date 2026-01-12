import requests
import json
from datetime import datetime

with open("config.json") as f:
    config = json.load(f)

WEBHOOK_URL = config["webhook_url"]
INPUT_FILE = "input.txt"

with open(INPUT_FILE, "r") as f:
    ip_list = [line.strip() for line in f if line.strip()]
total_ips = len(ip_list)

now = datetime.utcnow()
scan_id = hex(int(now.timestamp()))[2:]

payload = {
    "embeds": [
        {
            "author": {
                "name": "MineScan-System"
            },
            "title": "🛰️ Scan Status Update",
            "description": "The status of a scan has been updated and will be processed shortly.",
            "color": 0xFFFFFF,
            "fields": [
                {
                    "name": "Scan Status",
                    "value": "Processing",
                    "inline": False
                },
                {
                    "name": "Total",
                    "value": f"{total_ips:,}",
                    "inline": False
                },
                {
                    "name": "Worker",
                    "value": "Codespace-01",
                    "inline": False
                },
                {
                    "name": "Edition",
                    "value": "Java",
                    "inline": False
                },
            ],
            "timestamp": now.isoformat()
        }
    ]
}

response = requests.post(WEBHOOK_URL, json=payload)

if response.status_code == 204:
    print("Message sent successfully!")
else:
    print("Failed to send message:", response.text)
