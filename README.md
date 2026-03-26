# cb-sync

クロスプラットフォームのクリップボード同期ツール（Rust）

## 概要

Windows/WSL/Linux/Android間でクリップボードを安全に共有するツール。
常時同期ではなく、コピー後20秒間のみ同期することでセキュリティとプライバシーを確保。

## インストール

### NixOS / Nix

```bash
# 開発環境に入る
nix-shell

# ビルド
cargo build --release

# インストール（~/.cargo/bin/）
cargo install --path crates/cb-cli
```

### その他のLinux

```bash
# Rustが必要
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# ビルド
cargo build --release
```

## 使用方法

```bash
# ヘルプ
cb-sync --help

# ローカルクリップボード操作
cb-sync copy "テキスト"   # クリップボードにコピー
cb-sync paste              # クリップボードから出力

# リモート同期（平文）
cb-sync send <TARGET_IP>        # クリップボードを送信
cb-sync send <TARGET_IP> "text" # テキストを直接送信
cb-sync receive                    # 受信待機（1回）
cb-sync listen                     # 継続受信

# リモート同期（暗号化）
cb-sync -p 'password' send <TARGET_IP>
cb-sync -p 'password' receive

# 暗号化キー生成
cb-sync keygen
cb-sync -k 'base64key...' send <TARGET_IP>

# 環境情報
cb-sync info

# 画像同期
cb-sync send <TARGET_IP> --image     # クリップボードの画像を送信
cb-sync send <TARGET_IP> -f image.png # ファイルから画像を送信
cb-sync receive -o received.png         # 画像を受信してファイルに保存

# 設定ファイル
cb-sync config init   # 設定ファイルを作成
cb-sync config show   # 現在の設定を表示
cb-sync config path   # 設定ファイルのパスを表示
```

### 設定ファイル

`~/.config/cb-sync/config.toml` で設定を保存できます。

```bash
# 設定ファイルを作成
cb-sync config init
```

設定ファイルの例:

```toml
[general]
port = 34812
timeout_secs = 20
verbose = 0

[encryption]
password = "shared-secret"
# または key = "base64キー"

[targets]
default = "<TARGET_IP>"
home = "<HOME_IP>"
work = "<WORK_IP>"
```

名前付きターゲットを使用:

```bash
# @home は config の [targets] home = "..." を参照
cb-sync send @home
cb-sync send @work
```

### 暗号化

cb-syncはChaCha20-Poly1305による暗号化をサポート。

```bash
# パスワードで暗号化
cb-sync -p 'shared-secret' send <TARGET_IP>
cb-sync -p 'shared-secret' listen

# またはキーを生成して使用
cb-sync keygen  # => base64エンコードされたキーを出力
cb-sync -k 'キー' send <TARGET_IP>

# 環境変数も使用可能
export CB_SYNC_PASSWORD='shared-secret'
# または
export CB_SYNC_KEY='base64キー'
cb-sync send <TARGET_IP>
```

### 使用例: 2台のPC間でクリップボード共有

```bash
# PC-A (受信側): <TARGET_IP>
cb-sync -p 'secret123' listen

# PC-B (送信側)
cb-sync -p 'secret123' send <TARGET_IP>
# => PC-Aのクリップボードに自動コピーされる
```

## 対応プラットフォーム

| プラットフォーム | 状態 |
|------------------|------|
| Linux (Wayland)  | 対応 |
| Linux (X11)      | 対応 |
| Windows          | 対応 |
| Android          | 未実装 |

## プロジェクト構成

```
cb-sync/
├── Cargo.toml           # ワークスペース定義
├── shell.nix            # NixOS開発環境
├── crates/
│   ├── cb-core/         # コアライブラリ
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── clipboard.rs  # クリップボード抽象化
│   │       ├── config.rs     # 設定ファイル
│   │       ├── crypto.rs     # 暗号化
│   │       ├── protocol.rs   # メッセージ型
│   │       ├── sync.rs       # TCP送受信
│   │       └── error.rs      # エラー型
│   └── cb-cli/          # CLIアプリケーション
│       └── src/
│           └── main.rs
└── docs/
    ├── PLAN.md          # 開発計画
    ├── PROGRESS.md      # 進捗ログ
    └── DECISIONS.md     # 設計決定記録
```

## 技術仕様

### プロトコル

- TCP通信（デフォルトポート: 34812）
- JSON形式メッセージ
- 20秒タイムアウト
- ChaCha20-Poly1305暗号化（オプション）

### メッセージ形式

```json
// 平文
{"type":"clipboard","text":"内容","timestamp":1234567890}

// 暗号化
{"type":"encrypted","data":"base64暗号文","timestamp":1234567890}

// 応答
{"type":"ack"}
```

## ロードマップ

### Phase 1（MVP）- 完了
- [x] テキスト同期
- [x] Linux (Wayland/X11) 対応
- [x] LAN内P2P通信
- [x] 20秒タイムアウト
- [x] 暗号化（ChaCha20-Poly1305）

### Phase 2 - 完了
- [x] 設定ファイル
- [x] Windows対応
- [x] 画像同期

### Phase 3
- [ ] OS間パス翻訳
- [ ] Android対応

### Phase 4
- [ ] AI変換パイプライン
- [ ] プラグインシステム

## セキュリティ設計

- 常時常駐しない（明示的に起動）
- 20秒後に接続タイムアウト
- LAN内のみ（インターネット経由なし）
- ChaCha20-Poly1305暗号化（-p/-k オプション）
- パスワード/キーは環境変数でも指定可能

## ライセンス

MIT
