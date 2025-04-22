use crate::models::common::TimestampNs;

/// Returns the current Internet Computer time as nanoseconds since epoch.
pub fn get_current_time_ns() -> TimestampNs {
    ic_cdk::api::time()
} 