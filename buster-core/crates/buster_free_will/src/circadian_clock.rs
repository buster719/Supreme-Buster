//! Periodic wakeups that let Buster exist across time, not only when invoked.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircadianClock {
    pub wake_interval_minutes: u32,
    pub audit_interval_minutes: u32,
    pub reflection_interval_minutes: u32,
}

impl Default for CircadianClock {
    fn default() -> Self {
        Self {
            wake_interval_minutes: 60,
            audit_interval_minutes: 24 * 60,
            reflection_interval_minutes: 6 * 60,
        }
    }
}
