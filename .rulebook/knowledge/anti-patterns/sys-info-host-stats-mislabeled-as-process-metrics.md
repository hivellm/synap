# sys_info host stats mislabeled as process metrics

**Category**: observability
**Tags**: metrics, prometheus, cpu, sysinfo, issue-196

## Description

A gauge named synap_process_cpu_usage_percent was set from sys_info::loadavg() (whole-machine load average) and synap_process_memory_bytes from sys_info::mem_info() (whole-machine memory). On a busy shared host an IDLE broker then reports 100-180% CPU and tens of GB used, which reads as a broker busy-loop / leak but is purely a measurement artifact. sys-info (the sys_info crate) only exposes host-wide stats; it has no per-process API. For per-process CPU/RSS use the sysinfo crate (Process::cpu_usage/memory) with a sampler kept across scrapes. Keep host stats but name them host_*, not process_*.

## Example

// WRONG: host load average labelled as process CPU
PROCESS_CPU_USAGE.with_label_values(&["1min"]).set((load.one*100.0) as i64);
// RIGHT: real per-process sample, host stats under host_*
sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), true, ProcessRefreshKind::everything());
set_process_metrics(proc_.memory(), proc_.virtual_memory(), proc_.cpu_usage() as f64);

## When to Use

When adding or reviewing process-level resource metrics for any Synap/Rust service.
