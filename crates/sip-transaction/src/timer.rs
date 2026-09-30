use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct SipTimers {
    pub t1: Duration, // 500ms default
    pub t2: Duration, // 4000ms default
    pub t4: Duration, // 5000ms default
}

impl Default for SipTimers {
    fn default() -> Self {
        Self {
            t1: Duration::from_millis(500),
            t2: Duration::from_millis(4000),
            t4: Duration::from_millis(5000),
        }
    }
}

impl SipTimers {
    pub fn fast_test() -> Self {
        Self {
            t1: Duration::from_millis(50),
            t2: Duration::from_millis(200),
            t4: Duration::from_millis(200),
        }
    }

    /// Timer B / F / H / J: 64 * T1
    pub fn timer_b(&self) -> Duration {
        self.t1 * 64
    }

    /// Timer D / K: T4 for UDP
    pub fn timer_k(&self) -> Duration {
        self.t4
    }
}
