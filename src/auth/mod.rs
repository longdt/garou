//! JWT authentication validation for the Garou chat server.
//!
//! Provides `AuthValidator` for validating HS256 and RS256 JWTs.
//! Redis claim caching is wired in Unit 6; this unit does inline validation only.

use std::fs;

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::config::AuthSettings;
use crate::error::{ChatError, Result};
use crate::protocol::messages::UserId;

// ---------------------------------------------------------------------------
// Claims
// ---------------------------------------------------------------------------

/// JWT claims decoded from a bearer token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    /// Subject — the user's numeric ID serialised as a string.
    pub sub: String,
    /// Display name of the authenticated user.
    pub username: String,
    /// Expiry as Unix timestamp (seconds).
    pub exp: u64,
    /// Issued-at as Unix timestamp (seconds).
    pub iat: u64,
}

impl AuthClaims {
    /// Parse the numeric user ID out of `sub`.
    pub fn user_id(&self) -> Result<UserId> {
        self.sub.parse::<UserId>().map_err(|_| {
            ChatError::auth(format!("invalid user_id in JWT sub: '{}'", self.sub))
        })
    }
}

// ---------------------------------------------------------------------------
// Validator
// ---------------------------------------------------------------------------

/// Validates JWT bearer tokens and returns decoded `AuthClaims`.
///
/// Supports HS256 (shared secret) and RS256 (RSA public key).
/// Redis claim caching will be injected in Unit 6.
pub struct AuthValidator {
    decoding_key: DecodingKey,
    validation: Validation,
}

impl std::fmt::Debug for AuthValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthValidator")
            .field("algorithm", &self.validation.algorithms)
            .finish_non_exhaustive()
    }
}

