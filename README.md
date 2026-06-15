# ⛏️ DigtgAI — Rust Edition

**Auto Telegram Group Finder + TrueMoney Voucher Scanner**
แปลงจาก Python มาเป็น Rust ทั้งหมด

---

## 📁 โครงสร้างไฟล์

```
DigtgAI_rust/
├── Cargo.toml          ← dependency ทั้งหมด
├── setup.bat           ← ติดตั้ง Rust + build ครั้งแรก
├── start_bot.bat       ← รันบอท
├── README.md           ← เอกสารนี้
└── src/
    ├── main.rs         ← Discord Bot, slash commands, event handler
    ├── config.rs       ← ✏️ ตั้งค่าที่นี่ (TOKEN, ADMIN_IDS ฯลฯ)
    ├── database.rs     ← JSON file database + memory cache
    ├── group_finder.rs ← Telegram group scanner + webhook
    └── server.rs       ← HTTP API (axum) port 8080
```

---

## ⚙️ ตั้งค่า

แก้ไขไฟล์ `src/config.rs`:

```rust
pub const TOKEN: &str = "YOUR_DISCORD_BOT_TOKEN";
pub const ADMIN_IDS: &[&str] = &["YOUR_DISCORD_USER_ID"];
```

ปรับค่าอื่น ๆ ตามต้องการ:
| ค่า | ค่าเริ่มต้น | คำอธิบาย |
|-----|-----------|---------|
| `MAX_USERS` | 100 | จำนวนผู้ใช้พร้อมกันสูงสุด |
| `SCAN_LIMIT` | 500 | ข้อความสูงสุดต่อกลุ่ม |
| `MEMORY_LIMIT_MB` | 512 | RAM สูงสุดก่อน restart |
| `COOLDOWN_MIN_MS` | 180000 | หน่วงขั้นต่ำระหว่างเข้ากลุ่ม (3 นาที) |
| `COOLDOWN_MAX_MS` | 300000 | หน่วงสูงสุดระหว่างเข้ากลุ่ม (5 นาที) |
| `CLEAR_SCANNED_INTERVAL_MS` | 3600000 | รีเซ็ต scanned list ทุก 1 ชั่วโมง |

---

## 🚀 ติดตั้งและรัน

### Windows
```
1. รัน setup.bat  (ติดตั้ง Rust + build)
2. รัน start_bot.bat
```

### Linux / VPS
```bash
# ติดตั้ง Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Build
cargo build --release

# รัน
./target/release/digtg_ai
```

### PM2 (VPS)
```bash
pm2 start ./target/release/digtg_ai --name digtg_ai
pm2 save
```

---

## 🛠️ Slash Commands (Admin)

| Command | คำอธิบาย |
|---------|---------|
| `/setup_finder #channel` | ส่งแผงควบคุมไปยังช่อง |
| `/manage_users` | ดูรายชื่อผู้ใช้ทั้งหมด |
| `/system_info` | ดู RAM / uptime / จำนวนผู้ใช้ |

---

## 🌐 HTTP API (port 8080)

| Endpoint | คำอธิบาย |
|---------|---------|
| `GET /` | health check |
| `GET /users` | รายชื่อ user IDs ทั้งหมด |
| `GET /system` | RAM usage |

---

## 📦 Dependencies หลัก

| Crate | วัตถุประสงค์ |
|-------|-----------|
| `serenity 0.12` | Discord API |
| `poise 0.6` | Slash commands framework |
| `grammers-client 0.7` | Telegram MTProto client |
| `tokio` | Async runtime |
| `axum 0.7` | HTTP server |
| `reqwest` | HTTP client (webhook) |
| `sysinfo` | Memory monitoring |
| `dashmap` | Thread-safe HashMap |
| `regex` | Voucher / link extraction |

---

## ⚠️ หมายเหตุ Session

Session ของ grammers-client **ไม่เข้ากัน**กับ Telethon StringSession ของ Python
→ ผู้ใช้ต้องล็อกอิน Telegram ใหม่ครั้งแรกผ่านปุ่ม "เริ่มหากลุ่ม"
