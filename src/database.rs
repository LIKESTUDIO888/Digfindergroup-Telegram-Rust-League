// ============================================================
//  database.rs  –  JSON file database + async in-memory cache
// ============================================================

use std::{collections::HashMap, path::PathBuf};
use anyhow::Result;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sysinfo::{Pid, System};
use tokio::sync::RwLock;

// ── Paths ─────────────────────────────────────────────────

fn data_dir()     -> PathBuf { PathBuf::from("data") }
fn users_dir()    -> PathBuf { data_dir().join("users") }
fn sessions_dir() -> PathBuf { data_dir().join("sessions") }
fn joined_dir()   -> PathBuf { data_dir().join("joined_groups") }

// ── Structs ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserData {
    #[serde(rename = "apiId",         default)] pub api_id:         String,
    #[serde(rename = "apiHash",       default)] pub api_hash:        String,
    #[serde(rename = "telegramPhone", default)] pub telegram_phone:  String,
    #[serde(default)]                           pub webhook:          String,
    #[serde(default)]                           pub status:           String,
}

impl UserData {
    pub fn is_complete(&self) -> bool {
        !self.api_id.is_empty()
            && !self.api_hash.is_empty()
            && !self.telegram_phone.is_empty()
            && !self.webhook.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JoinedGroups {
    #[serde(default)] pub joined:  Vec<String>,
    #[serde(default)] pub scanned: Vec<String>,
}

// ── Global caches ─────────────────────────────────────────

static CACHE_USERS:  Lazy<RwLock<HashMap<String, UserData>>>     = Lazy::new(Default::default);
static CACHE_JOINED: Lazy<RwLock<HashMap<String, JoinedGroups>>> = Lazy::new(Default::default);

// ── Init ─────────────────────────────────────────────────

pub async fn init_database() -> Result<()> {
    for dir in [data_dir(), users_dir(), sessions_dir(), joined_dir()] {
        tokio::fs::create_dir_all(&dir).await?;
    }
    println!("[Database] ✅ ระบบฐานข้อมูลพร้อมใช้งาน");
    Ok(())
}

// ── User Data ─────────────────────────────────────────────

pub async fn get_user_data(uid: &str) -> Option<UserData> {
    // ลอง cache ก่อน
    if let Some(d) = CACHE_USERS.read().await.get(uid) {
        return Some(d.clone());
    }
    // อ่านจากไฟล์
    let path = users_dir().join(format!("{uid}.json"));
    let text = tokio::fs::read_to_string(&path).await.ok()?;
    let data: UserData = serde_json::from_str(&text).ok()?;
    CACHE_USERS.write().await.insert(uid.to_string(), data.clone());
    Some(data)
}

pub async fn save_user_data(uid: &str, data: &UserData) -> Result<()> {
    let path = users_dir().join(format!("{uid}.json"));
    tokio::fs::write(&path, serde_json::to_string_pretty(data)?).await?;
    CACHE_USERS.write().await.insert(uid.to_string(), data.clone());
    Ok(())
}

pub async fn delete_user_data(uid: &str) -> Result<()> {
    for path in [
        users_dir().join(format!("{uid}.json")),
        sessions_dir().join(format!("{uid}.session")),
        joined_dir().join(format!("{uid}.json")),
    ] {
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }
    }
    CACHE_USERS.write().await.remove(uid);
    CACHE_JOINED.write().await.remove(uid);
    Ok(())
}

pub async fn get_all_users() -> Vec<String> {
    let Ok(mut rd) = tokio::fs::read_dir(users_dir()).await else { return vec![]; };
    let mut out = Vec::new();
    while let Ok(Some(e)) = rd.next_entry().await {
        let name = e.file_name();
        let s    = name.to_string_lossy();
        if s.ends_with(".json") {
            out.push(s.trim_end_matches(".json").to_string());
        }
    }
    out
}

// ── Sessions  (grammers session bytes → base64 on disk) ──

pub async fn save_session(uid: &str, bytes: &[u8]) -> Result<()> {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let path    = sessions_dir().join(format!("{uid}.session"));
    tokio::fs::write(path, encoded).await?;
    Ok(())
}

pub async fn get_session(uid: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let path = sessions_dir().join(format!("{uid}.session"));
    let text = tokio::fs::read_to_string(path).await.ok()?;
    base64::engine::general_purpose::STANDARD.decode(text.trim()).ok()
}

// ── Joined / Scanned Groups ───────────────────────────────

pub async fn get_joined_groups(uid: &str) -> JoinedGroups {
    if let Some(d) = CACHE_JOINED.read().await.get(uid) {
        return d.clone();
    }
    let path = joined_dir().join(format!("{uid}.json"));
    let text = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    let data: JoinedGroups = serde_json::from_str(&text).unwrap_or_default();
    CACHE_JOINED.write().await.insert(uid.to_string(), data.clone());
    data
}

pub async fn save_joined_groups(uid: &str, data: &JoinedGroups) -> Result<()> {
    let path = joined_dir().join(format!("{uid}.json"));
    tokio::fs::write(path, serde_json::to_string_pretty(data)?).await?;
    CACHE_JOINED.write().await.insert(uid.to_string(), data.clone());
    Ok(())
}

pub async fn add_joined_group(uid: &str, link: &str) {
    let mut d = get_joined_groups(uid).await;
    if !d.joined.contains(&link.to_string()) {
        d.joined.push(link.to_string());
        let _ = save_joined_groups(uid, &d).await;
    }
}

pub async fn add_scanned_group(uid: &str, group_id: &str) {
    let mut d = get_joined_groups(uid).await;
    if !d.scanned.contains(&group_id.to_string()) {
        d.scanned.push(group_id.to_string());
        let _ = save_joined_groups(uid, &d).await;
    }
}

pub async fn clear_scanned_groups(uid: &str) -> usize {
    let mut d = get_joined_groups(uid).await;
    let n     = d.scanned.len();
    d.scanned.clear();
    let _ = save_joined_groups(uid, &d).await;
    if n > 0 { println!("[Database] 🧹 ล้าง scanned {n} กลุ่ม (USER: {uid})"); }
    n
}

// ── Memory / Cache ────────────────────────────────────────

pub async fn clear_memory_cache() -> usize {
    let mut users = CACHE_USERS.write().await;
    let n         = users.len();
    users.clear();
    drop(users);
    CACHE_JOINED.write().await.clear();
    if n > 0 { println!("[Database] 🧹 ล้าง cache {n} รายการ"); }
    n
}

#[derive(Debug, Serialize)]
pub struct MemoryUsage {
    pub rss_mb: f64,
    pub vms_mb: f64,
}

pub fn get_memory_usage() -> MemoryUsage {
    let mut sys = System::new_all();
    sys.refresh_all();
    let pid = std::process::id() as usize;
    if let Some(proc) = sys.process(Pid::from(pid)) {
        MemoryUsage {
            rss_mb: (proc.memory()         as f64 / 1_048_576.0 * 100.0).round() / 100.0,
            vms_mb: (proc.virtual_memory() as f64 / 1_048_576.0 * 100.0).round() / 100.0,
        }
    } else {
        MemoryUsage { rss_mb: 0.0, vms_mb: 0.0 }
    }
}
