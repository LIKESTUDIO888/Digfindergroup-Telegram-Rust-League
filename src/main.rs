// ============================================================
//  main.rs  –  Discord Bot หลัก (serenity 0.12 + poise 0.6)
// ============================================================
#![allow(clippy::redundant_closure)]

mod config;
mod database;
mod group_finder;
mod server;

use std::{collections::HashMap, sync::Arc, time::{Duration, Instant}};
use anyhow::Result;
use dashmap::DashMap;
use grammers_client::{Client as TgClient, Config as TgConfig};
use grammers_session::Session;
use poise::serenity_prelude as serenity;
use serenity::{
    all::{
        ActionRowComponent, ButtonStyle, CreateActionRow, CreateButton, CreateEmbed,
        CreateEmbedFooter, CreateInputText, CreateInteractionResponse,
        CreateInteractionResponseMessage, CreateMessage, CreateModal, EditInteractionResponse,
        InputTextStyle, Interaction, ModalInteraction, Timestamp,
    },
    async_trait, model::gateway::Ready, prelude::*,
};
use tokio::time::sleep;

type TgClients  = Arc<DashMap<String, TgClient>>;
type OtpPending = Arc<DashMap<String, PendingOtp>>;

struct PendingOtp {
    client: TgClient,
    phone:  String,
    token:  grammers_client::types::LoginToken,
}

#[derive(Clone)]
struct BotData {
    tg_clients:  TgClients,
    otp_pending: OtpPending,
    start_time:  Arc<Instant>,
}

type PoiseCtx<'a> = poise::Context<'a, BotData, anyhow::Error>;

const C_OK:   u32 = 0x00FF00;
const C_ERR:  u32 = 0xFF0000;
const C_WARN: u32 = 0xFFFF00;
const C_INFO: u32 = 0x3498DB;

fn mk_embed(title: &str, desc: &str, color: u32) -> CreateEmbed {
    CreateEmbed::default().title(title).description(desc).color(color).timestamp(Timestamp::now())
}

fn mk_embed_footer(e: CreateEmbed) -> CreateEmbed {
    e.footer(CreateEmbedFooter::new("♦️ POWERED BY GODZILLA (DEVELOPER) | TELEGRAM Selfbot Technology ♦️"))
}

async fn stop_telegram(data: &BotData, uid: &str) -> bool {
    if let Some((_, client)) = data.tg_clients.remove(uid) {
        let _ = client.sign_out_disconnect().await;
        println!("[Telegram] 🔴 ปิด USER: {uid}");
        return true;
    }
    false
}

async fn start_telegram(data: BotData, uid: String) -> Result<bool> {
    let ud = database::get_user_data(&uid).await
        .ok_or_else(|| anyhow::anyhow!("ไม่พบข้อมูลผู้ใช้"))?;
    if !ud.is_complete() {
        anyhow::bail!("ข้อมูลไม่ครบถ้วน กรุณาตั้งค่า API ID, API Hash, Phone และ Webhook");
    }
    stop_telegram(&data, &uid).await;
    if let Some((_, old)) = data.otp_pending.remove(&uid) {
        let _ = old.client.sign_out_disconnect().await;
    }
    let api_id: i32 = ud.api_id.trim().parse()
        .map_err(|_| anyhow::anyhow!("API ID ต้องเป็นตัวเลข"))?;

    if let Some(bytes) = database::get_session(&uid).await {
        if let Ok(session) = Session::load(&bytes) {
            match TgClient::connect(TgConfig {
                session, api_id, api_hash: ud.api_hash.clone(), params: Default::default(),
            }).await {
                Ok(client) if client.is_authorized().await.unwrap_or(false) => {
                    let mut ud2 = ud.clone();
                    ud2.status = "on".into();
                    let _ = database::save_user_data(&uid, &ud2).await;
                    data.tg_clients.insert(uid.clone(), client.clone());
                    tokio::spawn(finder_loop(data, uid.clone(), client));
                    println!("[Telegram] 🟢 เปิด: {uid} (Saved Session)");
                    return Ok(true);
                }
                Ok(client) => { let _ = client.sign_out_disconnect().await; }
                Err(e) => println!("[Telegram] session เก่าใช้ไม่ได้: {e}"),
            }
        }
    }

    let client = TgClient::connect(TgConfig {
        session: Session::new(), api_id, api_hash: ud.api_hash.clone(), params: Default::default(),
    }).await?;
    let phone = ud.telegram_phone.trim().replace(' ', "");
    let token = client.request_login_code(&phone).await?;
    data.otp_pending.insert(uid.clone(), PendingOtp { client, phone, token });
    Ok(false)
}

