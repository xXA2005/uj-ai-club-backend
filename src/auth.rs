use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts},
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::env;
use uuid::Uuid;

use crate::error::AppError;

static KEYS: Lazy<Keys> = Lazy::new(|| {
    let secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    Keys::new(secret.as_bytes())
});

struct Keys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

impl Keys {
    fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
}

impl Claims {
    pub fn new(user_id: Uuid) -> Self {
        Self {
            sub: user_id.to_string(),
            exp: (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp(),
        }
    }
}

pub fn create_token(user_id: Uuid) -> Result<String, AppError> {
    encode(&Header::default(), &Claims::new(user_id), &KEYS.encoding)
        .map_err(|e| AppError::InternalError(e.into()))
}

pub struct AuthUser {
    pub user_id: Uuid,
}

pub struct AdminUser {
    pub user_id: Uuid,
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let bearer = parts
            .headers
            .get(AUTHORIZATION)
            .ok_or(AppError::AuthError)?
            .to_str()
            .map_err(|_| AppError::AuthError)?
            .strip_prefix("Bearer ")
            .ok_or(AppError::AuthError)?;

        let token_data = decode::<Claims>(bearer, &KEYS.decoding, &Validation::default())
            .map_err(|_| AppError::AuthError)?;

        let user_id = Uuid::parse_str(&token_data.claims.sub).map_err(|_| AppError::AuthError)?;

        Ok(Self { user_id })
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for AdminUser
where
    S: Send + Sync,
    PgPool: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let bearer = parts
            .headers
            .get(AUTHORIZATION)
            .ok_or(AppError::AuthError)?
            .to_str()
            .map_err(|_| AppError::AuthError)?
            .strip_prefix("Bearer ")
            .ok_or(AppError::AuthError)?;

        let token_data = decode::<Claims>(bearer, &KEYS.decoding, &Validation::default())
            .map_err(|_| AppError::AuthError)?;

        let user_id = Uuid::parse_str(&token_data.claims.sub).map_err(|_| AppError::AuthError)?;

        let pool = PgPool::from_ref(state);

        let user_role: (String,) = sqlx::query_as("SELECT role FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| AppError::InternalError(e.into()))?
            .ok_or(AppError::AuthError)?;

        if user_role.0 != "admin" {
            return Err(AppError::AuthError);
        }

        Ok(Self { user_id })
    }
}
