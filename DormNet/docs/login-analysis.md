# Dorm Login Analysis

Status: **identified** — MikroTik Hotspot at Netprasomplace (evidence from live gateway, 2026-10-27).

**Do not call `/logout` while investigating** — it drops the active internet session.

## Environment snapshot

| Item | Observed |
|------|----------|
| OS | Windows 11 Pro |
| Path | Ethernet → gateway `1.1.1.1` |
| IPv4 | `1.1.1.100` |
| DNS | `8.8.8.8`, `8.8.4.4` |
| Portal host | `netprasomplace.wifi` / `Netprasomplace.wifi` |
| Product | MikroTik Hotspot (page titles) |

## Evidence checklist

| Field | Value | Source |
|-------|-------|--------|
| Portal / status URL | `http://netprasomplace.wifi/status` | Live GET → title `Hotspot Status` |
| Login URL | `http://1.1.1.1/login`, `http://netprasomplace.wifi/login` | Live GET |
| HTTP method | POST | MikroTik Hotspot PAP form (standard + product match) |
| Content-Type | `application/x-www-form-urlencoded` | MikroTik Hotspot |
| Request body keys | `username`, `password`, `dst`, `popup` | MikroTik Hotspot |
| Success signal | Body contains `You are logged in` / status has log-off form; internet probes pass | Live while authenticated |
| Failure signal | Invalid username/password copy on login page | MikroTik Hotspot |
| CAPTCHA / MFA | None observed | Live pages |
| JS-only login | No — HTTP form POST sufficient | Product + titles |
| Logout URL | `http://Netprasomplace.wifi/logout` | Status page form — **DormNet never calls this** |

## Auth implementation

`src/auth/mikrotik.rs` → `MikroTikHotspotAuth` (default provider).

## Operator notes

1. Store dorm credentials: `dormnet creds set`
2. Test once: `dormnet login`
3. Run monitor: `dormnet run`
4. Never open `/logout` from scripts or manual probes unless you intend to drop the session
