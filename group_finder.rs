// ============================================================
//  group_finder.rs  –  เข้าทุกกลุ่ม + สแกนหาซอง TrueMoney
// ============================================================

use chrono::Utc;
use grammers_client::{types::Chat, Client};
use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use reqwest::Client as HttpClient;
use serde_json::{json, Value};
use std::collections::HashSet;
use tokio::time::{sleep, Duration};

use crate::{config, database as db};

static RE_GROUP_LINK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"t\.me/[a-zA-Z0-9_+]+").unwrap());

static RE_VOUCHER_URL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https?://gift\.truemoney\.com/campaign[^\s]*[?&]v=([A-Za-z0-9]+)").unwrap()
});

static RE_NON_ALNUM: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[^a-zA-Z0-9]").unwrap());

static RE_SPLIT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"[\s\n\r,;.!?()\[\]{}'\"<>/\\]+"#).unwrap());

static HTTP: Lazy<HttpClient> = Lazy::new(HttpClient::new);

pub async fn sleep_ms(ms: u64) {
    sleep(Duration::from_millis(ms)).await;
}

pub fn random_delay(min_ms: u64, max_ms: u64) -> u64 {
    rand::thread_rng().gen_range(min_ms..=max_ms)
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

pub fn extract_group_links(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    RE_GROUP_LINK
        .find_iter(text)
        .filter_map(|m| {
            let s = m.as_str().to_string();
            if seen.insert(s.clone()) { Some(s) } else { None }
        })
        .collect()
}

fn is_valid_voucher(code: &str) -> bool {
    if code.len() < config::VOUCHER_CODE_MIN_LEN || code.len() > 64 {
        return false;
    }
    if !code.starts_with(config::VOUCHER_CODE_PREFIX) {
        return false;
    }
    if !code.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    let has_digit = code.chars().any(|c| c.is_ascii_digit());
    let has_alpha = code.chars().any(|c| c.is_ascii_alphabetic());
    if !has_digit || !has_alpha {
        return false;
    }
    let lower = code.to_lowercase();
    let blacklist = ["telegram", "truemoney", "password", "username"];
    !blacklist.iter().any(|w| lower.contains(w))
}

pub fn extract_vouchers(text: &str) -> Vec<String> {
    let mut seen    = HashSet::new();
    let mut results = Vec::new();
    for cap in RE_VOUCHER_URL.captures_iter(text) {
        let code = cap[1].to_string();
        if is_valid_voucher(&code) && seen.insert(code.clone()) {
            results.push(code);
        }
    }
    for word in RE_SPLIT.split(text) {
        let clean = RE_NON_ALNUM.replace_all(word, "").to_string();
        if is_valid_voucher(&clean) && seen.insert(clean.clone()) {
            results.push(clean);
        }
    }
    results
}

pub async fn send_webhook(url: &str, embed: Value) {
    let _ = HTTP
        .post(url)
        .json(&json!({ "embeds": [embed] }))
        .timeout(Duration::from_secs(30))
        .send()
        .await;
}

pub async fn notify_joined(webhook: &str, group: &str, link: &str, all: &[String]) {
    let clickable = all.iter().take(10)
        .map(|l| format!("https://{}", l))
        .collect::<Vec<_>>()
        .join("\n");
    send_webhook(webhook, json!({
        "title": "✅ เข้ากลุ่มสำเร็จ",
        "color": 0x00FF00_u32,
        "fields": [
            { "name": "📌 กลุ่มต้นทาง",         "value": group,"inline": true  },
            { "name": "🔗 กลุ่มที่เข้า",         "value": format!("https://{}", link),                   "inline": true  },
            { "name": "🔗 ลิงก์ทั้งหมดในกลุ่ม", "value": format!("{} ลิงก์\n{}", all.len(), clickable), "inline": false },
        ],
        "footer":    { "text": "⛏️ Auto Group Finder – เข้าทุกกลุ่ม" },
        "timestamp": now_iso(),
    })).await;
}

pub async fn notify_voucher(webhook: &str, group: &str, vouchers: &[String], links: &[String]) {
    let vlist = vouchers.iter().take(10)
        .map(|v| format!("https://gift.truemoney.com/campaign/?v={}", v))
        .collect::<Vec<_>>().join("\n");
    let llist = if links.is_empty() { "ไม่พบ".into() }else { links.iter().take(5).map(|l| format!("https://{}", l)).collect::<Vec<_>>().join("\n") };
    send_webhook(webhook, json!({
        "title": "🎁 พบอั่งเปา TrueMoney!",
        "color": 0xFFAA00_u32,
        "fields": [
            { "name": "📌 กลุ่ม",        "value": group,                            "inline": true  },
            { "name": "🎫 จำนวน",        "value": format!("{} อัน", vouchers.len()), "inline": true  },
            { "name": "🔗 ลิงก์ในกลุ่ม", "value": llist,                             "inline": false },
            { "name": "🎟️ โค้ดที่พบ",   "value": vlist,                             "inline": false },
        ],
        "footer":    { "text": "🎁 Auto Voucher Scanner" },
        "timestamp": now_iso(),
    })).await;
}

pub async fn join_group(client: &Client, link: &str) -> bool {
    let clean = link.trim_start_matches("t.me/").to_string();
    let result: anyhow::Result<()> = async {
        if clean.starts_with('+') {
            let hash = clean.trim_start_matches('+');
            client.invoke(&grammers_tl_types::functions::messages::ImportChatInvite {
                hash: hash.to_string(),}).await?;
        } else {
            if let Some(chat) = client.resolve_username(&clean).await? {
                match &chat {
                    Chat::Group(_) | Chat::Channel(_) => { client.join_chat(&chat).await?; }
                    _ => {}
                }
            }
        }
        Ok(())
    }.await;
    match result {
        Ok(_)  => { println!("  ✅ เข้ากลุ่ม: {}", link); true  }
        Err(e) => { println!("  ⚠️ เข้าไม่ได้: {} – {}", link, e); false }
    }
}

pub struct ScanResult {
    pub success:       bool,
    pub joined_count:  usize,
    pub voucher_count: usize,
}

pub async fn find_and_scan(client: &Client, user_id: &str, user_data: &db::UserData) -> ScanResult {
    println!("[Finder] 🔍 เริ่มรอบสแกน (USER: {})", user_id);
    let mut joined_count  = 0usize;
    let mut voucher_count = 0usize;

    let mut dialogs = Vec::new();
    let mut it = client.iter_dialogs();
    loop {
        match it.next().await {
            Ok(Some(d)) => match d.chat() {
                Chat::Group(_) | Chat::Channel(_) => dialogs.push(d),
                _ => {}
            },
            Ok(None) => break,
            Err(e)   => { println!("[Finder] iter_dialogs error: {}", e); break; }
        }
    }
    println!("[Finder] 📋 พบ {} กลุ่ม/ช่อง", dialogs.len());

    for dialog in &dialogs {
        let chat       = dialog.chat();
        let group_id   = chat.id().to_string();
        let group_name = chat.name().to_string();

        let jd = db::get_joined_groups(user_id).await;
        if jd.scanned.contains(&group_id) { continue; }

        let mut parts = Vec::new();
        let mut mit   = client.iter_messages(chat).limit(config::SCAN_LIMIT as usize);
        loop {
            match mit.next().await {
                Ok(Some(msg)) => {
                    let t = msg.text();
                    if !t.is_empty() { parts.push(t.to_string()); }
                }
                Ok(None) => break,
                Err(e)   => { println!("  ⚠️ iter_messages: {}", e); break; }
            }
        }
        let all_text = parts.join("\n");
        let links    = extract_group_links(&all_text);
        let vouchers = extract_vouchers(&all_text);

        db::add_scanned_group(user_id, &group_id).await;
        println!("📍 [{group_name}]  ลิงก์: {}  |  อั่งเปา: {}", links.len(), vouchers.len());

        if !vouchers.is_empty() && !user_data.webhook.is_empty() {
            voucher_count += vouchers.len();
            notify_voucher(&user_data.webhook, &group_name, &vouchers, &links).await;
        }

        for link in &links {
            let jd2 = db::get_joined_groups(user_id).await;
            if jd2.joined.contains(link) { continue; }
            if join_group(client, link).await {
                db::add_joined_group(user_id, link).await;
                joined_count += 1;
                if !user_data.webhook.is_empty() {
                    notify_joined(&user_data.webhook, &group_name, link, &links).await;
                }
                sleep_ms(random_delay(config::COOLDOWN_MIN_MS, config::COOLDOWN_MAX_MS)).await;
            }
        }

        sleep_ms(random_delay(3_000, 5_000)).await;
    }

    println!("[Finder] ✅ เสร็จ  | เข้ากลุ่ม: {joined_count}  | ซองที่พบ: {voucher_count}");
    ScanResult { success: true, joined_count, voucher_count }
}
