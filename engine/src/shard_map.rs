//! Static [`InstrumentId`] → [`ShardId`] routing table.
//!
//! The router uses a [`ShardMap`] to decide which shard owns a given
//! instrument. The default mapping is `instrument_id % shard_count`, with
//! an optional override table for pinning known hot symbols to a specific
//! shard.
//!
//! This is deliberately a pure, immutable-ish data structure: no channels,
//! no threads, no knowledge of live [`crate::shard::ShardThread`]s. That
//! keeps routing decisions independent of shard lifecycle and makes the
//! map trivially unit-testable.
//!
//! The map is "static" only in the sense that there is no dynamic
//! rebalancing / migration. [`ShardMap::add_override`] does allow admin-time
//! additions, matching the same "register instrument at runtime" shape the
//! gateway already exposes.

use std::collections::HashMap;
use std::num::NonZeroU16;

use thiserror::Error;

use crate::commands::InstrumentId;
use crate::shard::ShardId;

/// Errors that can occur when building or mutating a [`ShardMap`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ShardMapError {
    /// `shard_count` was zero; at least one shard is required.
    #[error("shard_count must be greater than 0")]
    ZeroShardCount,
    /// An override pointed at a shard id that exceeds the configured count.
    #[error("override shard_id {shard_id} is out of range for shard_count {shard_count}")]
    OverrideOutOfRange { shard_id: ShardId, shard_count: u16 },
}

/// Maps an [`InstrumentId`] to the [`ShardId`] that owns it.
#[derive(Debug, Clone)]
pub struct ShardMap {
    shard_count: NonZeroU16,
    overrides: HashMap<InstrumentId, ShardId>,
}

impl ShardMap {
    /// Build a map with `shard_count` shards and the default modulo mapping.
    pub fn new(shard_count: u16) -> Result<Self, ShardMapError> {
        let shard_count = NonZeroU16::new(shard_count).ok_or(ShardMapError::ZeroShardCount)?;
        Ok(Self {
            shard_count,
            overrides: HashMap::new(),
        })
    }

    /// Build a map with explicit overrides. Fails on zero `shard_count` or
    /// on any override pointing at an out-of-range shard id.
    pub fn with_overrides<I>(shard_count: u16, overrides: I) -> Result<Self, ShardMapError>
    where
        I: IntoIterator<Item = (InstrumentId, ShardId)>,
    {
        let mut map = Self::new(shard_count)?;
        for (instrument_id, shard_id) in overrides {
            map.add_override(instrument_id, shard_id)?;
        }
        Ok(map)
    }

    /// Number of configured shards. Always non-zero.
    pub fn shard_count(&self) -> u16 {
        self.shard_count.get()
    }

    /// Resolve which shard owns `instrument_id`.
    ///
    /// Returns the override if one exists, otherwise `instrument_id %
    /// shard_count`. This function is total: every [`InstrumentId`] is
    /// mapped to some shard.
    pub fn shard_for(&self, instrument_id: InstrumentId) -> ShardId {
        if let Some(shard_id) = self.overrides.get(&instrument_id) {
            return *shard_id;
        }
        let count = u32::from(self.shard_count.get());
        (instrument_id % count) as ShardId
    }

    /// Pin `instrument_id` to `shard_id`. Overrides replace any previous
    /// pin for the same instrument; useful for hot-symbol placement.
    pub fn add_override(
        &mut self,
        instrument_id: InstrumentId,
        shard_id: ShardId,
    ) -> Result<(), ShardMapError> {
        if u16::from(shard_id) >= self.shard_count.get() {
            return Err(ShardMapError::OverrideOutOfRange {
                shard_id,
                shard_count: self.shard_count.get(),
            });
        }
        self.overrides.insert(instrument_id, shard_id);
        Ok(())
    }

    /// Iterate currently-configured overrides. Test-only helper kept public
    /// so operational tools can introspect the routing table.
    pub fn overrides(&self) -> impl Iterator<Item = (InstrumentId, ShardId)> + '_ {
        self.overrides.iter().map(|(k, v)| (*k, *v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zero_shard_count() {
        assert_eq!(ShardMap::new(0).unwrap_err(), ShardMapError::ZeroShardCount);
    }

    #[test]
    fn default_mapping_is_modulo() {
        let map = ShardMap::new(2).unwrap();
        assert_eq!(map.shard_count(), 2);
        assert_eq!(map.shard_for(0), 0);
        assert_eq!(map.shard_for(1), 1);
        assert_eq!(map.shard_for(2), 0);
        assert_eq!(map.shard_for(3), 1);
        assert_eq!(map.shard_for(1_000_000), 0);
        assert_eq!(map.shard_for(1_000_001), 1);
    }

    #[test]
    fn shard_for_is_deterministic() {
        let map = ShardMap::new(4).unwrap();
        for instrument_id in [1u32, 7, 42, 100, u32::MAX] {
            assert_eq!(map.shard_for(instrument_id), map.shard_for(instrument_id));
        }
    }

    #[test]
    fn override_takes_precedence_over_modulo() {
        // Without override, 7 % 2 = 1.
        let map = ShardMap::with_overrides(2, [(7u32, 0u16)]).unwrap();
        assert_eq!(map.shard_for(7), 0);
        // Other instruments still go through modulo.
        assert_eq!(map.shard_for(8), 0);
        assert_eq!(map.shard_for(9), 1);
    }

    #[test]
    fn override_out_of_range_is_rejected() {
        let err = ShardMap::with_overrides(2, [(1u32, 5u16)]).unwrap_err();
        assert_eq!(
            err,
            ShardMapError::OverrideOutOfRange {
                shard_id: 5,
                shard_count: 2,
            }
        );
    }

    #[test]
    fn add_override_replaces_existing_pin() {
        let mut map = ShardMap::new(4).unwrap();
        map.add_override(42, 0).unwrap();
        assert_eq!(map.shard_for(42), 0);
        map.add_override(42, 3).unwrap();
        assert_eq!(map.shard_for(42), 3);
    }

    #[test]
    fn overrides_iter_matches_add_calls() {
        let mut map = ShardMap::new(2).unwrap();
        map.add_override(5, 0).unwrap();
        map.add_override(6, 1).unwrap();
        let mut overrides: Vec<_> = map.overrides().collect();
        overrides.sort();
        assert_eq!(overrides, vec![(5, 0), (6, 1)]);
    }
}
