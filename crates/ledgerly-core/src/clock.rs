use time::OffsetDateTime;

/// The source of "now". Passed in everywhere (rule C5) so tests never read the wall clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clock {
    /// The system clock, in UTC.
    System,
    /// A fixed instant, for tests.
    Fixed(OffsetDateTime),
}

impl Clock {
    /// The current instant according to this clock.
    pub fn now(self) -> OffsetDateTime {
        match self {
            Clock::System => OffsetDateTime::now_utc(),
            Clock::Fixed(at) => at,
        }
    }
}
