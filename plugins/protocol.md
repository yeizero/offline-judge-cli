## Host ↔ Plugin 通訊協定

本文件定義了**主控程式 (Host)** 與**插件 (Plugin)** 之間基於標準輸入/輸出 (`stdin`/`stdout`) 的通訊規則。協定的核心是由 Plugin 主動發送指令，Host 接收並在必要時回應。

### ■ 指令總覽 (Quick Reference)

- /info: 發送一般資訊 (單向)。
- /warn: 發送警告訊息 (單向)。
- /error: 發送錯誤訊息 (單向)。
- /confirm: 請求「是/否」確認 (雙向)。
- /ask: 請求文字輸入 (雙向)。
- /config: 查詢插件設定 (雙向)。
* /getdata: 查詢當前資料 (雙向)。
- /data: 提交資料，可多次呼叫以合併資料 (單向)。

### ■ 核心原則

*   **通訊媒介**:
    *   **Plugin → Host**: Plugin 將指令寫入其 `stdout`。
    *   **Host → Plugin**: Host 將回應寫入 Plugin 的 `stdin`。

*   **訊息單位**:
    *   通訊以**行**為單位，每一行都是一條完整的訊息。

*   **指令格式 (由 Plugin 發送)**:
    *   格式: `/<command_name> <JSON_payload>`
    *   `/<command_name>`: 以斜線 `/` 開頭，大小寫不敏感。
    *   `<JSON_payload>`: 一個單行、合法、UTF-8 編碼的 JSON 物件或字串。
    *   **注意**: 指令名稱和 JSON 之間**必須**有一個空格。

---

### ■ 指令詳解

#### 1. 日誌與狀態指令 (單向，沒有回應)

**`/info`**
*   **用途**: 發送一般資訊。
*   **Payload**: JSON 字串。
*   **範例**: `/info "Compilation started."`

**`/warn`**
*   **用途**: 發送警告訊息。
*   **Payload**: JSON 字串。
*   **範例**: `/warn "Cache not found, performance may be slower."`

**`/error`**
*   **用途**: 發送錯誤訊息。
*   **Payload**: JSON 字串。
*   **範例**: `/error "Segmentation fault detected."`

---

#### 2. 互動式指令 (雙向，有 Host 回應)

**`/confirm`**
*   **用途**: 提出「是/否」問題，並等待 Host 回應。
*   **Payload**: 包含 `message` 欄位的 JSON 物件。
*   **範例**: `/confirm {"message": "File already exists. Overwrite?"}`
*   **Host 回應**: 一行數字， `1` (同意) 或 `0` (不同意)。

**`/ask`**
*   **用途**: 提出問題，並等待 Host 輸入任意字串。
*   **Payload**: 包含 `message` 欄位的 JSON 物件。
*   **範例**: `/ask {"message": "Please enter your API key:"}`
*   **Host 回應**: 任意字串，以換行符 `\n` 結束。

**`/config`**
*   **用途**: 查詢 Plugin 自身的設定值。 (位於 `config.yaml > generator.plugins.\[plugin\].config`)
*   **Payload**: 無。
*   **範例**: `/config`
*   **Host 回應**: 一個 JSON 物件，格式如下：
    *   成功時: `{"status": "success", "data": { ... }}`
    *   失敗時: `{"status": "error", "error": "Error message"}`

**`/getdata`**
*   **用途**: 查詢當前資料。
*   **Payload**: 無。
*   **範例**: `/getdata`
*   **Host 回應**: 一個 JSON 物件，其 `data` 欄位包含當前的資料，格式與 `/config` 類似：
    *   `{"limit": ..., "cases": [...], "meta": {...}}`

---

#### 3. 結果提交指令 (單向，沒有回應)

**`/data`**
*   **用途**: 向 Host 提交一部分或完整的資料，Host 會將收到的資料進行合併。
*   **Payload**: 包含測試案例與全域設定的 JSON 物件。
*   **範例 (多次呼叫)**:
    1.  `/data {"limit": {"memory":5242880, "time":1000}}`
    2.  `/data {"cases":[{"input":"1 1","answer":"2"}]}`
    3.  `/data {"cases":[{"input":"50 50","answer":"100"}]}`
*   **JSON 結構詳解**:
    *   `limit` (object, 可選): 包含全域資源限制。
        *   `memory` (number, 可選): 全局記憶體限制 (單位: KB)。
        *   `time` (number, 可選): 全局時間限制 (單位: ms)。
    *   `cases` (array, 可選): 測試案例列表。
        *   每個測資都是一個物件，包含：
            *   `input` (string): 測試輸入。
            *   `answer` (string): 預期輸出。
    *   `meta` (object, 可選): 額外資訊。