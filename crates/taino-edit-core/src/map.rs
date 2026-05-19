//! Position mapping: [`StepMap`] (how one step moves positions) and
//! [`Mapping`] (a composable, mirror-aware pipeline of them).
//!
//! Faithful port of ProseMirror's `map.ts`. The mirror/recover machinery is
//! what lets a position survive being mapped through a step and later its
//! inverse (undo/redo) instead of collapsing into a deleted range.

const LOWER16: u64 = 0xffff;
const FACTOR16: u64 = 1 << 16;

fn make_recover(index: usize, offset: usize) -> u64 {
    index as u64 + (offset as u64) * FACTOR16
}
fn recover_index(value: u64) -> usize {
    (value & LOWER16) as usize
}
fn recover_offset(value: u64) -> usize {
    ((value - (value & LOWER16)) / FACTOR16) as usize
}

/// Position was deleted on the side the association pointed at.
pub const DEL_SIDE: u8 = 8;
const DEL_BEFORE: u8 = 1;
const DEL_AFTER: u8 = 2;
const DEL_ACROSS: u8 = 4;

/// The result of mapping a single position: the new position plus
/// information about whether the content around it was deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapResult {
    /// The mapped position.
    pub pos: usize,
    del_info: u8,
    recover: Option<u64>,
}

impl MapResult {
    /// Whether the position itself fell inside deleted content (on the
    /// associated side).
    pub fn deleted(&self) -> bool {
        self.del_info & DEL_SIDE > 0
    }
    /// Whether content directly before the position was deleted.
    pub fn deleted_before(&self) -> bool {
        self.del_info & (DEL_BEFORE | DEL_ACROSS) > 0
    }
    /// Whether content directly after the position was deleted.
    pub fn deleted_after(&self) -> bool {
        self.del_info & (DEL_AFTER | DEL_ACROSS) > 0
    }
    /// Whether the position sat strictly inside deleted content.
    pub fn deleted_across(&self) -> bool {
        self.del_info & DEL_ACROSS > 0
    }
}

/// How one step remaps positions. `ranges` is a flat list of
/// `(start, old_size, new_size)` triples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepMap {
    ranges: Vec<usize>,
    inverted: bool,
}

impl StepMap {
    /// A map from `(start, old_size, new_size)` triples.
    pub fn new(ranges: Vec<usize>) -> Self {
        debug_assert!(ranges.len() % 3 == 0);
        StepMap {
            ranges,
            inverted: false,
        }
    }

    /// The identity map (no position changes).
    pub fn identity() -> Self {
        StepMap {
            ranges: Vec::new(),
            inverted: false,
        }
    }

    /// The inverse of this map (swaps old/new sizes).
    pub fn invert(&self) -> StepMap {
        StepMap {
            ranges: self.ranges.clone(),
            inverted: !self.inverted,
        }
    }

    fn recover(&self, value: u64) -> usize {
        let mut diff: i64 = 0;
        let index = recover_index(value);
        if !self.inverted {
            for i in 0..index {
                diff += self.ranges[i * 3 + 2] as i64 - self.ranges[i * 3 + 1] as i64;
            }
        }
        (self.ranges[index * 3] as i64 + diff + recover_offset(value) as i64) as usize
    }

    /// Map `pos` to its new location (association `assoc`: `-1` biases toward
    /// content before the position, `1` toward content after).
    pub fn map(&self, pos: usize, assoc: i32) -> usize {
        self.map_inner(pos, assoc).pos
    }

    /// Like [`map`](StepMap::map) but also reports deletion around `pos`.
    pub fn map_result(&self, pos: usize, assoc: i32) -> MapResult {
        self.map_inner(pos, assoc)
    }