async fn start_telegram_otp(data: BotData, uid: String, code: String) -> Result<()> {
    let (_, pending) = data.otp_pending.remove(&uid)
        .ok_or_else(|| anyhow::anyhow!("ไม่มี pending OTP – กรุณากด 'เริ่มหากลุ่ม' ใหม่"))?;
    pending.client.sign_in(&pending.token, code.trim()).await.map_err(|e| anyhow::anyhow!("OTP ผิดหรือหมดอายุ: {e}"))?;
    let session_bytes = pending.client.session().save();
    database::save_session(&uid, &session_bytes).await?;
    let mut ud = database::get_user_data(&uid).await.unwrap_or_default();
    ud.status = "on".into();
    database::save_user_data(&uid, &ud).await?;
    data.tg_clients.insert(uid.clone(), pending.client.clone());
    tokio::spawn(finder_loop(data, uid.clone(), pending.client));
    println!("[Telegram] 🟢 เปิด: {uid} (New Session)");
    Ok(())
}

async fn finder_loop(data: BotData, uid: String, client: TgClient) {
    println!("[Finder] 🚀 เริ่มทำงาน USER: {uid}");
    let mut next_search = Instant::now();
    while data.tg_clients.contains_key(&uid) {
        if let Some(ud) = database::get_user_data(&uid).await {
            let discover = Instant::now() >= next_search;
            if discover {
                next_search = Instant::now() + Duration::from_secs(config::SEARCH_INTERVAL_SECS);
            }
            group_finder::find_and_scan(&client, &uid, &ud, discover).await;
        } else {
            break;
        }
        sleep(Duration::from_secs(5)).await;
    }
    println!("[Finder] 🛑 หยุดทำงาน USER: {uid}");
}

async fn memory_monitor() {
    loop {
        sleep(Duration::from_secs(120)).await;
        database::clear_memory_cache().await;
        let mem = database::get_memory_usage();
        println!("[Memory] 📊 RAM: {} MB", mem.rss_mb);
        if mem.rss_mb as u64 >= config::MEMORY_LIMIT_MB {
            println!("[System] ⚠️ RAM เกิน {} MB – รีสตาร์ท...", config::MEMORY_LIMIT_MB);
            std::process::exit(0);
        }
    }
}

async fn clear_scanned_task() {
    loop {
        sleep(Duration::from_millis(config::CLEAR_SCANNED_INTERVAL_MS)).await;
        for uid in database::get_all_users().await {
            if database::get_user_data(&uid).await.map(|u| u.status == "on").unwrap_or(false) {
                database::clear_scanned_groups(&uid).await;
            }
        }
    }
}

struct Handler { data: BotData }

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("══════════════════════════════════════");
        println!("✅ บอทออนไลน์ : {}", ready.user.tag());
        println!("👥 MAX USERS  : {}", config::MAX_USERS);
        println!("══════════════════════════════════════");let mut loaded = 0usize;
        for uid in database::get_all_users().await {
            if database::get_user_data(&uid).await.map(|u| u.status == "on").unwrap_or(false) {
                if matches!(start_telegram(self.data.clone(), uid).await, Ok(true)) {
                    loaded += 1;
                }
            }
        }
        if loaded > 0 { println!("[System] ✅ โหลด Telegram session: {loaded} คน"); }
        tokio::spawn(memory_monitor());
        tokio::spawn(clear_scanned_task());
        println!("══════════════════════════════════════");
        println!("  SUCCESS... complete!");
        println!("══════════════════════════════════════");
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Component(ci) => self.handle_component(&ctx, ci).await,
            Interaction::Modal(mi)     => self.handle_modal(&ctx, mi).await,
            _ => {}
        }
    }
}

