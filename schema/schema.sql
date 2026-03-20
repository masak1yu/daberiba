-- Matrix サーバ スキーマ
-- sqldef (mariadb-def) で管理

CREATE TABLE IF NOT EXISTS users (
    user_id       VARCHAR(255)  NOT NULL COMMENT '@localpart:server_name',
    password_hash VARCHAR(255)  NOT NULL,
    created_at    DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deactivated   TINYINT(1)    NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS access_tokens (
    token      VARCHAR(255)  NOT NULL,
    user_id    VARCHAR(255)  NOT NULL,
    device_id  VARCHAR(255)  NOT NULL,
    created_at DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (token),
    INDEX idx_access_tokens_user_id (user_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS rooms (
    room_id         VARCHAR(255)  NOT NULL COMMENT '!opaque:server_name',
    creator_user_id VARCHAR(255)  NOT NULL,
    name            VARCHAR(255)  NULL,
    topic           TEXT          NULL,
    created_at      DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (room_id),
    FOREIGN KEY (creator_user_id) REFERENCES users(user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS room_memberships (
    room_id    VARCHAR(255)  NOT NULL,
    user_id    VARCHAR(255)  NOT NULL,
    membership VARCHAR(32)   NOT NULL COMMENT 'join | leave | invite | ban | knock',
    updated_at DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (room_id, user_id),
    INDEX idx_room_memberships_user_id (user_id),
    FOREIGN KEY (room_id) REFERENCES rooms(room_id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS events (
    event_id   VARCHAR(255)  NOT NULL COMMENT '$opaque:server_name',
    room_id    VARCHAR(255)  NOT NULL,
    sender     VARCHAR(255)  NOT NULL,
    event_type VARCHAR(255)  NOT NULL,
    state_key  VARCHAR(255)  NULL COMMENT 'NULLはtimeline event',
    content    MEDIUMTEXT    NOT NULL,
    created_at DATETIME(3)   NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    PRIMARY KEY (event_id),
    INDEX idx_events_room_id (room_id, created_at),
    FOREIGN KEY (room_id) REFERENCES rooms(room_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS room_state (
    room_id    VARCHAR(255)  NOT NULL,
    event_type VARCHAR(255)  NOT NULL,
    state_key  VARCHAR(255)  NOT NULL,
    event_id   VARCHAR(255)  NOT NULL,
    PRIMARY KEY (room_id, event_type, state_key),
    FOREIGN KEY (room_id)  REFERENCES rooms(room_id)   ON DELETE CASCADE,
    FOREIGN KEY (event_id) REFERENCES events(event_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
