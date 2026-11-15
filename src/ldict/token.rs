//! Connection tokens (PLAN-HULP-OP-AFSTAND.md, section 3.2 points 3 and 4).
//!
//! A technician starts a session from LD ICT's admin on www.ldict.nl (signed in with 2FA). The admin signs a short
//! token with LD ICT's own Ed25519 key: which device, which session, which technician, whether it may start without
//! a click, and until when. The technician's program sends it along in RustDesk's login request; the customer's
//! program checks it with the public key built into it (ldict/token.pub) and refuses every connection without one,
//! also with a password. The key that signs tokens is only on www.ldict.nl, never on the support server.
//!
//! Format: `ldict1.<payload>.<signature>`, both base64url without padding. The signature is over the bytes
//! `ldict1.<payload>`, so a token can't be mistaken for anything else that key might sign. The payload is JSON:
//! `{"v":1,"sid":"<session>","dev":"<RustDesk id>","tech":"<name>","una":false,"iat":<unix>,"exp":<unix>}`.
//!
//! Pure: no network, no settings. Tested here and against the website's signer (src/lib/control/support/tokens).

use hbb_common::base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use hbb_common::serde_json::{self, Value};
use hbb_common::sodiumoxide::crypto::sign::{verify_detached, PublicKey, Signature};

pub const PREFIX: &str = "ldict1.";

/// Clock difference we accept between www.ldict.nl and the customer's computer, in seconds.
const CLOCK_SKEW: i64 = 300;

/// A session may report its start, heartbeats and end this long after its token was issued, in seconds.
pub const REPORT_WINDOW: i64 = 24 * 3600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub session: String,
    pub device: String,
    pub technician: String,
    /// The token allows starting without "Accepteren". The device's own setting must say so too.
    pub unattended: bool,
    pub issued: i64,
    pub expires: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenError {
    Missing,
    NoKey,
    Malformed,
    BadSignature,
    WrongDevice,
    Expired,
}

impl TokenError {
    /// Shown to the technician (in Dutch: LD ICT's technicians are Dutch).
    pub fn message(self) -> &'static str {
        match self {
            TokenError::Missing => {
                "LD ICT Hulp accepteert alleen verbindingen die in de admin van www.ldict.nl zijn gestart (Verbinden)."
            }
            TokenError::NoKey => "Deze versie van LD ICT Hulp heeft geen tokensleutel. Installeer de nieuwste versie.",
            TokenError::Malformed | TokenError::BadSignature => {
                "Het verbindingstoken klopt niet. Start de verbinding opnieuw vanuit de admin."
            }
            TokenError::WrongDevice => "Dit verbindingstoken is voor een ander apparaat.",
            TokenError::Expired => "Het verbindingstoken is verlopen. Start de verbinding opnieuw vanuit de admin.",
        }
    }
}

fn public_key(key_b64: &str) -> Result<PublicKey, TokenError> {
    let key = key_b64.trim();
    if key.is_empty() {
        return Err(TokenError::NoKey);
    }
    let bytes = STANDARD.decode(key).map_err(|_| TokenError::NoKey)?;
    PublicKey::from_slice(&bytes).ok_or(TokenError::NoKey)
}

/// Checks the signature and reads the claims, without looking at the device or the time.
pub fn read(token: &str, key_b64: &str) -> Result<Grant, TokenError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(TokenError::Missing);
    }
    let pk = public_key(key_b64)?;
    let rest = token.strip_prefix(PREFIX).ok_or(TokenError::Malformed)?;
    let (payload_b64, sig_b64) = rest.split_once('.').ok_or(TokenError::Malformed)?;
    let sig = URL_SAFE_NO_PAD.decode(sig_b64).map_err(|_| TokenError::Malformed)?;
    let sig = Signature::from_bytes(&sig).map_err(|_| TokenError::Malformed)?;
    let signed = &token[..PREFIX.len() + payload_b64.len()];
    if !verify_detached(&sig, signed.as_bytes(), &pk) {
        return Err(TokenError::BadSignature);
    }
    let payload = URL_SAFE_NO_PAD.decode(payload_b64).map_err(|_| TokenError::Malformed)?;
    let claims: Value = serde_json::from_slice(&payload).map_err(|_| TokenError::Malformed)?;
    if claims["v"].as_i64() != Some(1) {
        return Err(TokenError::Malformed);
    }
    let text = |key: &str| claims[key].as_str().map(str::to_owned);
    let grant = Grant {
        session: text("sid").filter(|s| !s.is_empty()).ok_or(TokenError::Malformed)?,
        device: text("dev").filter(|s| !s.is_empty()).ok_or(TokenError::Malformed)?,
        technician: text("tech").unwrap_or_default(),
        unattended: claims["una"].as_bool().unwrap_or(false),
        issued: claims["iat"].as_i64().ok_or(TokenError::Malformed)?,
        expires: claims["exp"].as_i64().ok_or(TokenError::Malformed)?,
    };
    Ok(grant)
}

