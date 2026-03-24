# Handover — frontend (daberiba Matrix クライアント)

## 概要

daberiba バックエンド（Matrix Client-Server API）に対応する TypeScript 製 PWA クライアント。
iOS/Android でブックマークするとネイティブアプリと同等の挙動（ホーム画面追加・フルスクリーン起動）を実現する。

Element Web（https://github.com/element-hq/element-web）を技術選定の参考にしている。

---

## ブランチ戦略

| ブランチ | 役割 |
|---|---|
| `main` | バックエンドのリリース済みタグのみ |
| `frontend` | フロントエンドのベースブランチ（`main` から分岐） |
| `feat/<機能名>` | フロントエンドの機能ブランチ（`frontend` から分岐） |

### フィーチャーブランチの命名例

```
feat/login          # 認証フロー
feat/room-list      # ルーム一覧 + sync
feat/timeline       # タイムラインとメッセージ送信
feat/pwa-setup      # PWA 初期設定（manifest, SW, アイコン）
feat/mobile-ux      # モバイル UX（SafeArea, スワイプ等）
```

フォーマット: `feat/<kebab-case>` — 機能が分かれば命名は自由。

### バックエンドへの追従

バックエンドの `feature/vX.X.0` が更新されるたびに `frontend` をリベースして追従する。

```sh
# バックエンドブランチが更新されたとき
git checkout frontend
git rebase feature/v0.X.0
# 以降の feat/* ブランチも必要に応じてリベース
git checkout feat/timeline
git rebase frontend
```

> `frontend` ブランチは `front/` 以下のみ変更するため、バックエンドとのコンフリクトは基本的に発生しない。

---

## 技術スタック

Element Web の構成（React 19 + TypeScript + pnpm + matrix-js-sdk + Jest + ESLint + Prettier）を参考に、
より小規模なシングルアプリ向けに軽量化した構成を採用する。

| 層 | 採用技術 | Element Web での採用 / 選定理由 |
|---|---|---|
| 言語 | TypeScript 5 | ○ 採用（strict モード） |
| フレームワーク | React 19 | ○ 採用 |
| パッケージマネージャ | pnpm | ○ 採用（pnpm v10） |
| ビルドツール | Vite 6 | 独自（Element は Nx） — 単体アプリには Vite が軽量 |
| PWA | vite-plugin-pwa + Workbox | Element にはなし — 追加価値として実装 |
| Matrix SDK | matrix-js-sdk | ○ 採用（Element のコア） |
| 状態管理 | Zustand | 独自（Jotai/Context でも可） |
| ルーティング | React Router v7 | 独自 |
| スタイリング | Tailwind CSS v4 | 独自（モバイルファースト設計が容易） |
| テスト | Jest + Testing Library | ○ 採用（Element と同じ） |
| Lint | ESLint + Prettier | ○ 採用 |
| Git フック | Husky + lint-staged | ○ 採用 |
| 型チェック | tsc --noEmit | ○ 採用 |

---

## ディレクトリ構成

```
front/
├── public/
│   ├── icons/               # PWA アイコン (192x192, 512x512, maskable)
│   └── screenshots/         # PWA ストア向けスクリーンショット（任意）
├── src/
│   ├── api/                 # matrix-js-sdk ラッパー
│   │   ├── client.ts        # MatrixClient シングルトン
│   │   ├── auth.ts          # ログイン・ログアウト
│   │   └── sync.ts          # sync ループ管理
│   ├── stores/              # Zustand ストア
│   │   ├── auth.ts          # アクセストークン・ユーザー情報
│   │   ├── rooms.ts         # ルーム一覧・タイムライン
│   │   └── ui.ts            # パネル開閉などの UI 状態
│   ├── components/
│   │   ├── layout/          # AppShell, Sidebar, Header
│   │   ├── room/            # RoomList, Timeline, MessageInput
│   │   └── common/          # Avatar, Spinner, Modal
│   ├── pages/
│   │   ├── LoginPage.tsx
│   │   ├── HomePage.tsx     # ルーム一覧
│   │   └── RoomPage.tsx     # タイムライン + 入力欄
│   ├── main.tsx
│   └── vite-env.d.ts
├── index.html
├── vite.config.ts
├── tsconfig.json
├── package.json
└── handover.md              # このファイル
```

---

## PWA 要件と実装方針

### Web App Manifest

`vite-plugin-pwa` が `vite.config.ts` の設定から `manifest.webmanifest` を自動生成する。

```ts
// vite.config.ts (抜粋)
VitePWA({
  registerType: 'autoUpdate',
  manifest: {
    name: 'daberiba',
    short_name: 'daberiba',
    description: 'Matrix chat client',
    theme_color: '#1a1a2e',
    background_color: '#1a1a2e',
    display: 'standalone',          // ← ネイティブアプリ風に起動
    orientation: 'portrait',
    start_url: '/',
    icons: [
      { src: '/icons/192.png', sizes: '192x192', type: 'image/png' },
      { src: '/icons/512.png', sizes: '512x512', type: 'image/png' },
      { src: '/icons/512-maskable.png', sizes: '512x512', type: 'image/png', purpose: 'maskable' },
    ],
  },
  workbox: {
    // アプリシェル（HTML/JS/CSS）をキャッシュ → オフライン起動
    globPatterns: ['**/*.{js,css,html,ico,png,svg,woff2}'],
    // Matrix API リクエストは network-first
    runtimeCaching: [
      {
        urlPattern: /^\/_matrix\//,
        handler: 'NetworkFirst',
        options: { cacheName: 'matrix-api', networkTimeoutSeconds: 5 },
      },
    ],
  },
})
```

### iOS Safari 対応

`index.html` に以下を追加（Safari は Manifest の一部プロパティを無視するため）:

