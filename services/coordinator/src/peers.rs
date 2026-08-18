use crate::app_state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use borealis_protocol::{PeerInfo, PeerMapResponse};
use chrono::{DateTime, Utc};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

type PeerRow = (i64, Vec<u8>, String, String, i32, DateTime<Utc>);

pub async fn network_map(
    State(state): State<AppState>,
    Path(node_id): Path<i64>,
) -> Result<Json<PeerMapResponse>, StatusCode> {
    let requester_exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM nodes
            WHERE id = $1
              AND status = 'active'
              AND lease_expires_at > NOW()
        )
        "#,
    )
    .bind(node_id)
    .fetch_one(&state.db)
    .await
    .map_err(internal_error)?;

    if !requester_exists {
        return Err(StatusCode::NOT_FOUND);
    }

    let rows = sqlx::query_as::<_, PeerRow>(
        r#"
        SELECT
            id,
            public_key,
            host(overlay_ip),
            host(observed_ip),
            advertised_port,
            lease_expires_at
        FROM nodes
        WHERE id != $1
          AND status = 'active'
          AND lease_expires_at > NOW()
        ORDER BY overlay_ip
        "#,
    )
    .bind(node_id)
    .fetch_all(&state.db)
    .await
    .map_err(internal_error)?;

    let peers = rows
        .into_iter()
        .map(peer_from_row)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(PeerMapResponse { peers }))
}

fn peer_from_row(row: PeerRow) -> Result<PeerInfo, StatusCode> {
    let (node_id, public_key, overlay_ip, observed_ip, advertised_port, lease_expires_at) = row;

    let public_key = public_key.try_into().map_err(|_| {
        internal_error("database returned a public key that is not exactly 32 bytes")
    })?;
    let overlay_ip = overlay_ip.parse::<Ipv4Addr>().map_err(internal_error)?;
    let observed_ip = observed_ip.parse::<IpAddr>().map_err(internal_error)?;
    let port = u16::try_from(advertised_port).map_err(internal_error)?;

    Ok(PeerInfo {
        node_id,
        public_key,
        overlay_ip,
        endpoint: SocketAddr::new(observed_ip, port),
        lease_expires_at,
    })
}

fn internal_error(error: impl std::fmt::Display) -> StatusCode {
    eprintln!("peer map lookup failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
