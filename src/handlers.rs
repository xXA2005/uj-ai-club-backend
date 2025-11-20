use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use bcrypt::{DEFAULT_COST, hash, verify};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{AdminUser, AuthUser, create_token},
    error::AppError,
    models::*,
};

#[derive(Serialize)]
pub struct HealthResponse {
    status: String,
    database: String,
}

pub async fn health_check(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, StatusCode> {
    // Check database connection
    let db_status = match sqlx::query("SELECT 1").fetch_one(&state.pool).await {
        Ok(_) => "healthy",
        Err(_) => "unhealthy",
    };

    let response = HealthResponse {
        status: if db_status == "healthy" {
            "ok".to_string()
        } else {
            "degraded".to_string()
        },
        database: db_status.to_string(),
    };

    if db_status == "healthy" {
        Ok(Json(response))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

pub async fn signup(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let existing_user = sqlx::query("SELECT id FROM users WHERE email = $1")
        .bind(&req.email)
        .fetch_optional(&state.pool)
        .await?;

    if existing_user.is_some() {
        return Err(AppError::UserExists);
    }

    let password_hash = hash(req.password.as_bytes(), DEFAULT_COST)
        .map_err(|e| AppError::InternalError(e.into()))?;

    let user_id = Uuid::new_v4();

    let user: User = sqlx::query_as(
        r#"
        INSERT INTO users (id, email, password_hash, full_name, phone_num, created_at)
        VALUES ($1, $2, $3, $4, $5, NOW())
        RETURNING id, email, password_hash, full_name, phone_num, image, points, rank, role, created_at
        "#,
    )
    .bind(user_id)
    .bind(&req.email)
    .bind(Some(password_hash))
    .bind(req.full_name)
    .bind(req.phone_num)
    .fetch_one(&state.pool)
    .await?;

    sqlx::query(
        "INSERT INTO user_stats (user_id, created_at, updated_at) VALUES ($1, NOW(), NOW())",
    )
    .bind(user_id)
    .execute(&state.pool)
    .await?;

    let token = create_token(user.id)?;

    Ok(Json(AuthResponse {
        token,
        user: UserResponse {
            id: user.id,
            full_name: user.full_name,
            email: user.email,
            image: user.image,
            role: user.role,
        },
    }))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let user: User = sqlx::query_as("SELECT * FROM users WHERE email = $1")
        .bind(req.email)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::AuthError)?;

    // Check if user has a password hash (not a Google OAuth-only user)
    let password_hash = user.password_hash.as_ref().ok_or_else(|| {
        AppError::BadRequest(
            "This account uses Google Sign-In. Please use the 'Sign in with Google' button."
                .to_string(),
        )
    })?;

    if !verify(req.password.as_bytes(), password_hash)
        .map_err(|e| AppError::InternalError(e.into()))?
    {
        return Err(AppError::AuthError);
    }

    let token = create_token(user.id)?;

    Ok(Json(AuthResponse {
        token,
        user: UserResponse {
            id: user.id,
            full_name: user.full_name,
            email: user.email,
            image: user.image,
            role: user.role,
        },
    }))
}

pub async fn get_leaderboards(
    State(state): State<AppState>,
) -> Result<Json<Vec<LeaderboardResponse>>, AppError> {
    // Get top 10 users by points
    let entries: Vec<LeaderboardEntry> =
        sqlx::query_as("SELECT full_name as name, points FROM users ORDER BY points DESC LIMIT 10")
            .fetch_all(&state.pool)
            .await?;

    // Return a single leaderboard with top 10 users
    let response = LeaderboardResponse {
        id: 1,
        title: "Top Users".to_string(),
        entries,
    };

    Ok(Json(vec![response]))
}

pub async fn get_resources(
    State(state): State<AppState>,
) -> Result<Json<Vec<ResourceListResponse>>, AppError> {
    let resources: Vec<Resource> =
        sqlx::query_as("SELECT * FROM resources WHERE visible = true ORDER BY id")
            .fetch_all(&state.pool)
            .await?;

    let responses: Vec<ResourceListResponse> = resources
        .into_iter()
        .map(|r| ResourceListResponse {
            id: r.id,
            title: r.title,
            provider: r.provider,
            cover_image: r.cover_image,
            instructor: InstructorResponse {
                name: r.instructor_name,
                image: r.instructor_image,
            },
        })
        .collect();

    Ok(Json(responses))
}

pub async fn get_resource_by_id(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<ResourceDetailResponse>, AppError> {
    let resource: Resource =
        sqlx::query_as("SELECT * FROM resources WHERE id = $1 AND visible = true")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or(AppError::NotFound)?;

    // Fetch a random quote from the quotes table
    let quote: Option<Quote> =
        sqlx::query_as("SELECT * FROM quotes WHERE visible = true ORDER BY RANDOM() LIMIT 1")
            .fetch_optional(&state.pool)
            .await?;

    let quote_response = quote.map(|q| QuoteResponse {
        text: q.text,
        author: q.author,
    });

    Ok(Json(ResourceDetailResponse {
        id: resource.id,
        title: resource.title,
        provider: resource.provider,
        notion_url: resource.notion_url,
        instructor: InstructorResponse {
            name: resource.instructor_name,
            image: resource.instructor_image,
        },
        quote: quote_response,
    }))
}

pub async fn get_current_challenge(
    _auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<ChallengeResponse>, AppError> {
    let challenge: Challenge = sqlx::query_as(
        r#"
        SELECT * FROM challenges 
        WHERE visible = true 
        AND (start_date IS NULL OR start_date <= NOW())
        AND (end_date IS NULL OR end_date >= NOW())
        ORDER BY created_at DESC 
        LIMIT 1
        "#,
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(ChallengeResponse {
        id: challenge.id,
        week: challenge.week,
        title: challenge.title,
        description: challenge.description,
        challenge_url: challenge.challenge_url,
    }))
}

pub async fn get_challenge_leaderboard(
    _auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<ChallengeLeaderboardEntry>>, AppError> {
    // Get top 10 users by points from users table
    let entries: Vec<ChallengeLeaderboardEntry> = sqlx::query_as(
        r#"
        SELECT id, full_name as name, points, image
        FROM users
        ORDER BY points DESC
        LIMIT 10
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(entries))
}

pub async fn get_user_profile(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<UserProfileResponse>, AppError> {
    let user: User = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(auth.user_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let stats: UserStats = sqlx::query_as("SELECT * FROM user_stats WHERE user_id = $1")
        .bind(auth.user_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(UserProfileResponse {
        rank: user.rank,
        name: user.full_name,
        points: user.points,
        image: user.image,
        stats: UserStatsResponse {
            best_subject: stats.best_subject,
            improveable: stats.improveable,
            quickest_hunter: stats.quickest_hunter,
            challenges_taken: stats.challenges_taken,
        },
    }))
}

pub async fn create_contact(
    State(state): State<AppState>,
    Json(req): Json<ContactRequest>,
) -> Result<Json<ContactResponse>, AppError> {
    sqlx::query(
        "INSERT INTO contact_messages (name, email, message, created_at) VALUES ($1, $2, $3, NOW())",
    ).bind(req.name)
    .bind(req.email)
    .bind(req.message)
    .execute(&state.pool)
    .await?;

    Ok(Json(ContactResponse {
        success: true,
        message: "Message sent successfully".to_string(),
    }))
}

#[derive(Deserialize)]
pub struct AdminResourceQuery {
    #[serde(rename = "includeHidden")]
    include_hidden: Option<bool>,
}

pub async fn admin_get_resources(
    _auth: AdminUser,
    State(state): State<AppState>,
    Query(query): Query<AdminResourceQuery>,
) -> Result<Json<AdminItemsResponse<AdminResourceResponse>>, AppError> {
    let include_hidden = query.include_hidden.unwrap_or(false);

    let sql = if include_hidden {
        "SELECT * FROM resources ORDER BY id"
    } else {
        "SELECT * FROM resources WHERE visible = true ORDER BY id"
    };

    let resources: Vec<Resource> = sqlx::query_as(sql).fetch_all(&state.pool).await?;

    let responses: Vec<AdminResourceResponse> = resources
        .into_iter()
        .map(|r| AdminResourceResponse {
            id: r.id,
            title: r.title,
            provider: r.provider,
            cover_image: r.cover_image,
            notion_url: r.notion_url,
            instructor: Some(AdminInstructorResponse {
                name: r.instructor_name,
                image: r.instructor_image,
            }),
            quote: None, // Quotes are now in a separate table
            visible: r.visible,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect();

    Ok(Json(AdminItemsResponse { items: responses }))
}

pub async fn admin_get_resource_by_id(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<AdminItemResponse<AdminResourceResponse>>, AppError> {
    let resource: Resource = sqlx::query_as("SELECT * FROM resources WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let response = AdminResourceResponse {
        id: resource.id,
        title: resource.title,
        provider: resource.provider,
        cover_image: resource.cover_image,
        notion_url: resource.notion_url,
        instructor: Some(AdminInstructorResponse {
            name: resource.instructor_name,
            image: resource.instructor_image,
        }),
        quote: None,
        visible: resource.visible,
        created_at: resource.created_at,
        updated_at: resource.updated_at,
    };

    Ok(Json(AdminItemResponse { item: response }))
}

pub async fn admin_create_resource(
    _auth: AdminUser,
    State(state): State<AppState>,
    Json(req): Json<AdminCreateResourceRequest>,
) -> Result<Json<AdminItemResponse<AdminResourceResponse>>, AppError> {
    let visible = req.visible.unwrap_or(true);
    let instructor_name = req
        .instructor
        .as_ref()
        .map(|i| i.name.clone())
        .unwrap_or_default();
    let instructor_image = req.instructor.as_ref().and_then(|i| i.image.clone());

    let resource: Resource = sqlx::query_as(
        r#"
        INSERT INTO resources (title, provider, cover_image, notion_url, instructor_name, instructor_image, visible, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
        RETURNING *
        "#,
    )
    .bind(&req.title)
    .bind(&req.provider)
    .bind(&req.cover_image)
    .bind(&req.notion_url)
    .bind(&instructor_name)
    .bind(&instructor_image)
    .bind(visible)
    .fetch_one(&state.pool)
    .await?;

    let response = AdminResourceResponse {
        id: resource.id,
        title: resource.title,
        provider: resource.provider,
        cover_image: resource.cover_image,
        notion_url: resource.notion_url,
        instructor: Some(AdminInstructorResponse {
            name: resource.instructor_name,
            image: resource.instructor_image,
        }),
        quote: None,
        visible: resource.visible,
        created_at: resource.created_at,
        updated_at: resource.updated_at,
    };

    Ok(Json(AdminItemResponse { item: response }))
}

pub async fn admin_update_resource(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<AdminUpdateResourceRequest>,
) -> Result<Json<AdminItemResponse<AdminResourceResponse>>, AppError> {
    let existing: Resource = sqlx::query_as("SELECT * FROM resources WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let title = req.title.unwrap_or(existing.title);
    let provider = req.provider.unwrap_or(existing.provider);
    let cover_image = req.cover_image.or(existing.cover_image);
    let notion_url = req.notion_url.or(existing.notion_url);
    let instructor_name = req
        .instructor
        .as_ref()
        .map(|i| i.name.clone())
        .unwrap_or(existing.instructor_name);
    let instructor_image = req
        .instructor
        .as_ref()
        .and_then(|i| i.image.clone())
        .or(existing.instructor_image);
    let visible = req.visible.unwrap_or(existing.visible);

    let resource: Resource = sqlx::query_as(
        r#"
        UPDATE resources 
        SET title = $1, provider = $2, cover_image = $3, notion_url = $4, instructor_name = $5, instructor_image = $6, visible = $7, updated_at = NOW()
        WHERE id = $8
        RETURNING *
        "#,
    )
    .bind(&title)
    .bind(&provider)
    .bind(&cover_image)
    .bind(&notion_url)
    .bind(&instructor_name)
    .bind(&instructor_image)
    .bind(visible)
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    let response = AdminResourceResponse {
        id: resource.id,
        title: resource.title,
        provider: resource.provider,
        cover_image: resource.cover_image,
        notion_url: resource.notion_url,
        instructor: Some(AdminInstructorResponse {
            name: resource.instructor_name,
            image: resource.instructor_image,
        }),
        quote: None,
        visible: resource.visible,
        created_at: resource.created_at,
        updated_at: resource.updated_at,
    };

    Ok(Json(AdminItemResponse { item: response }))
}

pub async fn admin_delete_resource(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<AdminSuccessResponse>, AppError> {
    let result = sqlx::query("DELETE FROM resources WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(AdminSuccessResponse { success: true }))
}

pub async fn admin_patch_resource_visibility(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<AdminVisibilityRequest>,
) -> Result<Json<AdminItemResponse<AdminResourceResponse>>, AppError> {
    let resource: Resource = sqlx::query_as(
        "UPDATE resources SET visible = $1, updated_at = NOW() WHERE id = $2 RETURNING *",
    )
    .bind(req.visible)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let response = AdminResourceResponse {
        id: resource.id,
        title: resource.title,
        provider: resource.provider,
        cover_image: resource.cover_image,
        notion_url: resource.notion_url,
        instructor: Some(AdminInstructorResponse {
            name: resource.instructor_name,
            image: resource.instructor_image,
        }),
        quote: None,
        visible: resource.visible,
        created_at: resource.created_at,
        updated_at: resource.updated_at,
    };

    Ok(Json(AdminItemResponse { item: response }))
}

#[derive(Deserialize)]
pub struct AdminChallengeQuery {
    #[serde(rename = "includeHidden")]
    include_hidden: Option<bool>,
}

pub async fn admin_get_challenges(
    _auth: AdminUser,
    State(state): State<AppState>,
    Query(query): Query<AdminChallengeQuery>,
) -> Result<Json<AdminItemsResponse<AdminChallengeResponse>>, AppError> {
    let include_hidden = query.include_hidden.unwrap_or(false);

    let sql = if include_hidden {
        "SELECT * FROM challenges ORDER BY id"
    } else {
        "SELECT * FROM challenges WHERE visible = true ORDER BY id"
    };

    let challenges: Vec<Challenge> = sqlx::query_as(sql).fetch_all(&state.pool).await?;

    let responses: Vec<AdminChallengeResponse> = challenges
        .into_iter()
        .map(|c| AdminChallengeResponse {
            id: c.id,
            title: c.title,
            description: c.description,
            start_date: c.start_date,
            end_date: c.end_date,
            visible: c.visible,
            created_at: c.created_at,
            updated_at: c.updated_at,
        })
        .collect();

    Ok(Json(AdminItemsResponse { items: responses }))
}

pub async fn admin_get_challenge_by_id(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<AdminItemResponse<AdminChallengeResponse>>, AppError> {
    let challenge: Challenge = sqlx::query_as("SELECT * FROM challenges WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let response = AdminChallengeResponse {
        id: challenge.id,
        title: challenge.title,
        description: challenge.description,
        start_date: challenge.start_date,
        end_date: challenge.end_date,
        visible: challenge.visible,
        created_at: challenge.created_at,
        updated_at: challenge.updated_at,
    };

    Ok(Json(AdminItemResponse { item: response }))
}

pub async fn admin_create_challenge(
    _auth: AdminUser,
    State(state): State<AppState>,
    Json(req): Json<AdminCreateChallengeRequest>,
) -> Result<Json<AdminItemResponse<AdminChallengeResponse>>, AppError> {
    let visible = req.visible.unwrap_or(true);
    let week = req.week.unwrap_or(1);
    let challenge_url = req.challenge_url.unwrap_or_default();

    let challenge: Challenge = sqlx::query_as(
        r#"
        INSERT INTO challenges (title, description, start_date, end_date, visible, week, challenge_url, is_current, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, false, NOW(), NOW())
        RETURNING *
        "#,
    )
    .bind(&req.title)
    .bind(&req.description)
    .bind(req.start_date)
    .bind(req.end_date)
    .bind(visible)
    .bind(week)
    .bind(&challenge_url)
    .fetch_one(&state.pool)
    .await?;

    let response = AdminChallengeResponse {
        id: challenge.id,
        title: challenge.title,
        description: challenge.description,
        start_date: challenge.start_date,
        end_date: challenge.end_date,
        visible: challenge.visible,
        created_at: challenge.created_at,
        updated_at: challenge.updated_at,
    };

    Ok(Json(AdminItemResponse { item: response }))
}

pub async fn admin_update_challenge(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<AdminUpdateChallengeRequest>,
) -> Result<Json<AdminItemResponse<AdminChallengeResponse>>, AppError> {
    let existing: Challenge = sqlx::query_as("SELECT * FROM challenges WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let title = req.title.unwrap_or(existing.title);
    let description = req.description.unwrap_or(existing.description);
    let week = req.week.unwrap_or(existing.week);
    let challenge_url = req.challenge_url.unwrap_or(existing.challenge_url);
    let start_date = req.start_date.or(existing.start_date);
    let end_date = req.end_date.or(existing.end_date);
    let visible = req.visible.unwrap_or(existing.visible);

    let challenge: Challenge = sqlx::query_as(
        r#"
        UPDATE challenges 
        SET title = $1, description = $2, week = $3, challenge_url = $4, start_date = $5, end_date = $6, visible = $7, updated_at = NOW()
        WHERE id = $8
        RETURNING *
        "#,
    )
    .bind(&title)
    .bind(&description)
    .bind(week)
    .bind(&challenge_url)
    .bind(start_date)
    .bind(end_date)
    .bind(visible)
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    let response = AdminChallengeResponse {
        id: challenge.id,
        title: challenge.title,
        description: challenge.description,
        start_date: challenge.start_date,
        end_date: challenge.end_date,
        visible: challenge.visible,
        created_at: challenge.created_at,
        updated_at: challenge.updated_at,
    };

    Ok(Json(AdminItemResponse { item: response }))
}

pub async fn admin_delete_challenge(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<AdminSuccessResponse>, AppError> {
    let result = sqlx::query("DELETE FROM challenges WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(AdminSuccessResponse { success: true }))
}

pub async fn admin_patch_challenge_visibility(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<AdminVisibilityRequest>,
) -> Result<Json<AdminItemResponse<AdminChallengeResponse>>, AppError> {
    let challenge: Challenge = sqlx::query_as(
        "UPDATE challenges SET visible = $1, updated_at = NOW() WHERE id = $2 RETURNING *",
    )
    .bind(req.visible)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let response = AdminChallengeResponse {
        id: challenge.id,
        title: challenge.title,
        description: challenge.description,
        start_date: challenge.start_date,
        end_date: challenge.end_date,
        visible: challenge.visible,
        created_at: challenge.created_at,
        updated_at: challenge.updated_at,
    };

    Ok(Json(AdminItemResponse { item: response }))
}

// User profile management endpoints

pub async fn update_user_profile(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<UpdateProfileRequest>,
) -> Result<Json<UpdateProfileResponse>, AppError> {
    // Get current user data
    let current_user: User = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(auth.user_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check if email is being changed and if it's already taken
    if let Some(ref new_email) = req.email {
        if new_email != &current_user.email {
            let existing_user = sqlx::query("SELECT id FROM users WHERE email = $1 AND id != $2")
                .bind(new_email)
                .bind(auth.user_id)
                .fetch_optional(&state.pool)
                .await?;

            if existing_user.is_some() {
                return Err(AppError::UserExists);
            }
        }
    }

    let full_name = req.full_name.unwrap_or(current_user.full_name);
    let email = req.email.unwrap_or(current_user.email);
    let image = req.image.or(current_user.image);

    let updated_user: User = sqlx::query_as(
        r#"
        UPDATE users 
        SET full_name = $1, email = $2, image = $3
        WHERE id = $4
        RETURNING *
        "#,
    )
    .bind(&full_name)
    .bind(&email)
    .bind(&image)
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(UpdateProfileResponse {
        id: updated_user.id,
        full_name: updated_user.full_name,
        email: updated_user.email,
        image: updated_user.image,
        role: updated_user.role,
    }))
}

// Helper function to save uploaded file
async fn save_uploaded_file(
    _field_name: &str,
    file_name: &str,
    data: &[u8],
    subdirectory: &str,
) -> Result<String, AppError> {
    use tokio::io::AsyncWriteExt;

    let upload_dir = format!("uploads/{subdirectory}");

    tracing::info!("Creating directory: {}", upload_dir);

    tokio::fs::create_dir_all(&upload_dir).await.map_err(|e| {
        tracing::error!("Failed to create directory {}: {}", upload_dir, e);
        AppError::InternalError(anyhow::anyhow!("Failed to create upload directory: {e}"))
    })?;

    let unique_filename = format!("{}_{}", Uuid::new_v4(), file_name);
    let file_path = format!("{upload_dir}/{unique_filename}");

    tracing::info!("Saving file to: {}", file_path);

    let mut file = tokio::fs::File::create(&file_path).await.map_err(|e| {
        tracing::error!("Failed to create file {}: {}", file_path, e);
        AppError::InternalError(anyhow::anyhow!("Failed to create file: {e}"))
    })?;

    file.write_all(data).await.map_err(|e| {
        tracing::error!("Failed to write file {}: {}", file_path, e);
        AppError::InternalError(anyhow::anyhow!("Failed to write file: {e}"))
    })?;

    let result_url = format!("/{upload_dir}/{unique_filename}");
    tracing::info!("File saved successfully: {}", result_url);

    Ok(result_url)
}

// Admin resource endpoints with multipart form data

pub async fn admin_create_resource_multipart(
    _auth: AdminUser,
    State(state): State<AppState>,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<AdminItemResponse<AdminResourceResponse>>, AppError> {
    tracing::info!("Starting multipart resource creation");

    let mut title: Option<String> = None;
    let mut provider: Option<String> = None;
    let mut cover_image: Option<String> = None;
    let mut notion_url: Option<String> = None;
    let mut instructor_name: Option<String> = None;
    let mut instructor_image: Option<String> = None;
    let mut visible: Option<bool> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::error!("Error reading multipart field: {}", e);
        AppError::InternalError(e.into())
    })? {
        let field_name = field.name().unwrap_or("").to_string();
        tracing::info!("Processing field: {}", field_name);

        match field_name.as_str() {
            "title" => {
                title = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::InternalError(e.into()))?,
                );
            }
            "provider" => {
                provider = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::InternalError(e.into()))?,
                );
            }
            "notionUrl" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::InternalError(e.into()))?;
                if !text.is_empty() {
                    notion_url = Some(text);
                }
            }
            "instructorName" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::InternalError(e.into()))?;
                if !text.is_empty() {
                    instructor_name = Some(text);
                }
            }
            "quoteText" | "quoteAuthor" => {
                // Ignore quote fields - quotes are now in a separate table
                let _ = field.text().await;
            }
            "visible" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::InternalError(e.into()))?;
                visible = Some(text == "true" || text == "1");
            }
            "coverImage" => {
                if let Some(file_name) = field.file_name().map(|s| s.to_string()) {
                    let data = field
                        .bytes()
                        .await
                        .map_err(|e| AppError::InternalError(e.into()))?;
                    let url =
                        save_uploaded_file("coverImage", &file_name, &data, "resources/covers")
                            .await?;
                    cover_image = Some(url);
                }
            }
            "instructorImage" => {
                if let Some(file_name) = field.file_name().map(|s| s.to_string()) {
                    let data = field
                        .bytes()
                        .await
                        .map_err(|e| AppError::InternalError(e.into()))?;
                    let url = save_uploaded_file(
                        "instructorImage",
                        &file_name,
                        &data,
                        "resources/instructors",
                    )
                    .await?;
                    instructor_image = Some(url);
                }
            }
            _ => {}
        }
    }

    let title =
        title.ok_or_else(|| AppError::BadRequest("Missing required field: title".to_string()))?;
    let provider = provider
        .ok_or_else(|| AppError::BadRequest("Missing required field: provider".to_string()))?;
    let instructor_name = instructor_name.unwrap_or_default();
    let visible = visible.unwrap_or(true);

    let resource: Resource = sqlx::query_as(
        r#"
        INSERT INTO resources (title, provider, cover_image, notion_url, instructor_name, instructor_image, visible, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
        RETURNING *
        "#,
    )
    .bind(&title)
    .bind(&provider)
    .bind(&cover_image)
    .bind(&notion_url)
    .bind(&instructor_name)
    .bind(&instructor_image)
    .bind(visible)
    .fetch_one(&state.pool)
    .await?;

    let response = AdminResourceResponse {
        id: resource.id,
        title: resource.title,
        provider: resource.provider,
        cover_image: resource.cover_image,
        notion_url: resource.notion_url,
        instructor: Some(AdminInstructorResponse {
            name: resource.instructor_name,
            image: resource.instructor_image,
        }),
        quote: None,
        visible: resource.visible,
        created_at: resource.created_at,
        updated_at: resource.updated_at,
    };

    Ok(Json(AdminItemResponse { item: response }))
}

pub async fn admin_update_resource_multipart(
    _auth: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<AdminItemResponse<AdminResourceResponse>>, AppError> {
    let existing: Resource = sqlx::query_as("SELECT * FROM resources WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    let mut title: Option<String> = None;
    let mut provider: Option<String> = None;
    let mut cover_image: Option<String> = None;
    let mut notion_url: Option<Option<String>> = None;
    let mut instructor_name: Option<String> = None;
    let mut instructor_image: Option<Option<String>> = None;
    let mut visible: Option<bool> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::InternalError(e.into()))?
    {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "title" => {
                title = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::InternalError(e.into()))?,
                );
            }
            "provider" => {
                provider = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::InternalError(e.into()))?,
                );
            }
            "notionUrl" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::InternalError(e.into()))?;
                notion_url = Some(if text.is_empty() { None } else { Some(text) });
            }
            "instructorName" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::InternalError(e.into()))?;
                if !text.is_empty() {
                    instructor_name = Some(text);
                }
            }
            "quoteText" | "quoteAuthor" => {
                // Ignore quote fields - quotes are now in a separate table
                let _ = field.text().await;
            }
            "visible" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::InternalError(e.into()))?;
                visible = Some(text == "true" || text == "1");
            }
            "coverImage" => {
                if let Some(file_name) = field.file_name().map(|s| s.to_string()) {
                    let data = field
                        .bytes()
                        .await
                        .map_err(|e| AppError::InternalError(e.into()))?;
                    let url =
                        save_uploaded_file("coverImage", &file_name, &data, "resources/covers")
                            .await?;
                    cover_image = Some(url);
                }
            }
            "instructorImage" => {
                if let Some(file_name) = field.file_name().map(|s| s.to_string()) {
                    let data = field
                        .bytes()
                        .await
                        .map_err(|e| AppError::InternalError(e.into()))?;
                    let url = save_uploaded_file(
                        "instructorImage",
                        &file_name,
                        &data,
                        "resources/instructors",
                    )
                    .await?;
                    instructor_image = Some(Some(url));
                }
            }
            _ => {}
        }
    }

    let title = title.unwrap_or(existing.title);
    let provider = provider.unwrap_or(existing.provider);
    let cover_image = cover_image.or(existing.cover_image);
    let notion_url = notion_url.unwrap_or(existing.notion_url);
    let instructor_name = instructor_name.unwrap_or(existing.instructor_name);
    let instructor_image = instructor_image.unwrap_or(existing.instructor_image);
    let visible = visible.unwrap_or(existing.visible);

    let resource: Resource = sqlx::query_as(
        r#"
        UPDATE resources 
        SET title = $1, provider = $2, cover_image = $3, notion_url = $4, instructor_name = $5, instructor_image = $6, visible = $7, updated_at = NOW()
        WHERE id = $8
        RETURNING *
        "#,
    )
    .bind(&title)
    .bind(&provider)
    .bind(&cover_image)
    .bind(&notion_url)
    .bind(&instructor_name)
    .bind(&instructor_image)
    .bind(visible)
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    let response = AdminResourceResponse {
        id: resource.id,
        title: resource.title,
        provider: resource.provider,
        cover_image: resource.cover_image,
        notion_url: resource.notion_url,
        instructor: Some(AdminInstructorResponse {
            name: resource.instructor_name,
            image: resource.instructor_image,
        }),
        quote: None,
        visible: resource.visible,
        created_at: resource.created_at,
        updated_at: resource.updated_at,
    };

    Ok(Json(AdminItemResponse { item: response }))
}

