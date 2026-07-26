use i18n_macro::catalog;

catalog! {
    fallback;

    UnsupportedConfiguredLocale { locale } =
        "不支援 config.yaml 中設定的語言 {locale}；將使用 zh-TW";
    RunningWithoutJudge = "正在運行程式";
    ReuseCompilation = "重複使用編譯檔案";
    CompilingProgress { secs: f64 } = "正在編譯檔案 / {secs:.2}s";
    ExecutionProgress { secs: f64 } =
    "執行中... {secs:.2}s";
    ConfigConflict { path1, path2 } = "配置檔衝突：同時存在 {path1} 和 {path2}";
    NoConfigFile { path } = "找不到配置檔：{path}，考慮用'-n'參數直接執行程式";
    FileNotFound { path } = "檔案不存在：{path}";
    FolderNotFound { path } = "資料夾不存在：{path}";
    UnknownSourceExtension { extension } =
        "未知原始碼副檔名 {extension} ，請選擇 config.yaml 中含有的類型";
    AmbiguousSourceExtension { a, b } = "發現多個可能的副檔名 (.{a} vs .{b})";
    JudgeReportTestCaseHeader = "測資";
    JudgeReportTimeHeader = "用時 (ms)";
    JudgeReportMemoryHeader = "記憶體 (KiB)";
    JudgeReportResultHeader = "結果";
    JudgeSummaryLabel = "總結:";
    JudgeSummary { correct: usize, incorrect: usize, ratio: usize } =
        "正確 {correct} 錯誤 {incorrect} 正確比 {ratio}%";
    DiffAboveOmitted = "... (以上省略)";
    DiffBelowOmitted = "... (以下省略)";
    VerdictACInfo = "答案正確！";
    VerdictSEInfo { error } = "內部錯誤：{error}";
    VerdictOLEInfo = "程式輸出量超過限制！";
    VerdictTLEInfo = "程式執行時間超過限制！";
    VerdictMLEInfo = "程式記憶體使用量超過限制！";
    VerdictWAInfo = "答案比對失敗！";
    HiddenLongInput = "(已隱藏過長內容)";
    Unlimited = "無限制";
    MemoryUsage { used, limit } = "記憶體使用量: {used} KiB / {limit} KiB";
    ExecutionTime { used, limit } = "程式執行耗時: {used} ms / {limit} ms";
    VerdictRE = "運行時錯誤 RE";
    VerdictSE = "系統錯誤 SE";
    VerdictWA = "答案錯誤 WA";
    VerdictNA = "答案不正確 NA";
    VerdictOLE = "輸出超限 OLE";
    VerdictTLE = "超時錯誤 TLE";
    VerdictMLE = "記憶體超限 MLE";
    VerdictAC = "答案正確 AC";
}
