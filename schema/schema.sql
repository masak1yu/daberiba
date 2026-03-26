-- Matrix サーバ スキーマ
-- sqldef (mysqldef) で管理

CREATE TABLE IF NOT EXISTS users (
    user_id       VARCHAR(255)  NOT NULL COMMENT '@localpart:server_name',
    password_hash VARCHAR(255)  NOT NULL,
    display_name  VARCHAR(255)  NULL,
    avatar_url    VARCHAR(1024) NULL,
    created_at    DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deactivated   TINYINT(1)    NOT NULL DEFAULT 0,
    admin         TINYINT(1)    NOT NULL DEFAULT 0 COMMENT '管理者フラグ',
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
    creator_user_id VARCHAR(255)  NULL COMMENT 'NULLはfederationから招待されたルーム（プレースホルダー）',
    room_version    VARCHAR(16)   NOT NULL DEFAULT '10' COMMENT 'Matrix room version (1-10)',
    name            VARCHAR(255)  NULL,
    topic           TEXT          NULL,
    created_at      DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (room_id),
    FOREIGN KEY (creator_user_id) REFERENCES users(user_id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS room_memberships (
    room_id    VARCHAR(255)  NOT NULL,
    user_id    VARCHAR(255)  NOT NULL,
    membership VARCHAR(32)   NOT NULL COMMENT 'join | leave | invite | ban | knock',
    invited_by VARCHAR(255)  NULL     COMMENT 'invite 時の送信者 user_id',
    updated_at DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (room_id, user_id),
    INDEX idx_room_memberships_user_id (user_id),
    FOREIGN KEY (room_id) REFERENCES rooms(room_id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS events (
    event_id          VARCHAR(255)    NOT NULL COMMENT '$opaque:server_name',
    room_id           VARCHAR(255)    NOT NULL,
    sender            VARCHAR(255)    NOT NULL,
    event_type        VARCHAR(255)    NOT NULL,
    state_key         VARCHAR(255)    NULL COMMENT 'NULLはtimeline event',
    content           MEDIUMTEXT      NOT NULL,
    auth_events       TEXT            NULL COMMENT 'federation PDU の auth_events（JSON 配列）',
    prev_events       TEXT            NULL COMMENT 'federation PDU の prev_events（JSON 配列）',
    depth             BIGINT          NOT NULL DEFAULT 0 COMMENT 'DAG の深さ（m.room.create = 1、以降インクリメント）',
    origin_server_ts  BIGINT          NULL COMMENT 'federation PDU のオリジナルタイムスタンプ (ms)。ローカルイベントは NULL',
    created_at        DATETIME(3)     NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    stream_ordering   BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    PRIMARY KEY (event_id),
    UNIQUE KEY uq_events_stream_ordering (stream_ordering),
    INDEX idx_events_room_id (room_id, stream_ordering),
    FOREIGN KEY (room_id) REFERENCES rooms(room_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS devices (
    device_id    VARCHAR(255)  NOT NULL,
    user_id      VARCHAR(255)  NOT NULL,
    display_name VARCHAR(255)  NULL,
    last_seen_ts BIGINT        NULL COMMENT 'Unix milliseconds',
    last_seen_ip VARCHAR(64)   NULL,
    created_at   DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (device_id, user_id),
    INDEX idx_devices_user_id (user_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS media (
    media_id     VARCHAR(255)  NOT NULL,
    server_name  VARCHAR(255)  NOT NULL,
    user_id      VARCHAR(255)  NOT NULL,
    content_type VARCHAR(255)  NOT NULL,
    filename     VARCHAR(255)  NULL,
    file_size    BIGINT        NOT NULL,
    room_id      VARCHAR(255)  NULL COMMENT 'NULL=全認証ユーザーアクセス可、非NULL=ルームメンバーのみ',
    created_at   DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (media_id, server_name),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE,
    FOREIGN KEY (room_id) REFERENCES rooms(room_id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS pushers (
    app_id              VARCHAR(255) NOT NULL COMMENT 'アプリ識別子',
    pushkey             VARCHAR(512) NOT NULL COMMENT 'デバイストークン等',
    user_id             VARCHAR(255) NOT NULL,
    kind                VARCHAR(32)  NOT NULL COMMENT 'http | email',
    app_display_name    VARCHAR(255) NOT NULL,
    device_display_name VARCHAR(255) NOT NULL,
    lang                VARCHAR(32)  NOT NULL,
    data                TEXT         NOT NULL COMMENT 'JSON {"url": "..."}',
    created_at          DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (app_id, pushkey),
    INDEX idx_pushers_user_id (user_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS receipts (
    room_id      VARCHAR(255) NOT NULL,
    user_id      VARCHAR(255) NOT NULL,
    receipt_type VARCHAR(64)  NOT NULL COMMENT 'm.read | m.read.private',
    event_id     VARCHAR(255) NOT NULL,
    ts           BIGINT       NOT NULL COMMENT 'Unix milliseconds',
    PRIMARY KEY (room_id, user_id, receipt_type),
    INDEX idx_receipts_room_id (room_id),
    FOREIGN KEY (room_id) REFERENCES rooms(room_id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS room_aliases (
    alias      VARCHAR(255)  NOT NULL COMMENT '#localpart:server_name',
    room_id    VARCHAR(255)  NOT NULL,
    creator    VARCHAR(255)  NOT NULL,
    PRIMARY KEY (alias),
    INDEX idx_room_aliases_room_id (room_id),
    FOREIGN KEY (room_id)  REFERENCES rooms(room_id)  ON DELETE CASCADE,
    FOREIGN KEY (creator)  REFERENCES users(user_id)  ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS presence (
    user_id        VARCHAR(255)  NOT NULL,
    presence       VARCHAR(32)   NOT NULL COMMENT 'online | offline | unavailable',
    status_msg     TEXT          NULL,
    last_active_ts BIGINT        NOT NULL COMMENT 'Unix milliseconds',
    PRIMARY KEY (user_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS room_tags (
    user_id  VARCHAR(255)  NOT NULL,
    room_id  VARCHAR(255)  NOT NULL,
    tag      VARCHAR(255)  NOT NULL COMMENT 'm.favourite | m.lowpriority | u.custom 等',
    order_   DOUBLE        NULL     COMMENT '0.0 - 1.0 の任意ソート順',
    PRIMARY KEY (user_id, room_id, tag),
    INDEX idx_room_tags_user_id (user_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE,
    FOREIGN KEY (room_id) REFERENCES rooms(room_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS filters (
    filter_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id   VARCHAR(255)   NOT NULL,
    filter    MEDIUMTEXT     NOT NULL COMMENT 'JSON filter 定義',
    PRIMARY KEY (filter_id),
    INDEX idx_filters_user_id (user_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS device_keys (
    user_id    VARCHAR(255) NOT NULL,
    device_id  VARCHAR(255) NOT NULL,
    key_json   MEDIUMTEXT   NOT NULL COMMENT 'デバイス公開鍵 JSON（algorithms, keys, signatures 含む）',
    updated_at DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, device_id),
    INDEX idx_device_keys_user_id (user_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS one_time_keys (
    id        BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id   VARCHAR(255)    NOT NULL,
    device_id VARCHAR(255)    NOT NULL,
    key_id    VARCHAR(255)    NOT NULL COMMENT 'e.g. signed_curve25519:AAAAAQ',
    key_json  MEDIUMTEXT      NOT NULL COMMENT 'key value（object or string）',
    PRIMARY KEY (id),
    UNIQUE KEY uq_otk (user_id, device_id, key_id),
    INDEX idx_otk_user_device (user_id, device_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS to_device_messages (
    id         BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    sender     VARCHAR(255)    NOT NULL,
    recipient  VARCHAR(255)    NOT NULL,
    device_id  VARCHAR(255)    NOT NULL COMMENT '* = 全デバイス',
    event_type VARCHAR(255)    NOT NULL,
    content    MEDIUMTEXT      NOT NULL COMMENT 'JSON',
    txn_id     VARCHAR(255)    NOT NULL,
    created_at DATETIME        NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    INDEX idx_to_device_recipient (recipient, id),
    FOREIGN KEY (recipient) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS account_data (
    user_id    VARCHAR(255) NOT NULL,
    room_id    VARCHAR(255) NOT NULL DEFAULT '' COMMENT '空文字=グローバル、非空=ルーム固有',
    event_type VARCHAR(255) NOT NULL,
    content    MEDIUMTEXT   NOT NULL COMMENT 'JSON',
    updated_at DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3),
    PRIMARY KEY (user_id, room_id, event_type),
    INDEX idx_account_data_user_id (user_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
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

CREATE TABLE IF NOT EXISTS room_key_backup_versions (
    id         BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id    VARCHAR(255)    NOT NULL,
    algorithm  VARCHAR(255)    NOT NULL,
    auth_data  MEDIUMTEXT      NOT NULL COMMENT 'JSON',
    deleted    TINYINT(1)      NOT NULL DEFAULT 0,
    created_at DATETIME        NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    INDEX idx_rkbv_user_id (user_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS server_signing_key (
    key_id      VARCHAR(255) NOT NULL COMMENT 'e.g. ed25519:auto',
    private_key VARCHAR(512) NOT NULL COMMENT 'unpadded base64 encoded 32-byte private key',
    created_at  DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (key_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- push rule ハイライト評価結果を記録するテーブル。
-- dispatch_push で highlight tweak が付いた場合に挿入し、unread_highlights の
-- highlight_count 計算に使用する。既読送信時は receipt テーブルで管理するため
-- このテーブルの cleanup は行わない（COUNT は receipts との結合で絞り込む）。
CREATE TABLE IF NOT EXISTS unread_highlights (
    room_id          VARCHAR(255) NOT NULL,
    user_id          VARCHAR(255) NOT NULL,
    event_id         VARCHAR(255) NOT NULL,
    stream_ordering  BIGINT       NOT NULL,
    PRIMARY KEY (room_id, user_id, event_id),
    INDEX idx_uh_room_user_ordering (room_id, user_id, stream_ordering),
    FOREIGN KEY (room_id) REFERENCES rooms(room_id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- プッシュ通知履歴テーブル。dispatch_push で notify アクションが発火した際に挿入する。
-- GET /notifications の応答源。read_at は receipt POST 時に更新する。
CREATE TABLE IF NOT EXISTS notifications (
    id          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    user_id     VARCHAR(255)    NOT NULL,
    room_id     VARCHAR(255)    NOT NULL,
    event_id    VARCHAR(255)    NOT NULL,
    read_at     BIGINT          NULL,
    notified_at BIGINT          NOT NULL,
    PRIMARY KEY (id),
    INDEX idx_notifications_user (user_id, id),
    INDEX idx_notifications_room_event (room_id, event_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- イベントリレーションテーブル。
-- m.relates_to を持つイベントを events に挿入する際に同時に記録する。
-- GET /relations の応答源。
CREATE TABLE IF NOT EXISTS event_relations (
    event_id           VARCHAR(255) NOT NULL COMMENT 'リレーション元イベント（新しい側）',
    room_id            VARCHAR(255) NOT NULL,
    rel_type           VARCHAR(128) NOT NULL COMMENT 'm.replace / m.reaction / m.thread 等',
    relates_to_event_id VARCHAR(255) NOT NULL COMMENT 'リレーション先イベント（参照される側）',
    PRIMARY KEY (event_id),
    INDEX idx_er_target (relates_to_event_id, rel_type, event_id),
    FOREIGN KEY (room_id) REFERENCES rooms(room_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- ログイントークンテーブル。POST /v1/login/get_token で発行し、m.login.token フローで消費する。
-- 有効期限 120 秒・シングルユース。
CREATE TABLE IF NOT EXISTS login_tokens (
    token      VARCHAR(255) NOT NULL,
    user_id    VARCHAR(255) NOT NULL,
    expires_at BIGINT       NOT NULL COMMENT 'Unix milliseconds',
    used       TINYINT(1)   NOT NULL DEFAULT 0,
    PRIMARY KEY (token),
    INDEX idx_login_tokens_user_id (user_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- サードパーティ ID テーブル。GET/POST /account/3pid* で管理する。
CREATE TABLE IF NOT EXISTS user_threepids (
    user_id      VARCHAR(255) NOT NULL,
    medium       VARCHAR(32)  NOT NULL COMMENT 'email | msisdn',
    address      VARCHAR(255) NOT NULL,
    validated_at BIGINT       NOT NULL COMMENT 'Unix milliseconds',
    added_at     BIGINT       NOT NULL COMMENT 'Unix milliseconds',
    PRIMARY KEY (medium, address),
    INDEX idx_user_threepids_user_id (user_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

CREATE TABLE IF NOT EXISTS room_key_backup_sessions (
    version             BIGINT UNSIGNED NOT NULL,
    user_id             VARCHAR(255)    NOT NULL,
    room_id             VARCHAR(255)    NOT NULL,
    session_id          VARCHAR(255)    NOT NULL,
    first_message_index INT             NOT NULL DEFAULT 0,
    forwarded_count     INT             NOT NULL DEFAULT 0,
    is_verified         TINYINT(1)      NOT NULL DEFAULT 0,
    session_data        MEDIUMTEXT      NOT NULL COMMENT 'JSON',
    PRIMARY KEY (version, user_id, room_id, session_id),
    INDEX idx_rkbs_version_user (version, user_id),
    FOREIGN KEY (version) REFERENCES room_key_backup_versions(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- コンテンツ報告テーブル（POST /rooms/{roomId}/report/{eventId}）
CREATE TABLE IF NOT EXISTS event_reports (
    id         BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    room_id    VARCHAR(255)    NOT NULL,
    event_id   VARCHAR(255)    NOT NULL,
    user_id    VARCHAR(255)    NOT NULL COMMENT '報告者',
    score      INT             NULL     COMMENT '-100 (最悪) ～ 0 (問題なし)',
    reason     TEXT            NULL,
    created_at DATETIME        NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    INDEX idx_event_reports_room (room_id),
    INDEX idx_event_reports_event (event_id),
    INDEX idx_event_reports_user (user_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