/// A token that lets a technician in to this device now.
pub fn verify(token: &str, key_b64: &str, device: &str, now: i64) -> Result<Grant, TokenError> {
    let grant = read(token, key_b64)?;
    if grant.device != device {
        return Err(TokenError::WrongDevice);
    }
    if now > grant.expires || grant.issued > now + CLOCK_SKEW {
        return Err(TokenError::Expired);
    }
    Ok(grant)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::sodiumoxide::crypto::sign;

    fn make(sk: &sign::SecretKey, payload: &str) -> String {
        let signed = format!("{}{}", PREFIX, URL_SAFE_NO_PAD.encode(payload));
        let sig = sign::sign_detached(signed.as_bytes(), sk);
        format!("{}.{}", signed, URL_SAFE_NO_PAD.encode(sig.to_bytes()))
    }

    fn setup() -> (String, sign::SecretKey) {
        let (pk, sk) = sign::gen_keypair();
        (STANDARD.encode(pk.as_ref()), sk)
    }

    const NOW: i64 = 1_800_000_000;

    fn payload(dev: &str, una: bool) -> String {
        format!(
            r#"{{"v":1,"sid":"0b0c","dev":"{}","tech":"Lars","una":{},"iat":{},"exp":{}}}"#,
            dev,
            una,
            NOW - 10,
            NOW + 600
        )
    }

    #[test]
    fn accepts_a_token_for_this_device() {
        let (pk, sk) = setup();
        let grant = verify(&make(&sk, &payload("123456789", true)), &pk, "123456789", NOW).unwrap();
        assert_eq!(grant.session, "0b0c");
        assert_eq!(grant.technician, "Lars");
        assert!(grant.unattended);
    }

    #[test]
    fn refuses_other_devices_expired_tokens_and_tampering() {
        let (pk, sk) = setup();
        let token = make(&sk, &payload("123456789", false));
        assert_eq!(verify(&token, &pk, "987654321", NOW), Err(TokenError::WrongDevice));
        assert_eq!(verify(&token, &pk, "123456789", NOW + 601), Err(TokenError::Expired));
        assert_eq!(verify(&token, &pk, "123456789", NOW - 400), Err(TokenError::Expired));
        let other = make(&sk, &payload("987654321", true));
        let forged = format!("{}.{}", other.rsplit_once('.').unwrap().0, token.rsplit_once('.').unwrap().1);
        assert_eq!(verify(&forged, &pk, "987654321", NOW), Err(TokenError::BadSignature));
        let (other_pk, _) = setup();
        assert_eq!(verify(&token, &other_pk, "123456789", NOW), Err(TokenError::BadSignature));
    }

    #[test]
    fn refuses_missing_tokens_keys_and_garbage() {
        let (pk, sk) = setup();
        assert_eq!(verify("", &pk, "1", NOW), Err(TokenError::Missing));
        assert_eq!(verify(&make(&sk, &payload("1", false)), "", "1", NOW), Err(TokenError::NoKey));
        assert_eq!(verify("password", &pk, "1", NOW), Err(TokenError::Malformed));
        assert_eq!(verify("ldict1.abc", &pk, "1", NOW), Err(TokenError::Malformed));
        assert_eq!(verify(&make(&sk, r#"{"v":2}"#), &pk, "1", NOW), Err(TokenError::Malformed));
    }

    // The same token as src/lib/control/support/tokens.test.ts on the website, so both sides agree on the format.
    #[test]
    fn reads_a_token_signed_by_the_website() {
        let pk = "6kpsY+KcUgq+9VB7Ey7F+ZVHdq6+vnuSQh7qaRRG0iw=";
        let token = "ldict1.eyJ2IjoxLCJzaWQiOiIwMDAwMDAwMC0wMDAwLTQwMDAtODAwMC0wMDAwMDAwMDAwMDEiLCJkZXYiOiIxMjM0NTY3ODkiLCJ0ZWNoIjoiTGFycyIsInVuYSI6ZmFsc2UsImlhdCI6MTgwMDAwMDAwMCwiZXhwIjoxODAwMDAwNjAwfQ.PQBKbfgv9oeJ5LVhgCTxDb8TWm1Y_C4HIdhu4KgoRD0k0gH8qothyoDJ5tg_vcvNSaKC6TtHQ1uuheZYuIZIBw";
        let grant = verify(token, pk, "123456789", 1_800_000_100).unwrap();
        assert_eq!(grant.session, "00000000-0000-4000-8000-000000000001");
        assert!(!grant.unattended);
    }
}