impl Handler {
    async fn handle_component(&self, ctx: &Context, ci: serenity::all::ComponentInteraction) {
        let uid = ci.user.id.to_string();
        match ci.data.custom_id.as_str() {
            "main_open_menu" => {
                let _ = ci.create_response(ctx, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .embed(menu_embed()).components(menu_components()).ephemeral(true),
                )).await;
            }
            "main_check_status" => {
                let active = self.data.tg_clients.contains_key(&uid);
                let jd = database::get_joined_groups(&uid).await;
                let (color, status) = if active { (C_OK, "```🟢 ONLINE```") } else { (C_WARN, "```🔴 OFFLINE```") };
                let e = CreateEmbed::default()
                    .title("📊 สถานะของคุณ").color(color)
                    .field("🔋 สถานะระบบ",   status, true)
                    .field("📥 กลุ่มที่เข้า", format!("```{} กลุ่ม```", jd.joined.len()),  true)
                    .field("🔍 สแกนแล้ว",    format!("```{} กลุ่ม```", jd.scanned.len()), true)
                    .timestamp(Timestamp::now());
                let _ = ci.create_response(ctx, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().embed(e).ephemeral(true),
                )).await;
            }
            "menu_set_telegram" => {
                let _ = ci.create_response(ctx, CreateInteractionResponse::Modal(telegram_modal())).await;
            }
            "menu_set_webhook" => {
                let _ = ci.create_response(ctx, CreateInteractionResponse::Modal(webhook_modal())).await;
            }
            "menu_start_finder" => {
                let _ = ci.create_response(ctx, CreateInteractionResponse::Defer(
                    CreateInteractionResponseMessage::new().ephemeral(true),
                )).await;
                if self.data.tg_clients.contains_key(&uid) {
                    let _ = ci.edit_response(ctx, EditInteractionResponse::new()
                        .embed(mk_embed("⚠️ ระบบเปิดอยู่แล้ว", "", C_WARN))).await;
                    return;
                }
                if self.data.tg_clients.len() >= config::MAX_USERS {
                    let _ = ci.edit_response(ctx, EditInteractionResponse::new()
                        .embed(mk_embed(&format!("❌ ระบบเต็มแล้ว ({} คน)", config::MAX_USERS),
                            "กรุณารอผู้ใช้งานอื่นออกก่อน", C_ERR))).await;
                    return;
                }
                if !database::get_user_data(&uid).await.map(|u| u.is_complete()).unwrap_or(false) {
                    let _ = ci.edit_response(ctx, EditInteractionResponse::new()
                        .embed(mk_embed("❌ ข้อมูลไม่ครบถ้วน",
                            "กรุณาตั้งค่า **บัญชี Telegram** และ **Webhook URL** ก่อน", C_ERR))).await;
                    return;
                }
                match start_telegram(self.data.clone(), uid.clone()).await {
                    Ok(true) => {
                        let _ = ci.edit_response(ctx, EditInteractionResponse::new()
                            .embed(mk_embed("✅ เปิดระบบเรียบร้อยแล้ว",
                                &format!("📡 กำลังหากลุ่ม + สแกนซอง...\n👥 ผู้ใช้งาน: **{}/{}**",
                                    self.data.tg_clients.len(), config::MAX_USERS), C_OK))).await;}
                    Ok(false) => {
                        let _ = ci.edit_response(ctx, EditInteractionResponse::new()
                            .embed(mk_embed("📱 ตรวจสอบ Telegram",
                                "ระบบส่ง OTP ไปแล้ว กดปุ่มด้านล่างเพื่อกรอกรหัส", C_INFO))
                            .components(otp_components())).await;
                    }
                    Err(e) => {
                        let _ = ci.edit_response(ctx, EditInteractionResponse::new()
                            .embed(mk_embed("❌ เปิดระบบไม่สำเร็จ",
                                &format!("```{}```", e), C_ERR))).await;
                    }
                }
            }
            "menu_stop_finder" => {
                let _ = ci.create_response(ctx, CreateInteractionResponse::Defer(
                    CreateInteractionResponseMessage::new().ephemeral(true),
                )).await;
                if !self.data.tg_clients.contains_key(&uid) {
                    let _ = ci.edit_response(ctx, EditInteractionResponse::new()
                        .embed(mk_embed("❌ ยังไม่ได้เปิดระบบ", "", C_ERR))).await;
                    return;
                }
                stop_telegram(&self.data, &uid).await;
                if let Some(mut ud) = database::get_user_data(&uid).await {
                    ud.status = "off".into();
                    let _ = database::save_user_data(&uid, &ud).await;
                }
                let _ = ci.edit_response(ctx, EditInteractionResponse::new()
                    .embed(mk_embed("✅ ปิดระบบเรียบร้อยแล้ว", "", C_OK))).await;
            }
            "menu_delete_data" => {
                let _ = ci.create_response(ctx, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .embed(mk_embed("⚠️ ยืนยันการลบข้อมูล",
                            "ข้อมูล Telegram, session, และกลุ่มทั้งหมดจะถูกลบถาวร", C_WARN))
                        .components(confirm_delete_components()).ephemeral(true),
                )).await;
            }
            "confirm_delete_yes" => {
                let _ = ci.create_response(ctx, CreateInteractionResponse::Defer(
                    CreateInteractionResponseMessage::new().ephemeral(true),
                )).await;
                stop_telegram(&self.data, &uid).await;
                let _ = database::delete_user_data(&uid).await;
                let _ = ci.edit_response(ctx, EditInteractionResponse::new()
                    .embed(mk_embed("✅ ลบข้อมูลเรียบร้อยแล้ว", "", C_OK))
                    .components(vec![])).await;
            }
            "confirm_delete_no" => {
                let _ = ci.create_response(ctx, CreateInteractionResponse::Defer(
                    CreateInteractionResponseMessage::new().ephemeral(true),
                )).await;
                let _ = ci.edit_response(ctx, EditInteractionResponse::new()
                    .embed(mk_embed("❌ ยกเลิกการลบข้อมูล", "", C_INFO))
                    .components(vec![])).await;
            }
            "otp_enter" => {
                let _ = ci.create_response(ctx, CreateInteractionResponse::Modal(otp_modal())).await;
            }
            _ => {}
        }
    }

    async fn handle_modal(&self, ctx: &Context, mi: ModalInteraction) {
        let uid  = mi.user.id.to_string();
        let vals = parse_modal_values(&mi);
        match mi.data.custom_id.as_str() {
            "modal_telegram" => {
                let mut ud = database::get_user_data(&uid).await.unwrap_or_default();
                ud.api_id         = vals.get("api_id")  .cloned().unwrap_or_default().trim().to_string();
                ud.api_hash       = vals.get("api_hash") .cloned().unwrap_or_default().trim().to_string();
                ud.telegram_phone = vals.get("phone")    .cloned().unwrap_or_default().trim().to_string();
                let _ = database::save_user_data(&uid, &ud).await;
                let _ = mi.create_response(ctx, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .embed(mk_embed("✅ บันทึกข้อมูล Telegram เรียบร้อยแล้ว", "", C_OK))
                        .ephemeral(true),
                )).await;
            }
            "modal_webhook" => {
                let url = vals.get("webhook").cloned().unwrap_or_default().trim().to_string();
                let valid = url.starts_with("https://discord.com/api/webhooks/")
                         || url.starts_with("https://discordapp.com/api/webhooks/");
                if !valid {
                    let _ = mi.create_response(ctx, CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .embed(mk_embed("❌ รูปแบบ Webhook ไม่ถูกต้อง",
                                "ต้องเป็น `https://discord.com/api/webhooks/...`", C_ERR))
                            .ephemeral(true),
                    )).await;
                    return;
                }
                let mut ud = database::get_user_data(&uid).await.unwrap_or_default();
                ud.webhook = url;
                let _ = database::save_user_data(&uid, &ud).await;
                let _ = mi.create_response(ctx, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .embed(mk_embed("✅ บันทึก Webhook เรียบร้อยแล้ว", "", C_OK))
                        .ephemeral(true),
                )).await;
            }
            "modal_otp" => {
                let otp = vals.get("otp").cloned().unwrap_or_default().trim().to_string();
                let _ = mi.create_response(ctx, CreateInteractionResponse::Defer(
                    CreateInteractionResponseMessage::new().ephemeral(true),
                )).await;
                match start_telegram_otp(self.data.clone(), uid.clone(), otp).await {
                    Ok(_) => {
                        let _ = mi.edit_response(ctx, EditInteractionResponse::new()
                            .embed(mk_embed("✅ เปิดระบบเรียบร้อยแล้ว",
                                "📡 กำลังหากลุ่ม + สแกนซอง...", C_OK))).await;
                    }
                    Err(e) => {
                        let _ = mi.edit_response(ctx, EditInteractionResponse::new()
                            .embed(mk_embed("❌ OTP ไม่ถูกต้องหรือหมดอายุ",
                                &format!("```{}```\n\nกดปุ่ม **เริ่มหากลุ่ม** ใหม่อีกครั้ง", e),
                                C_ERR))).await;
                    }
                }
            }
            _ => {}
        }
    }
}

