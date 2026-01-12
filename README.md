MineScan (Pipeline Repo)
================================

⚠️ WARNING / READ FIRST
----------------------
This repository is NOT plug-and-play.

In its current form, it mostly works **only for me**.

Why?
- It fetches IPs from my own API
- That API already contains a massive list of known Minecraft servers
- New users will likely scan **nothing**, because most IPs are already known

You are expected to modify this project if you want to use it yourself.

What This Repo Is
-----------------
This is a full scanning *pipeline* around Minecraft server probing:
- IP collection
- Fast ping detection
- Deep probing
- Database importing
- Discord webhook updates

It is NOT just a scanner.

Required API
------------
This pipeline depends on this API:
https://mcapi.shit.vc/docs

The API stores already-known Minecraft servers and is used to avoid rescanning them with auth-mode detection on.
Without replacing or modifying this logic, results will be very limited.

Required File: ips.txt
----------------------
You MUST provide an `ips.txt` file.

Format (one per line):
1.2.3.4
2.5.6.7

If `ips.txt` is missing or empty, scanning will fail.

Main Components
---------------
- start.sh              → runs everything
- fetch_api_ips.py      → pulls IPs from the API
- mcping (Rust)         → fast Minecraft detection
- mcprobe / mcprobe_auth (Rust) → detailed scanning
- import.py             → DB import
- import_auth.py        → DB import with auth mode
- config.json           → database + webhook config

Important Credit
----------------
The actual Minecraft scanning logic comes from:

https://github.com/cqlnx/mcprobe

That repository contains ONLY the scanning code (`mcprobe.rs`).
This repo adds databases, APIs, filtering, and automation on top.

Community
---------
Discord:
https://discord.gg/AYbDNEWgHE

Disclaimer
----------
For research and data collection only.
Do not scan networks you do not own or have permission to test.
