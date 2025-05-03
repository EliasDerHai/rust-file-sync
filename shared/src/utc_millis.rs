use chrono::{DateTime, Local};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Display, Formatter};
use std::time::{SystemTime, UNIX_EPOCH};

/// simple Value object that serializes to utc milliseconds and can be printed as readable datetime string
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct UtcMillis {
    millis: u64,
}

impl Display for UtcMillis {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let d = DateTime::from(self.clone());
        write!(f, "{}", d)
    }
}

/// instantiation
impl UtcMillis {
    pub fn now() -> Self {
        UtcMillis::from(chrono::Utc::now().timestamp_millis() as u64)
    }
}

impl From<UtcMillis> for DateTime<Local> {
    fn from(value: UtcMillis) -> Self {
        let utc_secs = value.millis as i64 / 1_000;
        let utc_remainder_nanos = (utc_secs % 1_000) * 1_000_000;
        DateTime::from_timestamp(utc_secs, utc_remainder_nanos as u32)
            .unwrap()
            .with_timezone(&Local)
    }
}

impl From<u64> for UtcMillis {
    fn from(millis: u64) -> Self {
        UtcMillis { millis }
    }
}

impl From<SystemTime> for UtcMillis {
    fn from(time: SystemTime) -> Self {
        let utc = time.duration_since(UNIX_EPOCH);
        UtcMillis {
            millis: utc.unwrap().as_millis() as u64,
        }
    }
}

/// serde
impl<'de> Deserialize<'de> for UtcMillis {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(UtcMillis { millis })
    }
}

impl Serialize for UtcMillis {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(self.millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_display_millis() {
        let millis = UtcMillis { millis: 0 };
        let display_value = format!("{}", millis);
        // this test passes but is dependent on my timezone (Germany/Berlin)
        // assert_eq!("1970-01-01 01:00:00 +01:00", format!("{}", millis));
        assert!(display_value.starts_with("1970-01-01"));
    }

    #[test]
    fn should_compare() {
        let a = UtcMillis::from(500);
        let b = UtcMillis::from(900);
        assert!(a < b);
    }

    #[test]
    fn should_be_equal() {
        let a = UtcMillis::from(500);
        let b = UtcMillis::from(500);
        assert_eq!(a, b);
    }

    #[test]
    fn should_sort() {
        let mut elements = [UtcMillis::from(5), UtcMillis::from(50), UtcMillis::from(1)];
        elements.sort();

        assert_eq!(
            vec![1, 5, 50],
            elements
                .iter()
                .map(|elem| elem.millis)
                .collect::<Vec<u64>>()
        )
    }

    /// serde tests
    #[test]
    fn should_serialization() {
        let millis = UtcMillis {
            millis: 1711747200000,
        };
        let serialized = serde_json::to_string(&millis).unwrap();
        assert_eq!(serialized, "1711747200000");
    }

    #[test]
    fn should_deserialize() {
        let json_data = "1711747200000";
        let deserialized: UtcMillis = serde_json::from_str(json_data).unwrap();
        assert_eq!(deserialized.millis, 1711747200000);
    }

    #[test]
    fn should_serialize_deserialize_roundtrip() {
        let millis = UtcMillis { millis: 1234567890 };
        let serialized = serde_json::to_string(&millis).unwrap();
        let deserialized: UtcMillis = serde_json::from_str(&serialized).unwrap();
        assert_eq!(millis, deserialized);
    }

    #[test]
    fn should_deserialize_invalid_data() {
        let json_data = "\"invalid_string\"";
        let result: Result<UtcMillis, _> = serde_json::from_str(json_data);
        assert!(result.is_err());
    }

    #[test]
    fn should_deserialize_negative_value() {
        let json_data = "-1000";
        let result: Result<UtcMillis, _> = serde_json::from_str(json_data);
        assert!(result.is_err());
    }
}
