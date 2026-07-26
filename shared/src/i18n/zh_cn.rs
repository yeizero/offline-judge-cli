use i18n_macro::catalog;

catalog! {
    schema: super::en_us;

    RunningWithoutJudge = "正在运行程序";
    ReuseCompilation = "重复使用编译文件";
    CompilingProgress { secs: f64 } = "正在编译文件 / {secs:.2}s";
    ExecutionProgress { secs: f64 } = "运行中... {secs:.2}s";
    ConfigConflict { path1, path2 } = "配置文件冲突：同时存在 {path1} 和 {path2}";
    NoConfigFile { path } = "找不到配置文件：{path}，考虑用'-n'参数直接运行程序";
    FileNotFound { path } = "文件不存在：{path}";
    FolderNotFound { path } = "文件夹不存在：{path}";
    UnknownSourceExtension { extension } =
        "未知源代码扩展名 {extension}，请选择 config.yaml 中包含的类型";
    AmbiguousSourceExtension { a, b } = "发现多个可能的扩展名 (.{a} vs .{b})";
    JudgeReportTestCaseHeader = "测试数据";
    JudgeReportTimeHeader = "用时 (ms)";
    JudgeReportMemoryHeader = "内存 (KiB)";
    JudgeReportResultHeader = "结果";
    JudgeSummaryLabel = "总结:";
    JudgeSummary { correct: usize, incorrect: usize, ratio: usize } =
        "正确 {correct} 错误 {incorrect} 正确率 {ratio}%";
    DiffAboveOmitted = "... (以上省略)";
    DiffBelowOmitted = "... (以下省略)";
    VerdictACInfo = "答案正确！";
    VerdictSEInfo { error } = "内部错误：{error}";
    VerdictOLEInfo = "程序输出量超过限制！";
    VerdictTLEInfo = "程序运行时间超过限制！";
    VerdictMLEInfo = "程序内存使用量超过限制！";
    VerdictWAInfo = "答案比对失败！";
    HiddenLongInput = "(已隐藏过长内容)";
    Unlimited = "无限制";
    MemoryUsage { used, limit } = "内存使用量: {used} KiB / {limit} KiB";
    ExecutionTime { used, limit } = "程序运行耗时: {used} ms / {limit} ms";
    VerdictRE = "运行时错误 RE";
    VerdictSE = "系统错误 SE";
    VerdictWA = "答案错误 WA";
    VerdictNA = "答案不正确 NA";
    VerdictOLE = "输出超限 OLE";
    VerdictTLE = "超时错误 TLE";
    VerdictMLE = "内存超限 MLE";
    VerdictAC = "答案正确 AC";
}
