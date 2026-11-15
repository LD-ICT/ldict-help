//! The glue between RustDesk and LD ICT's website (PLAN-HULP-OP-AFSTAND.md, section 3.2 points 3 to 6).
//!
//! - The technician's program: an `ldict-help://connection/new/<id>?password=ldict1....` link from the admin carries
//!   the connection token. RustDesk hands it over as a preset password; we keep it aside (it is never hashed or sent
//!   as a password) and put it in the login request.
//! - The customer's program: every incoming login must carry a valid token for this device (token.rs). A session
//!   starts without "Accepteren" only when the token says so and this device's unattended access is on at
//!   www.ldict.nl, checked again right then. Start, a heartbeat every minute and the end of each session go to
//!   /api/hulp/sessies/melding with the token; the answer says whether unattended access is still on, so switching it
//!   off ends a running unattended session within a minute.
//! - Every hour (and at start) the program tells /api/hulp/apparaten/status its version, system and name, and reads
//!   whether unattended access is on. Without an answer it counts as off.

use super::token::{self, Grant};
use hbb_common::{config::Config, lazy_static, log, serde_json, tokio};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const API: &str = "https://www.ldict.nl/api/hulp";

/// LD ICT's public key for connection tokens, base64 (the admin shows it on /admin/servers/hulp). Not secret.
pub const TOKEN_KEY: &str = include_str!("../../ldict/token.pub");

const HEARTBEAT: Duration = Duration::from_secs(60);
const STATUS_EVERY: Duration = Duration::from_secs(3600);

struct Incoming {
    token: String,
    grant: Grant,
    kind: &'static str,
    started: bool,
    without_click: bool,
    last_beat: Instant,
}

lazy_static::lazy_static! {
    static ref OUTGOING: Mutex<HashMap<String, String>> = Default::default();
    static ref INCOMING: Mutex<HashMap<i32, Incoming>> = Default::default();
    /// Sessions that started here, so the technician's program can reconnect after a network hiccup once the
    /// short-lived token has expired (within token::REPORT_WINDOW of its issue).
    static ref SEEN: Mutex<HashMap<String, i64>> = Default::default();
}

/// Whether www.ldict.nl last said unattended access is on for this device.
static UNATTENDED: AtomicBool = AtomicBool::new(false);

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// RustDesk's version plus our build number, like 1.5.3-2 (the version in the release feed).
pub fn full_version() -> String {
    format!("{}-{}", crate::VERSION, super::BUILD)
}

// ---- The technician's program ----

/// Keeps a token that came in as the preset password of a link. Returns whether it was one.
pub fn remember_outgoing(peer_id: &str, preset: &str) -> bool {
    if !preset.starts_with(token::PREFIX) {
        return false;
    }
    OUTGOING.lock().unwrap().insert(peer_id.to_owned(), preset.to_owned());
    true
}

/// The token for a login request to this peer, or empty.
pub fn outgoing_token(peer_id: &str) -> String {
    OUTGOING.lock().unwrap().get(peer_id).cloned().unwrap_or_default()
}

// ---- The customer's program ----

/// Lets a login in only with a valid token for this device. The error is shown to the technician.
pub async fn admit(conn_id: i32, token: &str) -> Result<(), &'static str> {
    let id = Config::get_id();
    let grant = match token::verify(token, TOKEN_KEY, &id, now()) {
        Ok(grant) => grant,
        Err(token::TokenError::Expired) => {
            let grant = token::read(token, TOKEN_KEY).map_err(|e| e.message())?;
            let seen = SEEN.lock().unwrap().contains_key(&grant.session);
            if grant.device != id || !seen || now() > grant.issued + token::REPORT_WINDOW {
                return Err(token::TokenError::Expired.message());
            }
            grant
        }
        Err(err) => return Err(err.message()),
    };
    if grant.unattended {
        // Ask again right now: the customer may have switched it on or off a moment ago.
        refresh_status().await;
    }
    log::info!("LD ICT session {} by {} admitted (unattended token: {})", grant.session, grant.technician, grant.unattended);
    INCOMING.lock().unwrap().insert(
        conn_id,
        Incoming { token: token.to_owned(), grant, kind: "desktop", started: false, without_click: false, last_beat: Instant::now() },
    );
    Ok(())
}

