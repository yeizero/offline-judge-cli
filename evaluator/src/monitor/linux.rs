use cgroups_rs::Cgroup;
use cgroups_rs::CgroupPid;
use cgroups_rs::cgroup_builder::CgroupBuilder;
use cgroups_rs::hierarchies;
use cgroups_rs::memory::MemController;
use rand::Rng;
use std::thread;
use std::time::Duration;

const CHECK_PROCESS_INTERVAL: Duration = Duration::from_millis(5);

pub fn create_memory_monitor(pid: u32) -> Box<dyn FnOnce() -> Option<usize>> {
    let cgroup_job = match CgroupJob::new(pid) {
        Ok(job) => job,
        Err(e) => {
            log::warn!("無法創建 cgroup 來監控記憶體: {e}");
            return Box::new(|| None);
        }
    };
    let monitor_thread = std::thread::spawn(move || monitor_cgroup_memory_usage(cgroup_job));
    Box::new(|| monitor_thread.join().unwrap())
}

fn monitor_cgroup_memory_usage(job: CgroupJob) -> Option<usize> {
    loop {
        let tasks = job.cgroup.tasks();
        if tasks.is_empty() {
            break;
        }
        thread::sleep(CHECK_PROCESS_INTERVAL);
    }
    let mem_controller: &MemController = job.cgroup.controller_of().unwrap();
    (mem_controller.memory_stat().max_usage_in_bytes / 1024)
        .try_into()
        .ok()
}

struct CgroupJob {
    cgroup: Cgroup,
}

impl CgroupJob {
    pub fn new(pid: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let random_suffix = rand::rng().random_range(10000..99999);
        let cgroup_name = format!("offline-judge-{pid}-{random_suffix}");
        let hier = hierarchies::auto();
        let cgroup = CgroupBuilder::new(&cgroup_name)
            .memory()
            .done()
            .build(hier)?;
        cgroup
            .add_task_by_tgid(CgroupPid::from(pid as u64))
            .inspect_err(|e| log::warn!("{e}"))?;
        Ok(Self { cgroup })
    }
}

impl Drop for CgroupJob {
    fn drop(&mut self) {
        if let Err(e) = self.cgroup.delete() {
            log::warn!("刪除 cgroup '{}' 失敗: {}", self.cgroup.path(), e);
        }
    }
}
