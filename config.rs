// ============================================================
//  config.rs  –  ✏️ ตั้งค่าที่นี่ก่อน build
// ============================================================

/// โทเค่น Discord Bot (จาก Discord Developer Portal)
pub const TOKEN: &str = "MTQ2Njg3NzU1NzA3MTk0MTY0NA.Gw6X5M.BAt3YQu5ifDEiGGi22jTWILFJj8U29jMsswOW0";

/// Discord User ID ของแอดมิน (ใส่ได้หลายคน)
pub const ADMIN_IDS: &[&str] = &["745535330681225278"];

/// จำนวน Telegram session สูงสุดพร้อมกัน
pub const MAX_USERS: usize = 50;

/// จำนวนข้อความสูงสุดที่อ่านต่อกลุ่ม
pub const SCAN_LIMIT: i32 = 2000;

/// RAM สูงสุดก่อน auto-restart (MB)
pub const MEMORY_LIMIT_MB: u64 = 512;

/// หน่วงขั้นต่ำระหว่างเข้ากลุ่ม (ms) = 3 นาที
pub const COOLDOWN_MIN_MS: u64 = 180_000;

/// หน่วงสูงสุดระหว่างเข้ากลุ่ม (ms) = 5 นาที
pub const COOLDOWN_MAX_MS: u64 = 300_000;

/// รีเซ็ต scanned list ทุก 1 ชั่วโมง (ms)
pub const CLEAR_SCANNED_INTERVAL_MS: u64 = 3_600_000;

/// prefix โค้ดซอง TrueMoney
pub const VOUCHER_CODE_PREFIX: &str = "019";

/// ความยาวขั้นต่ำของโค้ดซอง TrueMoney
pub const VOUCHER_CODE_MIN_LEN: usize = 23;
