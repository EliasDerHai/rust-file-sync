#![allow(dead_code)]
use std::{collections::HashMap, fmt::Debug, hash::Hash};

pub struct Record<K, V>
where
    K: Hash + Eq,
{
    map: HashMap<K, V>,
    vec: Vec<K>,
}

impl<K, V> Record<K, V>
where
    K: Debug + Hash + Eq + Clone,
{
    pub fn new(it: impl Iterator<Item = K>, f: impl Fn(&K) -> V) -> Self {
        let vec = it.collect::<Vec<K>>();
        let map = vec
            .iter()
            .map(|k| {
                let v = f(k);
                (k.clone(), v)
            })
            .collect();
        Self { map, vec }
    }

    pub fn get(&self, key: &K) -> &V {
        if let Some(v) = self.map.get(key) {
            v
        } else {
            panic!("key '{:?}' not found", key)
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> + '_ {
        self.into_iter()
    }
}

// iter
impl<'a, K, V> IntoIterator for &'a Record<K, V>
where
    K: Clone + Eq + Hash + Debug,
{
    type Item = (&'a K, &'a V);
    type IntoIter = std::vec::IntoIter<(&'a K, &'a V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec
            .iter()
            .map(|k| (k, self.map.get(k).unwrap()))
            .collect::<Vec<(&K, &V)>>()
            .into_iter()
    }
}

impl<K, V> IntoIterator for Record<K, V>
where
    K: Clone + Eq + Hash + Debug,
{
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;

    fn into_iter(mut self) -> Self::IntoIter {
        self.vec
            .into_iter()
            .map(|k| {
                let v = self.map.remove(&k).unwrap();
                (k, v)
            })
            .collect::<Vec<(K, V)>>()
            .into_iter()
    }
}

// clone
impl<K, V> Clone for Record<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
            vec: self.vec.clone(),
        }
    }
}

// serde
impl<K, V> serde::Serialize for Record<K, V>
where
    K: serde::Serialize + Hash + Eq + Clone,
    V: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.map.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use strum::IntoEnumIterator;
    use strum_macros::EnumIter;

    #[derive(Debug, Clone, Hash, PartialEq, Eq, EnumIter, Serialize)]
    enum SomeEnum {
        A,
        B,
        C,
    }

    fn get_test_record() -> Record<SomeEnum, String> {
        Record::new(SomeEnum::iter(), |e| format!("{:?}", e))
    }

    fn get_expected_collected_vec() -> Vec<(SomeEnum, String)> {
        vec![(SomeEnum::A, "A"), (SomeEnum::B, "B"), (SomeEnum::C, "C")]
            .into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect()
    }

    #[test]
    fn test_record_new_and_get() {
        let r = get_test_record();
        assert_eq!(r.get(&SomeEnum::A), "A");
        assert_eq!(r.get(&SomeEnum::B), "B");
        assert_eq!(r.get(&SomeEnum::C), "C");
    }

    #[test]
    fn test_borrowed_iteration_via_iter() {
        let r = get_test_record();
        let actual: Vec<_> = r.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let expected = get_expected_collected_vec();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_borrowed_iteration_via_for() {
        let r = get_test_record();
        let actual: Vec<_> = (&r)
            .into_iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let expected = get_expected_collected_vec();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_owned_iteration() {
        let r = get_test_record();
        let actual: Vec<_> = r.into_iter().collect();
        let expected = get_expected_collected_vec();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_serde() {
        let r = get_test_record();
        let serialized = serde_json::to_string(&r).unwrap();
        // no deserialization on Record since the order is not guaranteed in json
        let deserialized: HashMap<String, String> = serde_json::from_str(&serialized).unwrap();

        let expected = r
            .map
            .values()
            .map(|v| (v.clone(), v.clone()))
            .collect::<HashMap<String, String>>();

        assert_eq!(expected, deserialized);
    }
}
