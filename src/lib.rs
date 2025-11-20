pub mod auth;
pub mod error;
pub mod handlers;
pub mod models;

use axum::{
    Router,
    extract::FromRef,
    routing::{delete, get, patch, post, put},
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

#[derive(Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub auth_url: String,
    pub token_url: String,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub oauth_config: Arc<OAuthConfig>,
}

// Implement FromRef to allow extracting PgPool from AppState
impl FromRef<AppState> for sqlx::PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

pub fn create_app(pool: sqlx::PgPool) -> Router {
    let google_client_id = std::env::var("GOOGLE_CLIENT_ID").expect("GOOGLE_CLIENT_ID must be set");
    let google_client_secret =
        std::env::var("GOOGLE_CLIENT_SECRET").expect("GOOGLE_CLIENT_SECRET must be set");
    let google_redirect_uri =
        std::env::var("GOOGLE_REDIRECT_URI").expect("GOOGLE_REDIRECT_URI must be set");

    let oauth_config = Arc::new(OAuthConfig {
        client_id: google_client_id,
        client_secret: google_client_secret,
        redirect_uri: google_redirect_uri,
        auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
        token_url: "https://oauth2.googleapis.com/token".to_string(),
    });

    let app_state = AppState {
        pool: pool.clone(),
        oauth_config,
    };
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(handlers::health_check))
        .route("/auth/signup", post(handlers::signup))
        .route("/auth/login", post(handlers::login))
        .route("/auth/google", get(handlers::google_auth_init))
        .route("/auth/google/callback", get(handlers::google_auth_callback))
        .route("/auth/complete-profile", post(handlers::complete_profile))
        .route("/leaderboards", get(handlers::get_leaderboards))
        .route("/resources", get(handlers::get_resources))
        .route("/resources/:id", get(handlers::get_resource_by_id))
        .route("/challenges/current", get(handlers::get_current_challenge))
        .route(
            "/challenges/leaderboard",
            get(handlers::get_challenge_leaderboard),
        )
        .route(
            "/users/profile",
            put(handlers::update_user_profile).get(handlers::get_user_profile),
        )
        .route("/users/avatar", post(handlers::upload_user_avatar))
        .route("/users/password", put(handlers::update_user_password))
        .route("/contact", post(handlers::create_contact))
        .route("/admin/resources", get(handlers::admin_get_resources))
        .route(
            "/admin/resources",
            post(handlers::admin_create_resource_multipart),
        )
        .route(
            "/admin/resources/:id",
            get(handlers::admin_get_resource_by_id),
        )
        .route(
            "/admin/resources/:id",
            put(handlers::admin_update_resource_multipart),
        )
        .route(
            "/admin/resources/:id",
            delete(handlers::admin_delete_resource),
        )
        .route(
            "/admin/resources/:id/visibility",
            patch(handlers::admin_patch_resource_visibility),
        )
        .route("/admin/challenges", get(handlers::admin_get_challenges))
        .route("/admin/challenges", post(handlers::admin_create_challenge))
        .route(
            "/admin/challenges/:id",
            get(handlers::admin_get_challenge_by_id),
        )
        .route(
            "/admin/challenges/:id",
            put(handlers::admin_update_challenge),
        )
        .route(
            "/admin/challenges/:id",
            delete(handlers::admin_delete_challenge),
        )
        .route(
            "/admin/challenges/:id/visibility",
            patch(handlers::admin_patch_challenge_visibility),
        )
        .nest_service("/uploads", ServeDir::new("uploads"))
        .layer(cors)
        .with_state(app_state)
}
