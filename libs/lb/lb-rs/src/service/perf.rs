//! Process RSS and CPU counters. Wall time is the parent span.

use std::time::Duration;

/// Emits `counter.rss_bytes` / `counter.cpu_us` on drop.
pub struct Sample {
    cpu: Option<Duration>,
}

impl Sample {
    pub fn new() -> Option<Self> {
        if !tracing::enabled!(tracing::Level::TRACE) {
            return None;
        }
        Some(Self { cpu: process_cpu() })
    }
}

impl Drop for Sample {
    fn drop(&mut self) {
        let cpu_us = process_cpu()
            .and_then(|end| self.cpu.map(|start| end.saturating_sub(start)))
            .map(|d| d.as_micros() as u64);
        let rss = current_rss_bytes();
        match (cpu_us, rss) {
            (Some(cpu_us), Some(rss_bytes)) => {
                tracing::trace!(counter.rss_bytes = rss_bytes, counter.cpu_us = cpu_us);
            }
            (Some(cpu_us), None) => {
                tracing::trace!(counter.cpu_us = cpu_us);
            }
            (None, Some(rss_bytes)) => {
                tracing::trace!(counter.rss_bytes = rss_bytes);
            }
            (None, None) => {}
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn process_cpu() -> Option<Duration> {
    None
}

#[cfg(not(any(unix, windows)))]
fn current_rss_bytes() -> Option<u64> {
    None
}

#[cfg(unix)]
fn process_cpu() -> Option<Duration> {
    // SAFETY: `usg` is zeroed; getrusage writes it on success.
    unsafe {
        let mut usg = std::mem::zeroed();
        if libc::getrusage(libc::RUSAGE_SELF, &mut usg) != 0 {
            return None;
        }
        Some(timeval(usg.ru_utime) + timeval(usg.ru_stime))
    }
}

#[cfg(unix)]
fn timeval(tv: libc::timeval) -> Duration {
    Duration::new(tv.tv_sec as u64, (tv.tv_usec as u32).saturating_mul(1_000))
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn current_rss_bytes() -> Option<u64> {
    // SAFETY: `info`/`count` are valid out-params for MACH_TASK_BASIC_INFO.
    unsafe {
        let mut info: libc::mach_task_basic_info = std::mem::zeroed();
        let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
        let kr = libc::task_info(
            libc::mach_task_self(),
            libc::MACH_TASK_BASIC_INFO,
            &mut info as *mut _ as libc::task_info_t,
            &mut count,
        );
        if kr == libc::KERN_SUCCESS { Some(info.resident_size) } else { None }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn current_rss_bytes() -> Option<u64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    // SAFETY: sysconf(_SC_PAGESIZE) takes no pointer args.
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if page <= 0 {
        return None;
    }
    Some(pages.saturating_mul(page as u64))
}

#[cfg(windows)]
fn process_cpu() -> Option<Duration> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    // SAFETY: FILETIME out-params are zeroed; GetProcessTimes writes them on success.
    unsafe {
        let mut created = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let mut exited = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let mut kernel = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let mut user = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        if GetProcessTimes(GetCurrentProcess(), &mut created, &mut exited, &mut kernel, &mut user)
            == 0
        {
            return None;
        }
        Some(filetime(user) + filetime(kernel))
    }
}

#[cfg(windows)]
fn filetime(ft: windows_sys::Win32::Foundation::FILETIME) -> Duration {
    let ticks = ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
    // FILETIME is 100 ns intervals.
    Duration::from_nanos(ticks.saturating_mul(100))
}

#[cfg(windows)]
fn current_rss_bytes() -> Option<u64> {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    // SAFETY: pmc.cb is the struct size GetProcessMemoryInfo requires.
    unsafe {
        let mut pmc: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut pmc, pmc.cb) == 0 {
            return None;
        }
        Some(pmc.WorkingSetSize as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_do_not_panic() {
        let _ = process_cpu();
        let rss = current_rss_bytes();
        assert!(rss.unwrap_or(0) > 0, "expected a current RSS reading, got {rss:?}");
        let _s = Sample::new();
    }
}