fn parse_modal_values(mi: &ModalInteraction) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for row in &mi.data.components {
        for comp in &row.components {
            if let ActionRowComponent::InputText(input) = comp {
                if let Some(v) = &input.value {
                    map.insert(input.custom_id.clone(), v.clone());
                }
            }
        }
    }
    map
}

fn main_embed() -> CreateEmbed {
    mk_embed_footer(
        CreateEmbed::default()
            .title("⛏️ Auto Telegram Group Finder")
            .description("**ระบบหากลุ่ม Telegram + สแกนซอง TrueMoney**\n\
                 • ค้นหากลุ่มด้วยคีย์เวิร์ดและลิงก์ t.me อัตโนมัติ\n\
                 • สแกนหาซอง TrueMoney ทุกกลุ่ม\n\
                 • แจ้งเตือนผ่าน Webhook ทันที | 24/7")
            .color(0xFF0000_u32)
            .field("⚙️ ฟีเจอร์",
                "```ini\n[✔] ขุดกลุ่มอัตโนมัติ\n[✔] สแกนหาซอง TrueMoney\n[✔] Webhook แจ้งเตือน\n[✔] ไม่จำกัดเวลา\n```",
                true)
            .image("https://s13.gifyu.com/images/b7MfF.gif")
            .timestamp(Timestamp::now()),
    )
}