    fn map_inner(&self, pos: usize, assoc: i32) -> MapResult {
        let mut diff: i64 = 0;
        let mut i = 0;
        while i < self.ranges.len() {
            let start_raw = self.ranges[i] as i64 - if self.inverted { diff } else { 0 };
            if start_raw > pos as i64 {
                break;
            }
            let start = start_raw as usize;
            let old_size = self.ranges[i + if self.inverted { 2 } else { 1 }];
            let new_size = self.ranges[i + if self.inverted { 1 } else { 2 }];
            let end = start + old_size;
            if pos <= end {
                let side = if old_size == 0 {
                    assoc
                } else if pos == start {
                    -1
                } else if pos == end {
                    1
                } else {
                    assoc
                };
                let result =
                    (start as i64 + diff + if side < 0 { 0 } else { new_size as i64 }) as usize;
                let edge = if assoc < 0 { start } else { end };
                let recover = if pos == edge {
                    None
                } else {
                    Some(make_recover(i / 3, pos - start))
                };
                let mut del_info = if pos == start {
                    DEL_AFTER
                } else if pos == end {
                    DEL_BEFORE
                } else {
                    DEL_ACROSS
                };
                let on_edge = if assoc < 0 { pos != start } else { pos != end };
                if on_edge {
                    del_info |= DEL_SIDE;
                }
                return MapResult {
                    pos: result,
                    del_info,
                    recover,
                };
            }
            diff += new_size as i64 - old_size as i64;
            i += 3;
        }
        MapResult {
            pos: (pos as i64 + diff) as usize,
            del_info: 0,
            recover: None,
        }
    }
}

/// A composable pipeline of [`StepMap`]s, with optional mirror links so a
/// position can be recovered when mapped through a map and its mirror.
#[derive(Debug, Clone, Default)]
pub struct Mapping {
    maps: Vec<StepMap>,
    /// Flat pairs `[a, b, ...]` linking mirrored map indices.
    mirror: Vec<usize>,
}

impl Mapping {
    /// An empty mapping.
    pub fn new() -> Self {
        Mapping::default()
    }

    /// Number of maps in the pipeline.
    pub fn len(&self) -> usize {
        self.maps.len()
    }

    /// Whether the pipeline is empty.
    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }

    /// The maps, in order.
    pub fn maps(&self) -> &[StepMap] {
        &self.maps
    }

    /// Append a map to the pipeline.
    pub fn append_map(&mut self, map: StepMap) {
        self.maps.push(map);
    }

    /// Append `map`, mirrored against the existing map at index `mirrors`.
    pub fn append_map_mirrored(&mut self, map: StepMap, mirrors: usize) {
        self.maps.push(map);
        let n = self.maps.len() - 1;
        self.mirror.push(mirrors);
        self.mirror.push(n);
    }

    fn get_mirror(&self, n: usize) -> Option<usize> {
        let mut i = 0;
        while i < self.mirror.len() {
            if self.mirror[i] == n {
                return Some(self.mirror[i + 1]);
            }
            if self.mirror[i + 1] == n {
                return Some(self.mirror[i]);
            }
            i += 2;
        }
        None
    }

    /// Map `pos` through the whole pipeline.
    pub fn map(&self, pos: usize, assoc: i32) -> usize {
        let mut pos = pos;
        let mut i = 0;
        while i < self.maps.len() {
            pos = self.maps[i].map(pos, assoc);
            i += 1;
        }
        pos
    }

    /// Map `pos`, skipping across mirrored map pairs via recovery so a
    /// position survives a step followed by its inverse.
    pub fn map_result(&self, pos: usize, assoc: i32) -> MapResult {
        let mut del_info = 0u8;
        let mut pos = pos;
        let mut i = 0;
        while i < self.maps.len() {
            let result = self.maps[i].map_result(pos, assoc);
            if let Some(rec) = result.recover {
                if let Some(corr) = self.get_mirror(i) {
                    if corr > i && corr < self.maps.len() {
                        i = corr;
                        pos = self.maps[corr].recover(rec);
                        i += 1;
                        continue;
                    }
                }
            }
            del_info |= result.del_info;
            pos = result.pos;
            i += 1;
        }
        MapResult {
            pos,
            del_info,
            recover: None,
        }
    }
}
