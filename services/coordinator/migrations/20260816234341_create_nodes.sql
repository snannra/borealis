CREATE TABLE nodes (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    public_key BYTEA NOT NULL,
    overlay_ip INET NOT NULL,
    observed_ip INET NOT NULL,
    advertised_port INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    lease_expires_at TIMESTAMPTZ NOT NULL,

    CONSTRAINT nodes_public_key_unique UNIQUE (public_key),
    CONSTRAINT nodes_overlay_ip_unique UNIQUE (overlay_ip),
    CONSTRAINT nodes_public_key_length CHECK (octet_length(public_key) = 32),
    CONSTRAINT nodes_overlay_ip_is_ipv4 CHECK (
        family(overlay_ip) = 4 AND masklen(overlay_ip) = 32
    ),
    CONSTRAINT nodes_observed_ip_is_host CHECK (
        masklen(observed_ip) IN (32, 128)
    ),
    CONSTRAINT nodes_advertised_port_valid CHECK (
        advertised_port BETWEEN 1 AND 65535
    ),
    CONSTRAINT nodes_status_valid CHECK (status IN ('active', 'revoked')),
    CONSTRAINT nodes_lease_after_last_seen CHECK (
        lease_expires_at > last_seen_at
    )
);

CREATE INDEX nodes_lease_expires_at_idx ON nodes (lease_expires_at);