fn main_components() -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![
        CreateButton::new("main_open_menu").label("เปิดเมนู").emoji('⚙').style(ButtonStyle::Success),
        CreateButton::new("main_check_status").label("เช็คสถานะ").emoji('📊').style(ButtonStyle::Primary),
    ])]
}

fn menu_embed() -> CreateEmbed {
    mk_embed_footer(
        CreateEmbed::default()
            .title("⚙️ SETTING")
            .description("```\n[🔑] บัญชี Telegram\n[🔔] Webhook\n[🔍] เริ่มหากลุ่ม + สแกนซอง\n[🔴] ปิดระบบ\n[💀] ลบข้อมูล\n```")
            .color(C_INFO),
    )
}

fn menu_components() -> Vec<CreateActionRow> {
    vec![
        CreateActionRow::Buttons(vec![
            CreateButton::new("menu_set_telegram").label("บัญชี Telegram").emoji('🔑').style(ButtonStyle::Secondary),
            CreateButton::new("menu_set_webhook").label("Webhook").emoji('🔔').style(ButtonStyle::Secondary),]),
        CreateActionRow::Buttons(vec![
            CreateButton::new("menu_start_finder").label("เริ่มหากลุ่ม").emoji('🔍').style(ButtonStyle::Success),
            CreateButton::new("menu_stop_finder").label("ปิดระบบ").emoji('🔴').style(ButtonStyle::Danger),
            CreateButton::new("menu_delete_data").label("ลบข้อมูล").emoji('💀').style(ButtonStyle::Danger),
        ]),
    ]
}

