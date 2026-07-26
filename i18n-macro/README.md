# i18n-macro

以 Rust 語法編寫、在編譯期檢查的型別化 i18n macro。default catalog 定義所有
訊息的完整呼叫 schema；其他語言只需列出自己翻譯會用到的欄位。這個 crate 目前
只提供 macro 核心，尚未接到 OJ 的 `config.yaml`、系統語系偵測或警告流程。

## 快速開始

default 定義 key、可呼叫的欄位與型別：

```rust
use i18n_macro::catalog;

catalog! {
    default;

    About = "Evaluator";
    Heartbeat {} = "alive";
    Hello { name } = "Hello, {name}";
    Count { count: usize, total: usize } = match count {
        0 => "No files",
        n => "{n} files",
    };
}
```

`A = "..."` 是靜態訊息；只要寫出 braces，`A {}` 也是動態訊息，即使沒有欄位，呼叫時
要寫 `tr!(A {})`。locale 指向 default schema；它的 `{ ... }` 可只列出翻譯 expression
真正使用的欄位，但省略任何 schema 欄位時，最後必須寫 `..`，明確承認該 locale
expression 未使用這些 schema 欄位。完全省略一筆 key 則整筆委派給 fallback。

```rust
use i18n_macro::catalog;

catalog! {
    schema: super::en_us;

    About = "評測器";
    Hello { name } = "你好，{name}";
    Count { count: usize, .. } = "有 {count} 個檔案";
}
```

locale 可選擇另一個使用同一 schema 的 catalog 作為 fallback：

```rust
catalog! {
    schema: super::en_us;
    fallback: super::zh_hant;

    Greeting = "繁中";
}
```

省略 locale 的 `fallback:` 時，會直接以 `schema:` 指向的 catalog 作為 parent。舊的
root/router `fallback` keyword 已拒絕；root catalog 與 `define_i18n!` 都要使用 `default`。

locale 已經列出全部 schema 欄位時不需要 `..`，例如上例的 `Hello { name }`；`{ .. }`
可明確省略所有欄位，完整列出後再寫 redundant 的 final `..` 也合法。

`About = "";` 是明確的空字串翻譯，不會 fallback。`catalog!` 可像上例先 import，
也可直接寫 `i18n_macro::catalog! { ... }`。

## 組合與呼叫

只有 default catalog 時也能建立 router：

```rust
use i18n_macro::define_i18n;

mod en_us;

pub fn current_locale() -> Locale {
    Locale::EnUs
}

define_i18n! {
    locale: pub Locale;
    current_locale: current_locale;
    schema: en_us;
    default EnUs: en_us;
}
```

加入其他語言時，將上例加上 module 與 locale 宣告：

```rust
mod zh_tw;

define_i18n! {
    locale: pub Locale;
    current_locale: current_locale;
    schema: en_us;
    default EnUs: en_us;
    locale ZhTw: zh_tw;
}
```

`define_i18n!` 會產生 `Locale`、`tr!` 與 `tr_for!`：

```rust
let about: &'static str = tr!(About);
let heartbeat = tr!(Heartbeat {});
let hello = tr_for!(Locale::ZhTw, Hello { name: "小明" });
println!("{heartbeat}, {hello}");
```

靜態訊息回傳 `&'static str`，可直接使用；動態訊息回傳延遲格式化、實作 `Display` 的值，
例如 `format!("{hello}")` 或 `println!("{hello}")` 時才渲染。

## Catalog expression 與參數

右側可用字串、Rust `if`、`match` 與 block；block 可建立區域變數，最後選出字串。
格式字串使用 Rust formatting 語法，例如 `{value:.2}`、`{{` 與 `}}`。

```rust
catalog! {
    default;

    Status { disabled: bool } =
        if disabled { "停用" } else { "啟用" };
    Summary { correct: usize, incorrect: usize } = {
        let total = correct + incorrect;
        "正確 {correct}，共 {total} 題"
    };
}
```

- `{ enabled: bool }` 這類明確型別欄位在 expression 中是該 Rust 型別；locale 若明確
  綁定它，值會被複製，因此必須實作 `Copy`。
- `{ name }` 這類未標型別欄位只要求 `Display`，locale 會以借用方式使用；可傳 owned
  value、`&str` 或其他引用。

## 目前限制

- 明確型別欄位必須實作 `Copy`；帶 lifetime 的借用型別請改用未標型別 `Display` 欄位。
- `return`、`break`、`continue` 與 `?` 在 catalog expression 的正式語義仍未決定。
- 外部函式呼叫與其他 macro 的 expression 支援延後；plural/ICU/CLDR 規則也尚未提供。
- `define_i18n!` 目前應放在 crate root；從另一 crate 新增 locale catalog 仍不在此版本範圍。

設計、relay 限制與驗證證據請參考
[Rust-native typed i18n catalog design](../docs/superpowers/specs/2026-07-25-rust-native-i18n-catalog-design.md)
與 [schema relay design](../docs/superpowers/specs/2026-07-26-i18n-schema-relay-design.md)。
