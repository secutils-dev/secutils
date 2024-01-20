/// Defines the reason why a script was terminated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScriptTerminationReason {
    /// The script was terminated because it hit the memory limit.
    MemoryLimit = 0,
    /// The script was terminated because it hit the time limit.
    TimeLimit = 1,
    /// The script was terminated for an unknown reason.
    Unknown = 2,
}

impl From<usize> for ScriptTerminationReason {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::MemoryLimit,
            1 => Self::TimeLimit,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ScriptTerminationReason;

    #[test]
    fn conversion() {
        assert_eq!(
            ScriptTerminationReason::MemoryLimit,
            ScriptTerminationReason::from(0)
        );
        assert_eq!(
            ScriptTerminationReason::TimeLimit,
            ScriptTerminationReason::from(1)
        );
        assert_eq!(
            ScriptTerminationReason::Unknown,
            ScriptTerminationReason::from(2)
        );
        assert_eq!(
            ScriptTerminationReason::Unknown,
            ScriptTerminationReason::from(100500)
        );
    }
}
