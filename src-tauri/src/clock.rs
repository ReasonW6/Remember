use std::sync::OnceLock;
use std::time::Instant;

/// Milliseconds since the first call, measured on a monotonic clock so
/// system clock adjustments (NTP, manual changes) cannot move recorded
/// timestamps backwards.
pub fn now_ms() -> u64 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_ms_is_monotonic() {
        let first = now_ms();
        let second = now_ms();
        assert!(second >= first);
    }
}
