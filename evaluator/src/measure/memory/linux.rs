use cgroups_rs::cgroup_builder::CgroupBuilder;
use cgroups_rs::Cgroup;
use cgroups_rs::hierarchies;
use cgroups_rs::memory::{MemController, MemoryStat};
use nix::unistd::Pid;
use rand::Rng;
use std::thread;
use std::time::Duration;

const CHECK_PROCESS_INTERVAL: Duration = Duration::from_millis(50);

/// 為 Linux 創建一個記憶體監控器。
///
/// 此函數使用 cgroups 來監控一個處理程序及其所有子處理程序的峰值記憶體使用量。
///
/// # Arguments
///
/// * `pid` - 要監控的目標處理程序的 PID。
///
/// # Returns
///
/// 一個 `Box<dyn FnOnce() -> Option<usize>>`。調用此閉包將會阻塞，
/// 直到所有受監控的處理程序終止，然後返回它們總和的峰值記憶體使用量（以 KB 為單位）。
/// 如果建立 cgroup 失敗，則閉包將立即返回 `None`。
///
/// # 注意
///
/// 執行此程式碼的用戶需要有權限管理 cgroups。通常這意味著以 root 身份運行，
/// 或者被授予了特定權限。
pub fn create_memory_monitor(pid: u32) -> Box<dyn FnOnce() -> Option<usize>> {
    // 嘗試為目標 PID 創建並應用一個 cgroup。
    let cgroup_job = match CgroupJob::new(pid) {
        Ok(job) => job,
        Err(e) => {
            log::warn!("無法創建 cgroup 來監控記憶體: {}", e);
            // 如果失敗，返回一個總是回傳 None 的閉包。
            return Box::new(|| None);
        }
    };

    // 啟動一個新執行緒來監控 cgroup。
    let monitor_thread = std::thread::spawn(move || {
        monitor_cgroup_memory_usage(cgroup_job)
    });

    // 返回一個閉包，它會等待監控執行緒完成並返回結果。
    Box::new(|| monitor_thread.join().unwrap())
}

/// 監控 cgroup 的記憶體使用量，直到其中所有處理程序都終止。
fn monitor_cgroup_memory_usage(job: CgroupJob) -> Option<usize> {
    loop {
        // 檢查 cgroup 中是否還有任何活躍的處理程序。
        let tasks = match job.cgroup.tasks() {
            Ok(tasks) => tasks,
            Err(e) => {
                log::warn!("無法查詢 cgroup 中的任務: {}", e);
                // 在出錯時，我們無法繼續，峰值記憶體可能不準確。
                // 讀取最後已知的峰值。
                break;
            }
        };

        if tasks.is_empty() {
            // 所有處理程序都已結束，跳出循環。
            break;
        }

        thread::sleep(CHECK_PROCESS_INTERVAL);
    }

    // 所有處理程序都結束後，從 cgroup 的 memory controller 讀取峰值記憶體使用量。
    let mem_controller: &MemController = job.cgroup.memory();
    match mem_controller.stat().peak {
        // cgroup v2 提供 'memory.peak'，這是最理想的。
        Some(peak_bytes) => {
            let peak_kb = (peak_bytes / 1024) as usize;
            log::info!("Cgroup 記錄的峰值記憶體使用量: {} KB", peak_kb);
            Some(peak_kb)
        }
        // 如果 'memory.peak' 不可用（例如在 cgroup v1 上），可以回退到 max_usage。
        None => {
            let stat: &MemoryStat = mem_controller.stat();
            let peak_bytes = stat.max_usage_in_bytes;
            if peak_bytes > 0 {
                let peak_kb = (peak_bytes / 1024) as usize;
                log::info!("Cgroup 記錄的最大記憶體使用量: {} KB", peak_kb);
                Some(peak_kb)
            } else {
                log::warn!("無法從 cgroup 獲取峰值記憶體使用量。");
                None
            }
        }
    }
    // `job` 在此處被 drop，其 Drop impl 會自動清理 cgroup。
}


/// 一個包裝 Cgroup 的結構體，用於確保在 Drop 時自動清理。
/// 這模擬了 Windows Job Object 的生命週期管理行為。
struct CgroupJob {
    cgroup: Cgroup,
}

impl CgroupJob {
    /// 創建一個新的 Cgroup，並將指定的 PID 加入其中。
    pub fn new(pid: u32) -> Result<Self, Box<dyn std::error::Error>> {
        // 使用 PID 和一個隨機數來創建一個唯一的 cgroup 名稱，以避免衝突。
        let random_suffix = rand::thread_rng().gen_range(10000..99999);
        let cgroup_name = format!("memory-monitor-{}-{}", pid, random_suffix);

        log::info!("為 PID {} 創建 cgroup: {}", pid, cgroup_name);

        // 獲取 cgroup 層級結構。`hierarchies::auto()` 會自動檢測 v1 或 v2。
        let hier = hierarchies::auto();
        
        // 創建 cgroup 並啟用 memory controller。
        let cgroup = CgroupBuilder::new(&cgroup_name)
            .memory()
            .done()
            .build(hier)?;

        // 將目標處理程序加入到 cgroup 中。
        // `cgroups-rs` 需要 `nix::unistd::Pid` 類型。
        let target_pid = Pid::from_raw(pid as i32);
        cgroup.add_task(target_pid)?;

        Ok(Self { cgroup })
    }
}

impl Drop for CgroupJob {
    /// 當 CgroupJob 離開作用域時，自動刪除 cgroup。
    fn drop(&mut self) {
        if let Err(e) = self.cgroup.delete() {
            log::error!("刪除 cgroup 失敗: {}", e);
        } else {
            log::info!("成功刪除 cgroup。");
        }
    }
}

// // ---- 測試用的範例 main 函數 ----
// fn main() {
//     // 初始化日誌記錄器，以便看到 log::info/warn/error 的輸出。
//     env_logger::init();

//     println!("啟動一個子處理程序來進行監控...");

//     // 啟動一個子處理程序，它會分配一些記憶體然後退出。
//     let mut child = std::process::Command::new("bash")
//         .arg("-c")
//         .arg("echo '開始分配記憶體...'; sleep 1; head -c 50M /dev/zero > /dev/null; echo '分配完成，即將退出。'; sleep 1")
//         .spawn()
//         .expect("無法啟動子處理程序");

//     let pid = child.id();
//     println!("已啟動子處理程序，PID: {}", pid);

//     // 創建記憶體監控器。
//     let get_peak_memory = create_memory_monitor(pid);

//     // 等待子處理程序結束。
//     let status = child.wait().expect("等待子處理程序失敗");
//     println!("子處理程序已結束，狀態: {}", status);

//     // 現在調用閉包來獲取峰值記憶體。
//     // 這將會等待監控執行緒完成其工作。
//     if let Some(peak_kb) = get_peak_memory() {
//         println!("偵測到的峰值記憶體使用量: {} KB", peak_kb);
//         // 預期值應略高於 50MB (51200 KB)，因為還包括 bash 本身和其他開銷。
//         assert!(peak_kb > 50_000);
//     } else {
//         println!("無法獲取峰值記憶體使用量。請檢查權限或日誌輸出。");
//     }
// }