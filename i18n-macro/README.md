# i18n-macro

以 Rust 語法編寫、在編譯期檢查的型別化 i18n macro。fallback 語言定義完整
schema，其他語言可以只翻譯需要的訊息；呼叫端透過 `tr!` 或 `tr_for!`
取得翻譯。

目前這個 crate 只提供 macro 核心，尚未用於 OJ 現有的 `config.yaml`、
系統語系偵測及 fallback 警告流程。

## 可以做什麼

- 靜態訊息與帶參數訊息。
- Rust `format_args!` 格式，例如 `{value:.2}` 與 `{{`／`}}`。
- 使用 Rust 原生 `if`、`match` 與 block 選擇翻譯。
- 在 block 中建立區域變數、進行前置計算。
- 在編譯期檢查訊息 key、參數名稱、缺漏參數及明確型別。
- 非 fallback 語言可省略訊息，整筆使用 fallback 翻譯。
- `tr!` 使用目前語言；`tr_for!` 使用指定語言。
- 靜態訊息回傳 `&'static str`，動態訊息回傳實作 `Display` 的值。

## 快速開始

### `src/en_us.rs`

fallback catalog 定義所有可用訊息：

```rust
use i18n_macro::catalog;

catalog! {
    fallback;

    About = "Evaluator";
    Hello { name } = "Hello, {name}";
    FileCount { count: usize } =
        match count {
            0 => "No files",
            n => "{n} files",
        };
}
```

### `src/zh_tw.rs`

其他語言指向 fallback schema，未列出的 `FileCount` 會自動使用英文：

```rust
use i18n_macro::catalog;

catalog! {
    schema: super::en_us;

    About = "評測器";
    Hello { name } = "你好，{name}";
}
```

### `src/lib.rs`

在 crate root 組合語言並提供目前語言：

```rust
use i18n_macro::define_i18n;

mod en_us;
mod zh_tw;

pub fn current_locale() -> Locale {
    Locale::EnUs
}

define_i18n! {
    locale: pub Locale;
    current_locale: current_locale;
    schema: en_us;
    fallback EnUs: en_us;
    locale ZhTw: zh_tw;
}
```

`Locale`、`tr!` 與 `tr_for!` 都由 `define_i18n!` 產生：

```rust
let about: &'static str = tr!(About);
println!("{about}");

let hello = tr_for!(Locale::ZhTw, Hello { name: "小明" });
println!("{hello}"); // 你好，小明
```

## Catalog 寫法

訊息右側可以使用字串、`if`、`match` 或帶有前置計算的 block：

```rust
catalog! {
    fallback;

    // 靜態訊息
    About = "關於";

    // 插值與 Rust formatting
    Score { value } = "分數：{value:.2}";
    Braces { value } = "{{{value}}}";

    // 明確型別參數可以用於條件
    Status { disabled: bool } =
        if disabled { "停用" } else { "啟用" };

    // match pattern 中建立的變數也能插值
    FileCount { count: usize } =
        match count {
            0 => "沒有檔案",
            1 => "1 個檔案",
            n => "{n} 個檔案",
        };

    // block 可先建立區域變數，最後選出字串
    Summary { correct: usize, incorrect: usize } = {
        let total = correct + incorrect;
        if total == 0 {
            "尚無結果"
        } else {
            "正確 {correct}，共 {total} 題"
        }
    };
}
```

每個正常顯示分支最後都應是字串 literal。格式內容遵循 Rust formatting
語法，因此可以使用寬度、精度、重複捕捉及大括號跳脫。

## 參數

```rust
Message { enabled: bool, name } =
    if enabled { "{name} 已啟用" } else { "{name} 已停用" };
```

- `{ enabled: bool }` 是明確型別參數。它在 expression 中是 `bool`，
  可直接用於 `if` 或 `match`，型別必須實作 `Copy`。
- `{ name }` 是未標型別參數，只要求實作 `Display`，適合插值。
  呼叫端可以傳入 owned value、`&str` 或其他引用。

不同語言的參數名稱與明確型別必須相同，但宣告順序可以不同。非 fallback
catalog 可以完全省略某筆訊息；若明確寫成 `About = "";`，結果是空字串，
不會 fallback。

## 目前限制

- 明確型別參數必須實作 `Copy`；一般借用值請使用未標型別參數。
- catalog expression 目前以字串、block、`if` 和 `match` 為支援範圍。
- `define_i18n!` 必須放在 crate root。
- 尚未提供 plural rule；單複數可先用 Rust `match` 表達。

更完整的設計與實作限制請參考
[Rust-native typed i18n catalog design](../docs/superpowers/specs/2026-07-25-rust-native-i18n-catalog-design.md)。
