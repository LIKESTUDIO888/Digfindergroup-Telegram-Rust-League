# ⛏️ DigtgAI — Rust Edition

**Auto Telegram Group Finder + TrueMoney Voucher Scanner**
แปลงจาก Python มาเป็น Rust ทั้งหมด

---

<!-- coderabbit-ai-generated:v1 -->

<!-- coderabbit-ai-generated-section:v1:c782a0fbd10ae08f176c74bc341e199600bf62001607b32e5c4ed610822a658a -->

## 📁 โครงสร้างไฟล์

```
├── Cargo.toml, Cargo.lock  ← dependencies และเวอร์ชันที่ใช้ build
├── README.md
└── src/
    ├── main.rs         ← Discord Bot และ slash commands
    ├── config.rs       ← ตั้งค่า TOKEN, ADMIN_IDS และการค้นหา
    ├── database.rs     ← JSON database และ cache
    ├── group_finder.rs ← ค้นหากลุ่ม สแกนข้อความ และ webhook
    └── server.rs       ← HTTP API port 8080
```

---



<!-- /coderabbit-ai-generated:v1 -->

## ⚙️ ตั้งค่า

แก้ไขไฟล์ `src/config.rs`:

```rust
pub const TOKEN: &str = "YOUR_DISCORD_BOT_TOKEN";
pub const ADMIN_IDS: &[&str] = &["YOUR_DISCORD_USER_ID"];
```

<!-- coderabbit-ai-generated:v1 -->

<!-- coderabbit-ai-generated-section:v1:ad0be656cb26f6930a35bac10cd958af5975aff4e81513c3b846d534f6d09542 -->

ปรับค่าอื่น ๆ ใน `src/config.rs` ตามต้องการ:

| ค่า | ค่าเริ่มต้น | คำอธิบาย |
|-----|-----------|---------|
| `SEARCH_KEYWORDS` | แจกฟรี, ทรูวอเลต, แจกซอง | คำค้นหากลุ่มและข้อความ |
| `SEARCH_LIMIT` | 50 | ผลลัพธ์สูงสุดต่อคำค้นในแต่ละวิธีค้นหา |
| `SEARCH_INTERVAL_SECS` | 3600 | ระยะห่างระหว่างรอบค้นหาเป็นวินาที |
| `MAX_USERS` | 50 | จำนวนผู้ใช้พร้อมกันสูงสุด |
| `SCAN_LIMIT` | 2000 | ข้อความสูงสุดต่อกลุ่ม |
| `MEMORY_LIMIT_MB` | 512 | RAM สูงสุดก่อน restart |
| `COOLDOWN_MIN_MS` / `COOLDOWN_MAX_MS` | 180000 / 300000 | หน่วงหลังเข้ากลุ่มสำเร็จ 3–5 นาที |
| `CLEAR_SCANNED_INTERVAL_MS` | 3600000 | รีเซ็ตรายชื่อกลุ่มที่สแกนทุกชั่วโมง |

ระบบค้นหากลุ่มและช่องสาธารณะจากไดเรกทอรี Telegram และค้นหาข้อความที่บัญชีมองเห็นด้วยคีย์เวิร์ด จากนั้นตรวจลิงก์ `t.me` ในข้อความและกลุ่มที่บัญชีเข้าร่วมแล้ว รวมถึงลิงก์ที่ซ่อนอยู่ในข้อความ ลิงก์ที่พบจะถูกนำไปลองเข้ากลุ่มหรือช่องอัตโนมัติ แล้วสแกนซองและแจ้งผ่าน webhook ผลการค้นหาขึ้นอยู่กับสิทธิ์บัญชีและผลลัพธ์ที่ Telegram ส่งกลับ

---

## 🚀 ติดตั้งและรัน

ติดตั้ง Rust แบบ stable ด้วย [rustup](https://rustup.rs/) แล้วตั้งค่าใน `src/config.rs` จากนั้นรัน:

```bash
cargo build --release --locked
./target/release/digtg_ai
```

Windows สามารถใช้ `cargo build --release --locked` และรัน `target\release\digtg_ai.exe` ได้เช่นกัน

### PM2 (VPS)

```bash
pm2 start ./target/release/digtg_ai --name digtg_ai
pm2 save
```

---



<!-- /coderabbit-ai-generated:v1 -->

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
