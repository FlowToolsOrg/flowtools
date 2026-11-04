//! Local time conversion with an explicit timezone and DST ambiguity policy.
use crate::protocol::ErrorCode;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LocalTimeChoice {
    Reject,
    Earlier,
    Later,
}
#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveScheduleTime {
    pub local: String,
    pub time_zone: String,
    pub choice: LocalTimeChoice,
}
#[derive(Clone, Debug, Serialize, Deserialize, specta::Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedScheduleTime {
    #[specta(type = specta_typescript::Number)]
    #[schemars(range(min = 0, max = 253402300799999u64))]
    pub due_ms: u64,
    pub utc: String,
    pub zoned: String,
    pub time_zone: String,
}
pub fn resolve_time(input: ResolveScheduleTime) -> Result<ResolvedScheduleTime, ErrorCode> {
    if input.local.len() > 32
        || input.time_zone.is_empty()
        || input.time_zone.len() > 64
        || !input
            .time_zone
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/_+-".contains(&byte))
    {
        return Err(ErrorCode::InputInvalid);
    }
    let local: jiff::civil::DateTime = input.local.parse().map_err(|_| ErrorCode::InputInvalid)?;
    let zone = jiff::tz::TimeZone::get(&input.time_zone).map_err(|_| ErrorCode::InputInvalid)?;
    let ambiguous = zone.to_ambiguous_zoned(local);
    let zoned = match input.choice {
        LocalTimeChoice::Reject => ambiguous.unambiguous(),
        LocalTimeChoice::Earlier => ambiguous.earlier(),
        LocalTimeChoice::Later => ambiguous.later(),
    }
    .map_err(|_| ErrorCode::InputInvalid)?;
    let timestamp = zoned.timestamp();
    let due_ms = u64::try_from(timestamp.as_millisecond()).map_err(|_| ErrorCode::InputInvalid)?;
    if due_ms > super::MAX_TIMESTAMP_MS {
        return Err(ErrorCode::InputInvalid);
    }
    Ok(ResolvedScheduleTime {
        due_ms,
        utc: timestamp.to_string(),
        zoned: zoned.to_string(),
        time_zone: input.time_zone,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn resolve(
        local: &str,
        zone: &str,
        choice: LocalTimeChoice,
    ) -> Result<ResolvedScheduleTime, ErrorCode> {
        resolve_time(ResolveScheduleTime {
            local: local.into(),
            time_zone: zone.into(),
            choice,
        })
    }
    #[test]
    fn conversion_uses_the_explicit_zone() {
        assert_eq!(
            resolve("2026-10-27T09:00", "Asia/Shanghai", LocalTimeChoice::Reject)
                .unwrap()
                .utc,
            "2026-10-27T01:00:00Z"
        );
        assert_eq!(
            resolve("2026-10-27T09:00", "UTC", LocalTimeChoice::Reject)
                .unwrap()
                .utc,
            "2026-10-27T09:00:00Z"
        );
    }
    #[test]
    fn repeated_dst_hour_requires_an_explicit_choice() {
        let local = "2026-11-01T01:30";
        assert_eq!(
            resolve(local, "America/New_York", LocalTimeChoice::Reject).err(),
            Some(ErrorCode::InputInvalid)
        );
        let earlier = resolve(local, "America/New_York", LocalTimeChoice::Earlier).unwrap();
        let later = resolve(local, "America/New_York", LocalTimeChoice::Later).unwrap();
        assert_eq!(later.due_ms - earlier.due_ms, 3_600_000);
        assert!(earlier.zoned.contains("-04:00"));
        assert!(later.zoned.contains("-05:00"));
    }
    #[test]
    fn nonexistent_dst_hour_is_not_silently_normalized() {
        assert_eq!(
            resolve(
                "2026-03-08T02:30",
                "America/New_York",
                LocalTimeChoice::Reject
            )
            .err(),
            Some(ErrorCode::InputInvalid)
        );
        assert!(resolve(
            "2026-03-08T02:30",
            "America/New_York",
            LocalTimeChoice::Later
        )
        .unwrap()
        .zoned
        .contains("03:30"));
    }
    #[test]
    fn invalid_zones_paths_and_instants_refuse() {
        for zone in ["../zone", "C:/zone", "Unknown/Nowhere", ""] {
            assert!(resolve("2026-10-27T09:00", zone, LocalTimeChoice::Reject).is_err());
        }
        for local in ["not-a-time", "1960-01-01T00:00", "2026-10-27T09:00Z"] {
            assert!(resolve(local, "UTC", LocalTimeChoice::Reject).is_err());
        }
    }
}
