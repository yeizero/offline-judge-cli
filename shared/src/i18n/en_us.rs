use i18n_macro::catalog;

catalog! {
    default;

    RunningWithoutJudge = "Running program";
    ReuseCompilation = "Reusing compiled file";
    CompilingProgress { secs: f64 } = "Compiling file / {secs:.2}s";
    ExecutionProgress { secs: f64 } = "Running... {secs:.2}s";
    ConfigConflict { path1, path2 } =
        "Configuration file conflict: both {path1} and {path2} exist";
    NoConfigFile { path } =
        "Configuration file not found: {path}; consider using '-n' to run the program directly";
    FileNotFound { path } = "File not found: {path}";
    FolderNotFound { path } = "Folder not found: {path}";
    UnknownSourceExtension { extension } =
        "Unknown source extension {extension}; choose a type defined in config.yaml";
    AmbiguousSourceExtension { a, b } =
        "Multiple possible extensions found (.{a} vs .{b})";
    JudgeReportTestCaseHeader = "Test";
    JudgeReportTimeHeader = "Time (ms)";
    JudgeReportMemoryHeader = "Memory (KiB)";
    JudgeReportResultHeader = "Result";
    JudgeSummaryLabel = "Summary:";
    JudgeSummary { correct: usize, incorrect: usize, ratio: usize } =
        "{correct} correct; {incorrect} incorrect; {ratio}% accuracy";
    DiffAboveOmitted = "... (omitted above)";
    DiffBelowOmitted = "... (omitted below)";
    VerdictACInfo = "Correct answer!";
    VerdictSEInfo { error } = "Internal error: {error}";
    VerdictOLEInfo = "Program output exceeded the limit!";
    VerdictTLEInfo = "Program execution time exceeded the limit!";
    VerdictMLEInfo = "Program memory usage exceeded the limit!";
    VerdictWAInfo = "Answer comparison failed!";
    HiddenLongInput = "(overlong content hidden)";
    Unlimited = "Unlimited";
    MemoryUsage { used, limit } = "Memory usage: {used} KiB / {limit} KiB";
    ExecutionTime { used, limit } = "Execution time: {used} ms / {limit} ms";
    VerdictRE = "Runtime Error";
    VerdictSE = "System Error";
    VerdictWA = "Wrong Answer";
    VerdictNA = "Not Accepted";
    VerdictOLE = "Output Limit Exceeded";
    VerdictTLE = "Time Limit Exceeded";
    VerdictMLE = "Memory Limit Exceeded";
    VerdictAC = "Accepted";
}
