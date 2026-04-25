# MineScan

Minecraft server discovery & analytics system.

MineScan is a personal project that grew into a continuously running system for discovering and indexing publicly reachable Minecraft servers. It started as something built during winter break and has evolved over time into a larger data collection and analysis pipeline.

It is built around two main components:
- A discovery scanner that finds new servers
- A live system that continuously updates and maintains server data

---

## Links

- Website: https://minescan.xyz  
- API: https://api.minescan.xyz  
- Discord: https://discord.gg/AYbDNEWgHE  

---

## What it does

MineScan collects publicly available Minecraft server data and organizes it into a structured database.

This includes:
- Server version and software
- MOTD (server description)
- Player counts and sample data
- Basic server behavior classification (online-mode, offline-mode, whitelist)

A separate live system continuously updates existing entries and tracks changes over time.

---

## Scanning system

New servers are discovered through a scanning pipeline that:
- Detects reachable Minecraft servers
- Reads publicly exposed server status data
- Performs a lightweight protocol handshake, including a minimal login probe (using `popiiumaa`), to classify authentication mode for unknown servers (online-mode, offline-mode, or whitelist)

Unknown servers are always processed through this initial scanning pipeline before being moved into the live tracking system.

Once a server is known, it is tracked by a separate live system that periodically re-pings servers to keep data up to date (e.g. version changes, player activity, and status updates).

---

## About "popiiumaa"

The identifier `popiiumaa` appears in server logs because it is used as the username during a minimal Minecraft login handshake.

This handshake is part of a lightweight probe used to determine how a server responds (for example: online-mode, offline-mode, or whitelist behavior).

No gameplay session is established, no chat is sent, and the connection is closed immediately after the response is received.

---

## Data handling

MineScan only collects information that Minecraft servers already expose publicly.

- No authentication is performed
- No exploitation or vulnerability scanning is used
- No persistent connections are maintained
- No gameplay interaction occurs

---

## Scale (as of January 2026)

- 500,000+ Minecraft servers indexed
- 1,500,000+ unique players observed through server activity

These values are continuously updated and reflect the current state of the dataset.

---

## Purpose

MineScan is a research and tooling project for:
- Server discovery
- Network-level Minecraft ecosystem analysis
- Data aggregation for publicly reachable servers

It was built as a personal project and expanded over time into a larger system.

---

## Opt-out

If you own a server and want it removed from indexing, you can request removal via Discord:

https://discord.gg/AYbDNEWgHE