fn confirm_delete_components() -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![
        CreateButton::new("confirm_delete_yes").label("ยืนยันลบข้อมูล").emoji('💀').style(ButtonStyle::Danger),
        CreateButton::new("confirm_delete_no").label("ยกเลิก").emoji('❌').style(ButtonStyle::Secondary),
    ])]
}

fn otp_components() -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![
        CreateButton::new("otp_enter").label("ใส่รหัส OTP").emoji('📟').style(ButtonStyle::Primary),
    ])]
}

fn telegram_modal() -> CreateModal {
    CreateModal::new("modal_telegram", "ตั้งค่าบัญชี Telegram").components(vec![
        CreateActionRow::InputText(
            CreateInputText::new(InputTextStyle::Short, "API ID", "api_id")
                .placeholder("12345678").required(true),
        ),
        CreateActionRow::InputText(
            CreateInputText::new(InputTextStyle::Short, "API Hash", "api_hash")
                .placeholder("abcdef1234567890abcdef").required(true),
        ),
        CreateActionRow::InputText(
            CreateInputText::new(InputTextStyle::Short, "เบอร์โทรศัพท์ (+66...)", "phone")
                .placeholder("+66812345678").required(true),
        ),
    ])
}

fn webhook_modal() -> CreateModal {
    CreateModal::new("modal_webhook", "ตั้งค่า Webhook URL").components(vec![
        CreateActionRow::InputText(
            CreateInputText::new(InputTextStyle::Short, "Discord Webhook URL", "webhook")
                .placeholder("https://discord.com/api/webhooks/...").required(true),
        ),
    ])
}

fn otp_modal() -> CreateModal {
    CreateModal::new("modal_otp", "กรุณาใส่รหัส OTP").components(vec![
        CreateActionRow::InputText(
            CreateInputText::new(InputTextStyle::Short, "รหัส OTP จาก Telegram", "otp")
                .placeholder("12345").required(true),
        ),
    ])
}

fn is_admin(uid: &str) -> bool {
    config::ADMIN_IDS.contains(&uid)
}

#[poise::command(slash_command, rename = "setup_finder")]
async fn cmd_setup(
    ctx: PoiseCtx<'_>,
    #[description = "ช่องที่จะส่งเมนู"] channel: serenity::Channel,
) -> Result<()> {
    if !is_admin(&ctx.author().id.to_string()) {
        ctx.send(poise::CreateReply::default()
            .embed(mk_embed("❌ ไม่มีสิทธิ์", "เฉพาะแอดมินเท่านั้น", C_ERR))
            .ephemeral(true)).await?;
        return Ok(());
    }
    let ch = channel.id();
    ch.send_message(ctx.http(), CreateMessage::new()
        .embed(main_embed()).components(main_components())).await?;
    ctx.send(poise::CreateReply::default()
        .embed(mk_embed(&format!("✅ ส่งเมนูไปยัง <#{ch}> เรียบร้อยแล้ว"), "", C_OK))
        .ephemeral(true)).await?;
    Ok(())
}

