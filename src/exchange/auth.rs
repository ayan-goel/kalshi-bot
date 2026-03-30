use anyhow::{Context, Result};
use base64::Engine;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::pss::BlindedSigningKey;
use rsa::sha2::Sha256;
use rsa::signature::RandomizedSigner;
use rsa::RsaPrivateKey;

/// Normalize a PEM string that may have come from an environment variable.
///
/// Environment variables can encode newlines as:
///   - Actual newlines (ideal)
///   - Literal `\n` two-character sequences (common in Railway / shell exports)
///   - `\r\n` Windows-style line endings
///   - The whole value wrapped in surrounding quotes
///
/// We reconstruct the PEM canonically: extract the base64 payload, strip all
/// whitespace from it, re-chunk it into 64-char lines, and wrap it back in the
/// original header/footer. This is immune to all encoding variations.
fn normalize_pem(raw: &str) -> String {
    // Strip surrounding quotes that some env systems add
    let s = raw.trim().trim_matches('"').trim_matches('\'');

    // Replace literal \n with real newlines, then normalise \r\n -> \n
    let s = s.replace("\\n", "\n").replace("\r\n", "\n");

    // Detect header line
    let (header, footer) = if s.contains("RSA PRIVATE KEY") {
        ("-----BEGIN RSA PRIVATE KEY-----", "-----END RSA PRIVATE KEY-----")
    } else {
        ("-----BEGIN PRIVATE KEY-----", "-----END PRIVATE KEY-----")
    };

    // Extract just the base64 payload (everything between header and footer)
    let b64: String = s
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("-----") && !l.is_empty())
        .collect();

    // Re-chunk into standard 64-char lines
    let body = b64
        .as_bytes()
        .chunks(64)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");

    format!("{header}\n{body}\n{footer}\n")
}

#[derive(Clone)]
pub struct KalshiAuth {
    api_key: String,
    signing_key: BlindedSigningKey<Sha256>,
}

impl KalshiAuth {
    pub fn new(api_key: String, private_key_path: &str) -> Result<Self> {
        let key_pem = std::fs::read_to_string(private_key_path)
            .with_context(|| format!("Failed to read private key from {private_key_path}"))?;
        Self::from_pem(api_key, &key_pem)
    }

    pub fn from_base64(api_key: String, base64_key: &str) -> Result<Self> {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(base64_key)
            .context("Failed to decode base64 private key")?;
        let key_pem =
            String::from_utf8(decoded).context("Decoded private key is not valid UTF-8")?;
        Self::from_pem(api_key, &key_pem)
    }

    fn from_pem(api_key: String, pem: &str) -> Result<Self> {
        let normalized = normalize_pem(pem);
        // Try PKCS#8 first, then fall back to PKCS#1
        let private_key = RsaPrivateKey::from_pkcs8_pem(&normalized)
            .or_else(|_| RsaPrivateKey::from_pkcs1_pem(&normalized))
            .context("Failed to parse RSA private key (tried PKCS#8 and PKCS#1 PEM)")?;
        let signing_key = BlindedSigningKey::<Sha256>::new(private_key);
        Ok(Self {
            api_key,
            signing_key,
        })
    }

    /// Resolve auth from config, checking env vars in priority order:
    /// 1. KALSHI_PRIVATE_KEY (raw PEM in env var)
    /// 2. KALSHI_PRIVATE_KEY_BASE64 (base64-encoded PEM)
    /// 3. KALSHI_PRIVATE_KEY_PATH (file path)
    pub fn from_config(config: &crate::config::AppConfig) -> Result<Self> {
        let api_key = config.api_key()?;

        // 1. Try raw PEM from env var
        if let Ok(raw_pem) = std::env::var("KALSHI_PRIVATE_KEY") {
            return Self::from_pem(api_key, &raw_pem);
        }

        // 2. Try base64
        if let Ok(b64) = std::env::var("KALSHI_PRIVATE_KEY_BASE64") {
            return Self::from_base64(api_key, &b64);
        }

        // 3. Try file path
        if let Ok(path) = std::env::var(&config.exchange.private_key_path_env) {
            return Self::new(api_key, &path);
        }

        anyhow::bail!(
            "No private key found. Set one of: KALSHI_PRIVATE_KEY, KALSHI_PRIVATE_KEY_BASE64, or {}",
            config.exchange.private_key_path_env
        )
    }

    pub fn sign_request(&self, method: &str, path: &str) -> AuthHeaders {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();

        let sign_path = path.split('?').next().unwrap_or(path);
        let message = format!("{}{}{}", timestamp, method, sign_path);

        let mut rng = rand::thread_rng();
        let signature = self.signing_key.sign_with_rng(&mut rng, message.as_bytes());
        let sig_bytes: Box<[u8]> = signature.into();
        let sig_b64 = base64::engine::general_purpose::STANDARD.encode(&*sig_bytes);

        AuthHeaders {
            api_key: self.api_key.clone(),
            timestamp,
            signature: sig_b64,
        }
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }
}

impl std::fmt::Debug for KalshiAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KalshiAuth")
            .field("api_key", &self.api_key)
            .field("signing_key", &"[REDACTED]")
            .finish()
    }
}

pub struct AuthHeaders {
    pub api_key: String,
    pub timestamp: String,
    pub signature: String,
}