pub async fn upload_user_avatar(
    auth: AuthUser,
    State(state): State<AppState>,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<UploadAvatarResponse>, AppError> {
    use tokio::io::AsyncWriteExt;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::InternalError(e.into()))?
    {
        let name = field.name().unwrap_or("").to_string();

        if name == "avatar" {
            let file_name = field
                .file_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}.jpg", Uuid::new_v4()));

            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::InternalError(e.into()))?;

            // Create uploads directory if it doesn't exist
            tokio::fs::create_dir_all("uploads/avatars")
                .await
                .map_err(|e| AppError::InternalError(e.into()))?;

            // Generate unique filename
            let unique_filename = format!("{}_{}", Uuid::new_v4(), file_name);
            let file_path = format!("uploads/avatars/{unique_filename}");

            // Save file
            let mut file = tokio::fs::File::create(&file_path)
                .await
                .map_err(|e| AppError::InternalError(e.into()))?;

            file.write_all(&data)
                .await
                .map_err(|e| AppError::InternalError(e.into()))?;

            // Generate URL (you may want to customize this based on your domain)
            let image_url = format!("/uploads/avatars/{unique_filename}");

            // Update user's image in database
            sqlx::query("UPDATE users SET image = $1 WHERE id = $2")
                .bind(&image_url)
                .bind(auth.user_id)
                .execute(&state.pool)
                .await?;

            return Ok(Json(UploadAvatarResponse { image_url }));
        }
    }

    Err(AppError::BadRequest("No avatar file provided".to_string()))
}