impl AuthValidator {
    /// Create an HS256 validator from a shared secret.
    pub fn new_hs256(secret: impl AsRef<[u8]>) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        // Require `sub` and `exp` in every token.
        validation.required_spec_claims = ["sub".to_string(), "exp".to_string()].into();
        Self {
            decoding_key: DecodingKey::from_secret(secret.as_ref()),
            validation,
        }
    }

    /// Create an RS256 validator from a PEM-encoded RSA public key.
    pub fn new_rs256(public_key_pem: &str) -> Result<Self> {
        let decoding_key = DecodingKey::from_rsa_pem(public_key_pem.as_bytes())
            .map_err(|e| ChatError::Config(format!("invalid RS256 public key: {}", e)))?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = true;
        validation.required_spec_claims = ["sub".to_string(), "exp".to_string()].into();
        Ok(Self { decoding_key, validation })
    }

    /// Build a validator from application `AuthSettings`.
    ///
    /// Falls back to an insecure dev-only secret when `jwt_secret` is empty (logs a
    /// warning).  Always set a real secret in production.
    pub fn from_config(cfg: &AuthSettings) -> Result<Self> {
        match cfg.jwt_algorithm.as_str() {
            "RS256" => {
                if cfg.jwt_public_key_path.is_empty() {
                    return Err(ChatError::Config(
                        "auth.jwt_public_key_path is required for RS256".into(),
                    ));
                }
                let pem = fs::read_to_string(&cfg.jwt_public_key_path).map_err(|e| {
                    ChatError::Config(format!(
                        "failed to read RS256 public key '{}': {}",
                        cfg.jwt_public_key_path, e
                    ))
                })?;
                Self::new_rs256(&pem)
            }
            "HS256" | "" => {
                let secret: Vec<u8> = if cfg.jwt_secret.is_empty() {
                    warn!(
                        "auth.jwt_secret is empty — using insecure dev default. \
                         Set auth.jwt_secret in config.toml for production."
                    );
                    b"garou-dev-only-insecure-secret-do-not-use-in-production".to_vec()
                } else {
                    cfg.jwt_secret.as_bytes().to_vec()
                };
                Ok(Self::new_hs256(&secret))
            }
            other => Err(ChatError::Config(format!(
                "unsupported auth.jwt_algorithm '{}': use HS256 or RS256",
                other
            ))),
        }
    }

    /// Validate a JWT and return its decoded claims.
    ///
    /// Returns `Err(ChatError::Auth)` with a specific reason:
    /// - "token expired" — `exp` claim is in the past.
    /// - "invalid token signature" — HMAC/RSA signature mismatch.
    /// - "invalid token: …" — any other parse or validation failure.
    pub fn validate(&self, token: &str) -> Result<AuthClaims> {
        decode::<AuthClaims>(token, &self.decoding_key, &self.validation)
            .map(|data| data.claims)
            .map_err(|e| {
                use jsonwebtoken::errors::ErrorKind;
                match e.kind() {
                    ErrorKind::ExpiredSignature => ChatError::auth("token expired"),
                    ErrorKind::InvalidSignature => ChatError::auth("invalid token signature"),
                    _ => ChatError::auth(format!("invalid token: {}", e)),
                }
            })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn make_claims(user_id: u64, exp_offset_secs: i64) -> AuthClaims {
        let now = now_secs();
        let exp = if exp_offset_secs >= 0 {
            now + exp_offset_secs as u64
        } else {
            now.saturating_sub((-exp_offset_secs) as u64)
        };
        AuthClaims {
            sub: user_id.to_string(),
            username: "alice".to_string(),
            exp,
            iat: now,
        }
    }

    fn encode_hs256(claims: &AuthClaims, secret: &[u8]) -> String {
        encode(&Header::default(), claims, &EncodingKey::from_secret(secret)).unwrap()
    }

    // -----------------------------------------------------------------------
    // HS256
    // -----------------------------------------------------------------------

    #[test]
    fn test_hs256_valid_token() {
        let secret = b"test-secret";
        let validator = AuthValidator::new_hs256(secret);
        let claims = make_claims(42, 3600);
        let token = encode_hs256(&claims, secret);

        let decoded = validator.validate(&token).expect("valid token should decode");
        assert_eq!(decoded.user_id().unwrap(), 42);
        assert_eq!(decoded.username, "alice");
    }

    #[test]
    fn test_hs256_invalid_signature() {
        let validator = AuthValidator::new_hs256(b"correct-secret");
        let claims = make_claims(1, 3600);
        // Encode with a different secret
        let token = encode_hs256(&claims, b"wrong-secret");

        let err = validator.validate(&token).unwrap_err();
        assert!(
            err.to_string().contains("invalid token"),
            "expected auth error, got: {}",
            err
        );
    }

    #[test]
    fn test_hs256_expired_token() {
        let secret = b"test-secret";
        let validator = AuthValidator::new_hs256(secret);
        // exp = 120 seconds ago (well past the 60-second leeway)
        let claims = make_claims(1, -120);
        let token = encode_hs256(&claims, secret);

        let err = validator.validate(&token).unwrap_err();
        assert!(
            err.to_string().contains("expired"),
            "expected expiry error, got: {}",
            err
        );
    }

    #[test]
    fn test_hs256_malformed_token() {
        let validator = AuthValidator::new_hs256(b"secret");
        let err = validator.validate("this.is.not.a.jwt").unwrap_err();
        assert!(matches!(err, ChatError::Auth(_)));
    }

    /// Property: any token signed with a different HS256 secret always fails.
    #[test]
    fn test_wrong_secret_always_fails() {
        let correct_secret = b"correct-secret-1234";
        let validator = AuthValidator::new_hs256(correct_secret);
        let claims = make_claims(99, 3600);

        let wrong_secrets: &[&[u8]] = &[
            b"wrong-secret",
            b"CORRECT-SECRET-1234",
            b"correct-secret-123",
            b"correct-secret-12345",
            b"",
            b"\x00",
        ];

        for &ws in wrong_secrets {
            let token = encode_hs256(&claims, ws);
            assert!(
                validator.validate(&token).is_err(),
                "token signed with {:?} should fail validation against correct secret",
                ws
            );
        }
    }

    // -----------------------------------------------------------------------
    // RS256
    // -----------------------------------------------------------------------

    /// Verify that `new_rs256` rejects non-PEM input.
    #[test]
    fn test_rs256_invalid_pem_rejected() {
        let err = AuthValidator::new_rs256("this is not a pem").unwrap_err();
        assert!(matches!(err, ChatError::Config(_)));
    }

    // -----------------------------------------------------------------------
    // AuthClaims helpers
    // -----------------------------------------------------------------------

    #[test]
    fn test_claims_user_id_parsing() {
        let claims = AuthClaims {
            sub: "12345".to_string(),
            username: "bob".to_string(),
            exp: now_secs() + 3600,
            iat: now_secs(),
        };
        assert_eq!(claims.user_id().unwrap(), 12345u64);
    }

    #[test]
    fn test_claims_invalid_user_id() {
        let claims = AuthClaims {
            sub: "not-a-number".to_string(),
            username: "bob".to_string(),
            exp: now_secs() + 3600,
            iat: now_secs(),
        };
        assert!(claims.user_id().is_err());
    }

    // -----------------------------------------------------------------------
    // from_config
    // -----------------------------------------------------------------------

    #[test]
    fn test_from_config_hs256() {
        let cfg = AuthSettings {
            jwt_secret: "my-secret".to_string(),
            jwt_public_key_path: String::new(),
            jwt_algorithm: "HS256".to_string(),
        };
        let validator = AuthValidator::from_config(&cfg).expect("should build");

        let claims = make_claims(7, 3600);
        let token = encode_hs256(&claims, b"my-secret");
        assert!(validator.validate(&token).is_ok());
    }

    #[test]
    fn test_from_config_empty_secret_uses_dev_default() {
        let cfg = AuthSettings::default();
        // Should not error — falls back to dev default
        let validator = AuthValidator::from_config(&cfg).expect("should build with dev fallback");
        // The dev default should reject a token signed with a different key
        let claims = make_claims(1, 3600);
        let token = encode_hs256(&claims, b"some-other-secret");
        assert!(validator.validate(&token).is_err());
    }

    #[test]
    fn test_from_config_unsupported_algorithm() {
        let cfg = AuthSettings {
            jwt_secret: "secret".to_string(),
            jwt_public_key_path: String::new(),
            jwt_algorithm: "ES256".to_string(),
        };
        let err = AuthValidator::from_config(&cfg).unwrap_err();
        assert!(matches!(err, ChatError::Config(_)));
    }
}
