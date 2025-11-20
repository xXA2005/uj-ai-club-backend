use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// Custom deserializer for date strings to OffsetDateTime
mod date_format {
    use serde::{self, Deserialize, Deserializer};
    use time::{Date, OffsetDateTime, Time, UtcOffset};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<OffsetDateTime>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<String> = Option::deserialize(deserializer)?;
        match s {
            Some(s) => {
                // Try to parse as date-only string (YYYY-MM-DD)
                if let Ok(date) =
                    Date::parse(&s, &time::format_description::well_known::Iso8601::DEFAULT)
                {
                    let datetime = date.with_time(Time::MIDNIGHT).assume_offset(UtcOffset::UTC);
                    Ok(Some(datetime))
                } else {
                    // Try to parse as full datetime
                    OffsetDateTime::parse(
                        &s,
                        &time::format_description::well_known::Iso8601::DEFAULT,
                    )
                    .map(Some)
                    .map_err(serde::de::Error::custom)
                }
            }
            None => Ok(None),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    pub full_name: String,
    pub phone_num: Option<String>,
    pub image: Option<String>,
    pub points: i32,
    pub rank: i32,
    pub role: String,
    pub created_at: time::OffsetDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    #[serde(rename = "fullName")]
    pub full_name: String,
    #[serde(rename = "phoneNum")]
    pub phone_num: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    #[serde(rename = "fullName")]
    pub full_name: String,
    pub email: String,
    pub image: Option<String>,
    pub role: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Leaderboard {
    pub id: i32,
    pub title: String,
    pub created_at: time::OffsetDateTime,
}

#[derive(Debug, Serialize, FromRow)]
pub struct LeaderboardEntry {
    pub name: String,
    pub points: i32,
}

#[derive(Debug, Serialize)]
pub struct LeaderboardResponse {
    pub id: i32,
    pub title: String,
    pub entries: Vec<LeaderboardEntry>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Resource {
    pub id: i32,
    pub title: String,
    pub provider: String,
    pub cover_image: Option<String>,
    pub instructor_name: String,
    pub instructor_image: Option<String>,
    pub notion_url: Option<String>,
    pub visible: bool,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Quote {
    pub id: i32,
    pub text: String,
    pub author: String,
    pub visible: bool,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}

#[derive(Debug, Serialize)]
pub struct ResourceListResponse {
    pub id: i32,
    pub title: String,
    pub provider: String,
    #[serde(rename = "coverImage")]
    pub cover_image: Option<String>,
    pub instructor: InstructorResponse,
}

#[derive(Debug, Serialize)]
pub struct ResourceDetailResponse {
    pub id: i32,
    pub title: String,
    pub provider: String,
    #[serde(rename = "notionUrl")]
    pub notion_url: Option<String>,
    pub instructor: InstructorResponse,
    pub quote: Option<QuoteResponse>,
}

#[derive(Debug, Serialize)]
pub struct InstructorResponse {
    pub name: String,
    pub image: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct QuoteResponse {
    pub text: String,
    pub author: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Challenge {
    pub id: i32,
    pub week: i32,
    pub title: String,
    pub description: String,
    pub challenge_url: String,
    pub is_current: bool,
    pub start_date: Option<time::OffsetDateTime>,
    pub end_date: Option<time::OffsetDateTime>,
    pub visible: bool,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}

#[derive(Debug, Serialize)]
pub struct ChallengeResponse {
    pub id: i32,
    pub week: i32,
    pub title: String,
    pub description: String,
    #[serde(rename = "challengeUrl")]
    pub challenge_url: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ChallengeLeaderboardEntry {
    pub id: Uuid,
    pub name: String,
    pub points: i32,
    pub image: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct UserStats {
    pub id: Uuid,
    pub user_id: Uuid,
    pub best_subject: Option<String>,
    pub improveable: Option<String>,
    pub quickest_hunter: i32,
    pub challenges_taken: i32,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}

#[derive(Debug, Serialize)]
pub struct UserProfileResponse {
    pub rank: i32,
    pub name: String,
    pub points: i32,
    pub image: Option<String>,
    pub stats: UserStatsResponse,
}

#[derive(Debug, Serialize)]
pub struct UserStatsResponse {
    #[serde(rename = "bestSubject")]
    pub best_subject: Option<String>,
    pub improveable: Option<String>,
    #[serde(rename = "quickestHunter")]
    pub quickest_hunter: i32,
    #[serde(rename = "challengesTaken")]
    pub challenges_taken: i32,
}

#[derive(Debug, Deserialize)]
pub struct ContactRequest {
    pub name: String,
    pub email: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ContactResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AdminResourceResponse {
    pub id: i32,
    pub title: String,
    pub provider: String,
    #[serde(rename = "coverImage")]
    pub cover_image: Option<String>,
    #[serde(rename = "notionUrl")]
    pub notion_url: Option<String>,
    pub instructor: Option<AdminInstructorResponse>,
    pub quote: Option<AdminQuoteResponse>,
    pub visible: bool,
    #[serde(rename = "createdAt")]
    pub created_at: time::OffsetDateTime,
    #[serde(rename = "updatedAt")]
    pub updated_at: time::OffsetDateTime,
}

#[derive(Debug, Serialize)]
pub struct AdminInstructorResponse {
    pub name: String,
    pub image: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AdminQuoteResponse {
    pub text: String,
    pub author: String,
}

#[derive(Debug, Deserialize)]
pub struct AdminCreateResourceRequest {
    pub title: String,
    pub provider: String,
    #[serde(rename = "coverImage")]
    pub cover_image: Option<String>,
    #[serde(rename = "notionUrl")]
    pub notion_url: Option<String>,
    pub instructor: Option<AdminInstructorRequest>,
    pub quote: Option<AdminQuoteRequest>,
    pub visible: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AdminUpdateResourceRequest {
    pub title: Option<String>,
    pub provider: Option<String>,
    #[serde(rename = "coverImage")]
    pub cover_image: Option<String>,
    #[serde(rename = "notionUrl")]
    pub notion_url: Option<String>,
    pub instructor: Option<AdminInstructorRequest>,
    pub quote: Option<AdminQuoteRequest>,
    pub visible: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AdminInstructorRequest {
    pub name: String,
    pub image: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AdminQuoteRequest {
    pub text: String,
    pub author: String,
}

#[derive(Debug, Deserialize)]
pub struct AdminVisibilityRequest {
    pub visible: bool,
}

#[derive(Debug, Serialize)]
pub struct AdminChallengeResponse {
    pub id: i32,
    pub title: String,
    pub description: String,
    #[serde(rename = "startDate")]
    pub start_date: Option<time::OffsetDateTime>,
    #[serde(rename = "endDate")]
    pub end_date: Option<time::OffsetDateTime>,
    pub visible: bool,
    #[serde(rename = "createdAt")]
    pub created_at: time::OffsetDateTime,
    #[serde(rename = "updatedAt")]
    pub updated_at: time::OffsetDateTime,
}

#[derive(Debug, Deserialize)]
pub struct AdminCreateChallengeRequest {
    pub title: String,
    pub description: String,
    pub week: Option<i32>,
    #[serde(rename = "challengeUrl")]
    pub challenge_url: Option<String>,
    #[serde(rename = "startDate", deserialize_with = "date_format::deserialize")]
    pub start_date: Option<time::OffsetDateTime>,
    #[serde(rename = "endDate", deserialize_with = "date_format::deserialize")]
    pub end_date: Option<time::OffsetDateTime>,
    pub visible: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AdminUpdateChallengeRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub week: Option<i32>,
    #[serde(rename = "challengeUrl")]
    pub challenge_url: Option<String>,
    #[serde(rename = "startDate", deserialize_with = "date_format::deserialize")]
    pub start_date: Option<time::OffsetDateTime>,
    #[serde(rename = "endDate", deserialize_with = "date_format::deserialize")]
    pub end_date: Option<time::OffsetDateTime>,
    pub visible: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct AdminItemResponse<T> {
    pub item: T,
}

#[derive(Debug, Serialize)]
pub struct AdminItemsResponse<T> {
    pub items: Vec<T>,
}

#[derive(Debug, Serialize)]
pub struct AdminSuccessResponse {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    #[serde(rename = "fullName")]
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub image: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateProfileResponse {
    pub id: Uuid,
    #[serde(rename = "fullName")]
    pub full_name: String,
    pub email: String,
    pub image: Option<String>,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct UploadAvatarResponse {
    #[serde(rename = "imageUrl")]
    pub image_url: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePasswordRequest {
    #[serde(rename = "currentPassword")]
    pub current_password: String,
    #[serde(rename = "newPassword")]
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct UpdatePasswordResponse {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct CompleteProfileRequest {
    pub university: String,
    pub major: String,
}

#[derive(Debug, Serialize)]
pub struct CompleteProfileResponse {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
    pub sub: String,
    pub email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
}