pub async fn update_user_password(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<UpdatePasswordRequest>,
) -> Result<Json<UpdatePasswordResponse>, AppError> {
    // Get current user
    let user: User = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(auth.user_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;

    // Check if user has a password (not a Google OAuth-only user)
    let current_password_hash = user.password_hash.as_ref().ok_or_else(|| {
        AppError::BadRequest(
            "This account uses Google Sign-In and doesn't have a password.".to_string(),
        )
    })?;

    // Verify current password
    if !verify(req.current_password.as_bytes(), current_password_hash)
        .map_err(|e| AppError::InternalError(e.into()))?
    {
        return Err(AppError::AuthError);
    }

    // Hash new password
    let new_password_hash = hash(req.new_password.as_bytes(), DEFAULT_COST)
        .map_err(|e| AppError::InternalError(e.into()))?;

    // Update password
    sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(new_password_hash)
        .bind(auth.user_id)
        .execute(&state.pool)
        .await?;

    Ok(Json(UpdatePasswordResponse { success: true }))
}

// Google OAuth handlers
#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    code: String,
    state: String,
}

pub async fn google_auth_init(State(state): State<AppState>) -> impl IntoResponse {
    use oauth2::basic::BasicClient;
    use oauth2::{AuthUrl, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope, TokenUrl};

    // Create OAuth client
    let client = BasicClient::new(
        ClientId::new(state.oauth_config.client_id.clone()),
        Some(ClientSecret::new(state.oauth_config.client_secret.clone())),
        AuthUrl::new(state.oauth_config.auth_url.clone())
            .expect("Invalid authorization endpoint URL"),
        Some(
            TokenUrl::new(state.oauth_config.token_url.clone())
                .expect("Invalid token endpoint URL"),
        ),
    )
    .set_redirect_uri(
        RedirectUrl::new(state.oauth_config.redirect_uri.clone()).expect("Invalid redirect URL"),
    );

    // Generate authorization URL
    let (auth_url, _csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .url();

    Redirect::temporary(auth_url.as_str())
}

pub async fn google_auth_callback(
    State(state): State<AppState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<impl IntoResponse, AppError> {
    use oauth2::basic::BasicClient;
    use oauth2::{
        AuthUrl, AuthorizationCode, ClientId, ClientSecret, RedirectUrl, TokenResponse, TokenUrl,
    };

    // Create OAuth client
    let client = BasicClient::new(
        ClientId::new(state.oauth_config.client_id.clone()),
        Some(ClientSecret::new(state.oauth_config.client_secret.clone())),
        AuthUrl::new(state.oauth_config.auth_url.clone())
            .expect("Invalid authorization endpoint URL"),
        Some(
            TokenUrl::new(state.oauth_config.token_url.clone())
                .expect("Invalid token endpoint URL"),
        ),
    )
    .set_redirect_uri(
        RedirectUrl::new(state.oauth_config.redirect_uri.clone()).expect("Invalid redirect URL"),
    );

    // Exchange authorization code for access token
    let token_result = client
        .exchange_code(AuthorizationCode::new(query.code))
        .request_async(oauth2::reqwest::async_http_client)
        .await
        .map_err(|e| AppError::InternalError(anyhow::anyhow!("Token exchange failed: {e}")))?;

    // Fetch user info from Google
    let user_info: GoogleUserInfo = reqwest::Client::new()
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(token_result.access_token().secret())
        .send()
        .await
        .map_err(|e| AppError::InternalError(e.into()))?
        .json()
        .await
        .map_err(|e| AppError::InternalError(e.into()))?;

    // Check if user exists with this google_id
    let existing_user: Option<User> = sqlx::query_as(
        "SELECT id, email, password_hash, full_name, phone_num, image, points, rank, role, created_at 
         FROM users WHERE google_id = $1"
    )
    .bind(&user_info.sub)
    .fetch_optional(&state.pool)
    .await?;

    let user = if let Some(user) = existing_user {
        // User exists, update their info if needed
        sqlx::query_as(
            "UPDATE users SET email = $1, full_name = $2, image = $3 
             WHERE google_id = $4
             RETURNING id, email, password_hash, full_name, phone_num, image, points, rank, role, created_at"
        )
        .bind(&user_info.email)
        .bind(user_info.name.as_deref().unwrap_or(&user.full_name))
        .bind(&user_info.picture)
        .bind(&user_info.sub)
        .fetch_one(&state.pool)
        .await?
    } else {
        // Check if user exists with same email (linking accounts)
        let email_user: Option<User> = sqlx::query_as(
            "SELECT id, email, password_hash, full_name, phone_num, image, points, rank, role, created_at 
             FROM users WHERE email = $1"
        )
        .bind(&user_info.email)
        .fetch_optional(&state.pool)
        .await?;

        if let Some(existing) = email_user {
            // Link Google account to existing user
            sqlx::query_as(
                "UPDATE users SET google_id = $1, image = COALESCE($2, image) 
                 WHERE id = $3
                 RETURNING id, email, password_hash, full_name, phone_num, image, points, rank, role, created_at"
            )
            .bind(&user_info.sub)
            .bind(&user_info.picture)
            .bind(existing.id)
            .fetch_one(&state.pool)
            .await?
        } else {
            // Create new user
            let user_id = Uuid::new_v4();
            let user: User = sqlx::query_as(
                r#"
                INSERT INTO users (id, email, password_hash, full_name, google_id, image, created_at)
                VALUES ($1, $2, NULL, $3, $4, $5, NOW())
                RETURNING id, email, password_hash, full_name, phone_num, image, points, rank, role, created_at
                "#,
            )
            .bind(user_id)
            .bind(&user_info.email)
            .bind(user_info.name.as_deref().unwrap_or(&user_info.email))
            .bind(&user_info.sub)
            .bind(&user_info.picture)
            .fetch_one(&state.pool)
            .await?;

            // Create user stats
            sqlx::query(
                "INSERT INTO user_stats (user_id, created_at, updated_at) VALUES ($1, NOW(), NOW())",
            )
            .bind(user_id)
            .execute(&state.pool)
            .await?;

            user
        }
    };

    // Check if user needs to complete profile (university and major)
    let needs_profile: Option<(bool,)> =
        sqlx::query_as("SELECT university_major_set FROM users WHERE id = $1")
            .bind(user.id)
            .fetch_optional(&state.pool)
            .await?;

    let needs_completion = needs_profile.map(|(set,)| !set).unwrap_or(true);

    // Create JWT token
    let token = create_token(user.id)?;

    // Encode user data
    let user_json = serde_json::to_string(&UserResponse {
        id: user.id,
        full_name: user.full_name,
        email: user.email,
        image: user.image,
        role: user.role,
    })
    .map_err(|e| AppError::InternalError(e.into()))?;

    let encoded_user = urlencoding::encode(&user_json);

    // Get frontend URL from environment or use default
    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "https://aiclub-uj.com".to_string());

    // Redirect to frontend with token and user data
    let redirect_url = if needs_completion {
        format!(
            "{frontend_url}/auth/callback?token={token}&user={encoded_user}&needs_profile_completion=true"
        )
    } else {
        format!("{frontend_url}/auth/callback?token={token}&user={encoded_user}")
    };

    Ok(Redirect::temporary(&redirect_url))
}

pub async fn complete_profile(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<CompleteProfileRequest>,
) -> Result<Json<CompleteProfileResponse>, AppError> {
    // Update user's university and major
    sqlx::query(
        "UPDATE users SET university = $1, major = $2, university_major_set = TRUE WHERE id = $3",
    )
    .bind(&req.university)
    .bind(&req.major)
    .bind(auth.user_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(CompleteProfileResponse { success: true }))
}
