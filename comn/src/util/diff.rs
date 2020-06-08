use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::util::join;

pub trait Diffable: Sized {
    type Diff: Diff<Value = Self> + Serialize + for<'a> Deserialize<'a>;

    fn diff(&self, other: &Self) -> Self::Diff;
}

#[derive(Debug, Clone)]
pub enum ApplyError {
    InvalidRemove,
    InvalidUpdate,
}

pub trait Diff: Sized + Serialize + for<'a> Deserialize<'a> {
    type Value: Diffable<Diff = Self>;

    fn apply(self, value: &mut Self::Value) -> Result<(), ApplyError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BTreeMapDiff<K, V>
where
    V: Diffable,
{
    pub insert: Vec<(K, V)>,
    pub remove: Vec<K>,
    pub update: Vec<(K, V::Diff)>,
}

impl<K, V> Diffable for BTreeMap<K, V>
where
    K: Serialize + for<'a> Deserialize<'a> + Ord + Clone,
    V: Serialize + for<'a> Deserialize<'a> + Diffable + PartialEq + Clone,
{
    type Diff = BTreeMapDiff<K, V>;

    fn diff(&self, other: &Self) -> Self::Diff {
        let mut diff = BTreeMapDiff {
            insert: Vec::new(),
            remove: Vec::new(),
            update: Vec::new(),
        };

        for item in join::full_join(self.iter(), other.iter()) {
            match item {
                join::Item::Left(k, _) => {
                    diff.remove.push(k.clone());
                }
                join::Item::Right(k, right) => {
                    diff.insert.push((k.clone(), right.clone()));
                }
                join::Item::Both(k, left, right) => {
                    if left != right {
                        diff.update.push((k.clone(), left.diff(right)));
                    }
                }
            }
        }

        diff
    }
}

impl<K, V> Diff for BTreeMapDiff<K, V>
where
    K: Serialize + for<'a> Deserialize<'a> + Ord + Clone,
    V: Serialize + for<'a> Deserialize<'a> + Diffable + PartialEq + Clone,
{
    type Value = BTreeMap<K, V>;

    fn apply(self, value: &mut Self::Value) -> Result<(), ApplyError> {
        for key in self.remove {
            value.remove(&key).ok_or(ApplyError::InvalidRemove)?;
        }

        value.extend(self.insert.into_iter());

        for (key, diff) in self.update {
            let value = value.get_mut(&key).ok_or(ApplyError::InvalidUpdate)?;
            diff.apply(value)?;
        }

        Ok(())
    }
}

#[macro_export]
macro_rules! impl_opaque_diff {
    ($ty:ident) => {
        impl $crate::util::diff::Diffable for $ty {
            type Diff = $ty;

            fn diff(&self, new: &Self) -> Self::Diff {
                new.clone()
            }
        }

        impl $crate::util::diff::Diff for $ty {
            type Value = $ty;

            fn apply(
                self,
                value: &mut Self,
            ) -> std::result::Result<(), $crate::util::diff::ApplyError> {
                *value = self;
                Ok(())
            }
        }
    };
}
