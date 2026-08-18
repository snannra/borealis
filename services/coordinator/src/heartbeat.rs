use crate::app_state::AppState;
use axum::Json;
use axum::extract::{ConnectInfo, Path, State};
use axum::http::StatusCode;
use borealis_protocol::{HeartbeatRequest, HeartbeatResponse};
use chrono::{DateTime, Duration, Utc};
use std::net::SocketAddr;

const LEASE_DURATION_MINUTES: i64 = 5;

pub async fn heartbeat(
    State(state): State<AppState>,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    Path(node_id): Path<i64>,
    Json(request): Json<HeartbeatRequest>,
) -> Result<Json<HeartbeatResponse>, StatusCode> {
    if request.listen_port == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let lease_expires_at = Utc::now() + Duration::minutes(LEASE_DURATION_MINUTES);
    let observed_ip = remote_addr.ip().to_string();

    let renewed_lease = sqlx::query_scalar::<_, DateTime<Utc>>(
        r#"
        UPDATE nodes
        SET observed_ip = $2::inet,
            advertised_port = $3,
            updated_at = NOW(),
            last_seen_at = NOW(),
            lease_expires_at = $4
        WHERE id = $1
          AND status = 'active'
        RETURNING lease_expires_at
        "#,
    )
    .bind(node_id)
    .bind(observed_ip)
    .bind(i32::from(request.listen_port))
    .bind(lease_expires_at)
    .fetch_optional(&state.db)
    .await
    .map_err(internal_error)?;

    let lease_expires_at = renewed_lease.ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(HeartbeatResponse { lease_expires_at }))
}

fn internal_error(error: impl std::fmt::Display) -> StatusCode {
    eprintln!("heartbeat failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
