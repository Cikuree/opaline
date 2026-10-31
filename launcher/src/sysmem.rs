//! RAM physique totale (en Mo), pour borner la mémoire allouée au jeu.

/// `MemTotal` de `/proc/meminfo` (en kB) converti en Mo.
#[allow(dead_code)] // Linux seulement
pub fn parse_meminfo(text: &str) -> Option<u64> {
    let kb: u64 = text.lines().find_map(|l| l.strip_prefix("MemTotal:"))?.trim().trim_end_matches("kB").trim().parse().ok()?;
    Some(kb / 1024)
}

#[cfg(windows)]
pub fn total_ram_mb() -> Option<u64> {
    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page_file: u64,
        avail_page_file: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }
    unsafe extern "system" {
        fn GlobalMemoryStatusEx(buf: *mut MemoryStatusEx) -> i32;
    }
    let mut m = MemoryStatusEx { length: size_of::<MemoryStatusEx>() as u32, memory_load: 0, total_phys: 0, avail_phys: 0, total_page_file: 0, avail_page_file: 0, total_virtual: 0, avail_virtual: 0, avail_extended_virtual: 0 };
    // SAFETY : `m` est une structure MEMORYSTATUSEX valide dont `length` est renseigné.
    (unsafe { GlobalMemoryStatusEx(&mut m) } != 0).then_some(m.total_phys / (1024 * 1024))
}

#[cfg(target_os = "linux")]
pub fn total_ram_mb() -> Option<u64> {
    parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn total_ram_mb() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meminfo() {
        assert_eq!(parse_meminfo("MemTotal:       16384000 kB\nMemFree: 1 kB\n"), Some(16000));
        assert_eq!(parse_meminfo("x"), None);
    }

    #[test]
    fn reads_real_ram_on_supported_platforms() {
        if cfg!(any(windows, target_os = "linux")) {
            assert!(total_ram_mb().unwrap() > 256);
        }
    }
}
