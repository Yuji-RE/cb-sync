# cliplink

クロスプラットフォームのクリップボード同期ツール（Rust）

## 概要

Windows/WSL/Linux/Android間でクリップボードを安全に共有するツール。
常時同期ではなく、コピー後20秒間のみ同期することでセキュリティとプライバシーを確保。

## 対応データ形式

- テキスト
- 画像（PNG/JPEG）
- URL/リンク
- ファイルパス（OS間翻訳対応）

## アーキテクチャ

```
┌─────────────────────────────────────┐
│         Core Library (Rust)         │
│  - クリップボード抽象化              │
│  - シリアライゼーション（画像含む）   │
│  - 暗号化・同期プロトコル            │
│  - Path翻訳ロジック                  │
│  - AI変換パイプライン（将来）        │
└─────────────────────────────────────┘
        ↓           ↓           ↓
   [Linux CLI]  [Windows exe]  [Android JNI]
```

## 同期フロー

```
コピー検知
    ↓
20秒タイマー開始 → 他デバイスへbroadcast（暗号化）
    ↓
タイムアウト → 同期停止、メモリクリア
```

## 主要クレート（予定）

| クレート | 用途 |
|----------|------|
| `arboard` | クロスプラットフォームクリップボード |
| `tokio` | 非同期ランタイム |
| `image` | 画像処理 |
| `serde` + `bincode` | シリアライゼーション |
| `ring` or `rustls` | 暗号化 |
| `mdns` | デバイス発見（オプション） |

## 機能一覧

### Phase 1（MVP）
- [ ] テキスト同期
- [ ] Linux (Wayland) 対応
- [ ] Windows 対応
- [ ] LAN内P2P通信
- [ ] 20秒タイムアウト

### Phase 2
- [ ] 画像同期
- [ ] URL/リンク対応
- [ ] TLS暗号化
- [ ] 設定ファイル対応

### Phase 3
- [ ] OS間パス翻訳
  - `/home/user/...` ⇔ `C:\Users\user\...`
  - WSLパス対応 (`/mnt/c/...`)
- [ ] Android対応（JNI）

### Phase 4（将来）
- [ ] AI変換パイプライン
  - テキスト変換（翻訳、要約等）
  - 画像変換（リサイズ、フォーマット変換等）
- [ ] プラグインシステム

## セキュリティ設計

- 常時常駐しない（コピー検知時のみアクティブ）
- 20秒後に自動でメモリクリア
- LAN内のみ（インターネット経由なし）
- E2E暗号化
- ログ最小化

## プロジェクト構成（予定）

```
cliplink/
├── Cargo.toml
├── cliplink-core/       # コアライブラリ
│   ├── src/
│   │   ├── clipboard/   # クリップボード抽象化
│   │   ├── protocol/    # 同期プロトコル
│   │   ├── crypto/      # 暗号化
│   │   ├── transform/   # パス変換、AI変換
│   │   └── lib.rs
│   └── Cargo.toml
├── cliplink-cli/        # CLI（Linux/Windows）
│   ├── src/
│   │   └── main.rs
│   └── Cargo.toml
└── cliplink-android/    # Android用（将来）
    └── ...
```

## 使用例（想定）

```bash
# デーモン起動
cliplink daemon

# 手動同期
cliplink send
cliplink receive

# パス変換
cliplink path-translate "/home/user/file.txt" --to windows
# => C:\Users\user\file.txt

# 設定
cliplink config set timeout 30
cliplink config set encryption on
```

## ライセンス

MIT（予定）