```html
<meta name="apple-mobile-web-app-capable" content="yes">
<meta name="apple-mobile-web-app-status-bar-style" content="black-translucent">
<meta name="apple-mobile-web-app-title" content="daberiba">
<link rel="apple-touch-icon" href="/icons/192.png">
```

### PWA インストール条件チェックリスト

| 条件 | 実装 |
|---|---|
| HTTPS または localhost | 開発: Vite dev server、本番: nginx/caddy で TLS 終端 |
| Web App Manifest | vite-plugin-pwa が自動生成 |
| Service Worker | Workbox ベースの SW |
| display: standalone | manifest に設定済み |
| 192×192 以上のアイコン | `public/icons/` に配置 |

---

## 実装フェーズ

各フェーズは `feat/<名前>` ブランチで作業し、`frontend` にマージする。

### Phase 1 — プロジェクト初期化（`feat/pwa-setup`）

1. `front/` 内で `pnpm create vite@latest . -- --template react-ts` を実行
2. 依存追加: `vite-plugin-pwa`, `matrix-js-sdk`, `zustand`, `react-router-dom`, `tailwindcss`
3. Dev 依存: `jest`, `@testing-library/react`, `eslint`, `prettier`, `husky`, `lint-staged`
4. `vite.config.ts` に PWA 設定を追加
5. `index.html` に Apple メタタグを追加
6. アイコン画像を `public/icons/` に配置
7. `pnpm dev` で起動確認 + `pnpm build && pnpm preview` で PWA 動作確認

### Phase 2 — 認証（`feat/login`）

実装対象 API:
- `POST /_matrix/client/v3/login` — パスワードログイン
- `POST /_matrix/client/v3/logout`
- `GET /_matrix/client/v3/account/whoami`

実装内容:
- `src/api/auth.ts`: MatrixClient を用いたログイン・トークン永続化（localStorage）
- `src/stores/auth.ts`: アクセストークン・ユーザー ID を保持
- `LoginPage.tsx`: ログインフォーム
- ルートガード: 未認証時は `/login` にリダイレクト

### Phase 3 — ルーム一覧と sync（`feat/room-list`）

実装対象 API:
- `GET /_matrix/client/v3/sync`
- `GET /_matrix/client/v3/joined_rooms`

実装内容:
- `src/api/sync.ts`: long-polling sync ループ（`since` カーソル管理）
- `src/stores/rooms.ts`: ルーム一覧・未読カウント・最終メッセージ
- `HomePage.tsx`: ルーム一覧表示（未読バッジ付き）

### Phase 4 — タイムラインとメッセージ送信（`feat/timeline`）

実装対象 API:
- `GET /_matrix/client/v3/rooms/{roomId}/messages`
- `PUT /_matrix/client/v3/rooms/{roomId}/send/m.room.message/{txnId}`
- `GET /_matrix/client/v3/rooms/{roomId}/context/{eventId}`

実装内容:
- `RoomPage.tsx`: 仮想スクロール付きタイムライン
- `MessageInput.tsx`: テキスト入力・送信（txnId は `crypto.randomUUID()`）
- 過去メッセージの遡りページング（`from` トークン）

### Phase 5 — モバイル UX 仕上げ（`feat/mobile-ux`）

- SafeArea 対応（iOS ノッチ・ホームインジケーター）
- スワイプでサイドバー開閉（Sidebar は drawer 形式）
- スプラッシュスクリーン（SW キャッシュ済みなら即起動）
- オフライン時のトースト通知

### Phase 6 以降 — 追加機能（バックエンド実装に合わせて随時追加）

| バックエンド API | フロント実装 | ブランチ例 |
|---|---|---|
| `POST /createRoom` | 新規ルーム作成ダイアログ | `feat/create-room` |
| `POST /invite` | メンバー招待 | `feat/invite` |
| `POST /rooms/{roomId}/leave` | ルーム退出 | `feat/leave-room` |
| `GET /rooms/{roomId}/members` | メンバーリスト表示 | `feat/members` |
| `GET/PUT /devices` | デバイス管理画面 | `feat/devices` |
| `POST /account/password` | パスワード変更 | `feat/account` |

---

## 開発コマンド

```sh
cd front

# 依存インストール
pnpm install

# 開発サーバー起動（localhost:5173）
pnpm dev

# 型チェック
pnpm type-check   # tsc --noEmit

# Lint
pnpm lint         # eslint + prettier check

# ビルド
pnpm build        # dist/ に出力

# プレビュー（PWA 動作確認 — SW が有効になる）
pnpm preview      # localhost:4173

# テスト
pnpm test
```

---

## 既知の課題・注意点

| 項目 | 詳細 |
|---|---|
| iOS Safari の SW 制限 | Background Sync / Push Notification は iOS 16.4+ 以降のみ対応 |
| matrix-js-sdk のバンドルサイズ | 数 MB になるため code-splitting が必要（React.lazy + Suspense） |
| E2EE | バックエンドが鍵交換のみのため、クライアント側 Olm/Megolm は Phase 6 以降 |
| txnId の冪等性 | 送信リトライ時は同じ txnId を再利用する必要がある |
| sync の長時間接続 | モバイルでバックグラウンドに入ると接続が切れる → 復帰時に再接続する実装が必要 |

---

## 次のアクション（Phase 1 を開始するには）

```sh
git checkout frontend
git checkout -b feat/pwa-setup
cd /workspaces/daberiba/front
pnpm create vite@latest . -- --template react-ts
pnpm add matrix-js-sdk zustand react-router-dom
pnpm add -D vite-plugin-pwa workbox-window tailwindcss @tailwindcss/vite
pnpm add -D jest @testing-library/react @testing-library/jest-dom
pnpm add -D eslint prettier husky lint-staged
```
