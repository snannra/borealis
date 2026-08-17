use crate::app_state::AppState;
use axum::extract::{ConnectInfo, Json, State};
use axum::http::StatusCode;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr};

const LEASE_DURATION_MINUTES: i64 = 5;

// Serializes address selection across coordinator instances using the same DB.
const OVERLAY_ALLOCATION_LOCK_ID: i64 = 4_778_961_177_345_337;

#[derive(Debug, Serialize)]
pub struct RegisterNodeResponse {
    pub node_id: i64,
    pub overlay_ip: Ipv4Addr,
    pub lease_expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterNodeRequest {
    pub public_key: [u8; 32],
    pub listen_port: u16,
}

pub async fn register(
    State(state): State<AppState>,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    Json(request): Json<RegisterNodeRequest>,
) -> Result<(StatusCode, Json<RegisterNodeResponse>), StatusCode> {
    if request.listen_port == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut transaction = state.db.begin().await.map_err(internal_error)?;

    // Address selection and insertion must be one serialized operation. The
    // transaction-scoped lock is automatically released on commit or rollback.
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(OVERLAY_ALLOCATION_LOCK_ID)
        .execute(&mut *transaction)
        .await
        .map_err(internal_error)?;

    let existing = sqlx::query_as::<_, (i64, String, String)>(
        r#"
        SELECT id, host(overlay_ip), status
        FROM nodes
        WHERE public_key = $1
        FOR UPDATE
        "#,
    )
    .bind(request.public_key.as_slice())
    .fetch_optional(&mut *transaction)
    .await
    .map_err(internal_error)?;

    if matches!(existing.as_ref(), Some((_, _, status)) if status == "revoked") {
        return Err(StatusCode::FORBIDDEN);
    }

    let overlay_ip = match existing.as_ref() {
        Some((_, overlay_ip, _)) => overlay_ip.clone(),
        None => allocate_overlay_ip(&mut transaction).await?,
    };

    let lease_expires_at = Utc::now() + Duration::minutes(LEASE_DURATION_MINUTES);
    let observed_ip = remote_addr.ip().to_string();

    let (node_id, overlay_ip, lease_expires_at) =
        sqlx::query_as::<_, (i64, String, DateTime<Utc>)>(
            r#"
            INSERT INTO nodes (
                public_key,
                overlay_ip,
                observed_ip,
                advertised_port,
                lease_expires_at
            )
            VALUES ($1, $2::inet, $3::inet, $4, $5)
            ON CONFLICT (public_key) DO UPDATE SET
                observed_ip = EXCLUDED.observed_ip,
                advertised_port = EXCLUDED.advertised_port,
                updated_at = NOW(),
                last_seen_at = NOW(),
                lease_expires_at = EXCLUDED.lease_expires_at
            RETURNING id, host(overlay_ip), lease_expires_at
            "#,
        )
        .bind(request.public_key.as_slice())
        .bind(overlay_ip)
        .bind(observed_ip)
        .bind(i32::from(request.listen_port))
        .bind(lease_expires_at)
        .fetch_one(&mut *transaction)
        .await
        .map_err(internal_error)?;

    transaction.commit().await.map_err(internal_error)?;

    let overlay_ip = overlay_ip.parse().map_err(internal_error)?;
    let response = RegisterNodeResponse {
        node_id,
        overlay_ip,
        lease_expires_at,
    };

    Ok((StatusCode::OK, Json(response)))
}

async fn allocate_overlay_ip(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<String, StatusCode> {
    // .0 is the network address, .1 is reserved for future coordinator/gateway
    // use, and .255 is the broadcast address.
    let address = sqlx::query_scalar::<_, String>(
        r#"
        SELECT '10.0.0.' || candidate
        FROM generate_series(2, 254) AS candidate
        WHERE NOT EXISTS (
            SELECT 1
            FROM nodes
            WHERE overlay_ip = ('10.0.0.' || candidate)::inet
        )
        ORDER BY candidate
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut **transaction)
    .await
    .map_err(internal_error)?;

    address.ok_or(StatusCode::SERVICE_UNAVAILABLE)
}

fn internal_error(error: impl std::fmt::Display) -> StatusCode {
    eprintln!("registration failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