/// Whether this admitted login may start without "Accepteren": the token and the device both say yes.
pub fn take_unattended(conn_id: i32) -> bool {
    let mut incoming = INCOMING.lock().unwrap();
    match incoming.get_mut(&conn_id) {
        Some(conn) if conn.grant.unattended && UNATTENDED.load(Ordering::SeqCst) => {
            conn.without_click = true;
            true
        }
        _ => false,
    }
}

/// The session is on (after "Accepteren" or unattended).
pub fn started(conn_id: i32, kind: &'static str) {
    let report = {
        let mut incoming = INCOMING.lock().unwrap();
        let Some(conn) = incoming.get_mut(&conn_id) else { return };
        if conn.started {
            return;
        }
        conn.started = true;
        conn.kind = kind;
        let mut seen = SEEN.lock().unwrap();
        seen.retain(|_, issued| now() <= *issued + token::REPORT_WINDOW);
        seen.insert(conn.grant.session.clone(), conn.grant.issued);
        conn.last_beat = Instant::now();
        (conn.token.clone(), conn.kind, conn.without_click)
    };
    spawn_report("start", report);
}

/// Called every second while a connection is open. False: end it, because unattended access was switched off.
pub fn tick(conn_id: i32) -> bool {
    let report = {
        let mut incoming = INCOMING.lock().unwrap();
        let Some(conn) = incoming.get_mut(&conn_id) else { return true };
        if conn.without_click && !UNATTENDED.load(Ordering::SeqCst) {
            log::info!("LD ICT session {}: unattended access was switched off, ending it", conn.grant.session);
            return false;
        }
        if !conn.started || conn.last_beat.elapsed() < HEARTBEAT {
            return true;
        }
        conn.last_beat = Instant::now();
        (conn.token.clone(), conn.kind, conn.without_click)
    };
    spawn_report("beat", report);
    true
}

/// The connection closed.
pub fn ended(conn_id: i32) {
    let Some(conn) = INCOMING.lock().unwrap().remove(&conn_id) else { return };
    if conn.started {
        spawn_report("end", (conn.token, conn.kind, conn.without_click));
    }
}

fn spawn_report(event: &'static str, (token, kind, without_click): (String, &'static str, bool)) {
    let body = serde_json::json!({
        "token": token,
        "event": event,
        "kind": kind,
        "unattended": without_click,
        "version": full_version(),
    })
    .to_string();
    let send = async move {
        match crate::common::post_request(format!("{}/sessies/melding", API), body, "").await {
            Ok(text) => read_unattended(&text),
            Err(err) => log::warn!("LD ICT session report ({}) failed: {}", event, err),
        }
    };
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(send);
    } else {
        std::thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() {
                rt.block_on(send);
            }
        });
    }
}

fn read_unattended(text: &str) {
    let value: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
    if let Some(on) = value["unattended"].as_bool() {
        UNATTENDED.store(on, Ordering::SeqCst);
    }
}

fn status_body() -> String {
    serde_json::json!({
        "id": Config::get_id(),
        "version": full_version(),
        "platform": hbb_common::whoami::platform().to_string(),
        "hostname": crate::common::hostname(),
    })
    .to_string()
}

async fn refresh_status() {
    match crate::common::post_request(format!("{}/apparaten/status", API), status_body(), "").await {
        Ok(text) => {
            let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            UNATTENDED.store(value["unattended"].as_bool().unwrap_or(false), Ordering::SeqCst);
        }
        Err(err) => {
            log::warn!("LD ICT device status failed: {}", err);
            UNATTENDED.store(false, Ordering::SeqCst);
        }
    }
}

/// Starts the hourly status report, once per process (from server.rs, where the program starts listening).
pub fn start_status_reports() {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| loop {
        if let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() {
            rt.block_on(refresh_status());
        }
        std::thread::sleep(STATUS_EVERY);
    });
}