#[poise::command(slash_command, rename = "manage_users")]
async fn cmd_users(ctx: PoiseCtx<'_>) -> Result<()> {
    if !is_admin(&ctx.author().id.to_string()) {
        ctx.send(poise::CreateReply::default()
            .embed(mk_embed("❌ ไม่มีสิทธิ์", "", C_ERR)).ephemeral(true)).await?;
        return Ok(());
    }
    let data  = ctx.data();
    let users = database::get_all_users().await;
    let lines: Vec<String> = users.iter().enumerate().map(|(i, uid)| {
        let s = if data.tg_clients.contains_key(uid.as_str()) { "🟢 ONLINE" } else { "🔴 OFFLINE" };
        format!("**{}.** <@{}> — {}", i + 1, uid, s)
    }).collect();
    let desc = if lines.is_empty() { "```ยังไม่มีผู้ใช้งาน```".into() } else { lines.join("\n") };
    let e = CreateEmbed::default().title("👥 จัดการผู้ใช้งาน").description(desc).color(C_INFO)
        .field("📊 สถิติ",
            format!("ทั้งหมด: **{}** | เปิดอยู่: **{}/{}**",
                users.len(), data.tg_clients.len(), config::MAX_USERS), false);
    ctx.send(poise::CreateReply::default().embed(e).ephemeral(true)).await?;
    Ok(())
}

#[poise::command(slash_command, rename = "system_info")]
async fn cmd_system(ctx: PoiseCtx<'_>) -> Result<()> {
    if !is_admin(&ctx.author().id.to_string()) {
        ctx.send(poise::CreateReply::default()
            .embed(mk_embed("❌ ไม่มีสิทธิ์", "", C_ERR)).ephemeral(true)).await?;
        return Ok(());
    }
    let data   = ctx.data();
    let uptime = data.start_time.elapsed().as_secs();
    let mem    = database::get_memory_usage();
    let e = CreateEmbed::default().title("📊 System Info").color(C_OK)
        .field("⏱️ Uptime", format!("`{}h {}m`", uptime / 3600, (uptime % 3600) / 60), true)
        .field("💾 Memory", format!("`{} MB`", mem.rss_mb),                true)
        .field("👥 ผู้ใช้", format!("`{}/{}`", data.tg_clients.len(), config::MAX_USERS), true)
        .field("🦀 Rust",   format!("`v{}`",   env!("CARGO_PKG_VERSION")),               true)
        .timestamp(Timestamp::now());
    ctx.send(poise::CreateReply::default().embed(e).ephemeral(true)).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("digtg_ai=info,serenity=warn,poise=warn")
        .init();
    if config::TOKEN == "YOUR_DISCORD_BOT_TOKEN" {
        eprintln!("[Error] กรุณาใส่โทเค่นบอทใน src/config.rs แล้ว build ใหม่");
        std::process::exit(1);
    }
    database::init_database().await?;
    let shared = BotData {
        tg_clients:  Arc::new(DashMap::new()),
        otp_pending: Arc::new(DashMap::new()),
        start_time:  Arc::new(Instant::now()),
    };
    println!("╔═══════════════════════════════════╗");
    println!("║  ⛏️  AUTO TELEGRAM GROUP FINDER    ║");
    println!("║     เข้าทุกกลุ่ม + สแกนหาซอง     ║");
    println!("╚═══════════════════════════════════╝");
    tokio::spawn(server::run_server(8080));
    let data_for_poise   = shared.clone();
    let data_for_handler = shared.clone();
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![cmd_setup(), cmd_users(), cmd_system()],
            ..Default::default()
        })
        .setup(move |ctx, _ready, fw| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &fw.options().commands).await?;
                Ok(data_for_poise)
            })
        })
        .build();
    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let mut client = serenity::ClientBuilder::new(config::TOKEN, intents)
        .framework(framework)
        .event_handler(Handler { data: data_for_handler })
        .await?;
    client.start().await?;
    Ok(())
}
