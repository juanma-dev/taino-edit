//! Phase 2: position mapping — StepMap shifts, deletion flags, inversion,
//! composition, and mirror/recover across a step + its inverse.

use taino_edit_core::{Mapping, StepMap};

#[test]
fn step_map_shifts_positions() {
    // At pos 5, delete 3, insert 1.
    let m = StepMap::new(vec![5, 3, 1]);
    assert_eq!(m.map(2, 1), 2, "before the change: unchanged");
    assert_eq!(m.map(5, 1), 5, "at the start");
    assert_eq!(m.map(8, 1), 6, "at the end maps past the inserted content");
    assert_eq!(m.map(10, 1), 8, "after: shifted by newSize - oldSize");
}

#[test]
fn step_map_reports_deletion() {
    let m = StepMap::new(vec![5, 3, 1]);
    let inside = m.map_result(6, 1);
    assert!(inside.deleted(), "a position inside the replaced range");
    assert_eq!(inside.pos, 6);

    let at_start_after = m.map_result(5, 1);
    assert!(at_start_after.deleted_after());
    let at_start_before = m.map_result(5, -1);
    assert!(
        !at_start_before.deleted(),
        "association -1 at the start is not itself deleted"
    );
}

#[test]
fn step_map_inverts() {
    let m = StepMap::new(vec![5, 3, 1]);
    let inv = m.invert();
    // Forward maps 10 → 8; the inverse takes 8 back to 10.
    assert_eq!(m.map(10, 1), 8);
    assert_eq!(inv.map(8, 1), 10);
}

#[test]
fn mapping_composes_maps() {
    let mut map = Mapping::new();
    map.append_map(StepMap::new(vec![5, 3, 1])); // 10 → 8
    map.append_map(StepMap::new(vec![0, 0, 2])); // +2 at start → 10
    assert_eq!(map.len(), 2);
    assert_eq!(map.map(10, 1), 10);
}

#[test]
fn mapping_recovers_through_a_mirrored_inverse() {
    let m = StepMap::new(vec![5, 3, 1]);
    let inv = m.invert();

    // Without mirroring, a position in the deleted range does not return.
    let mut plain = Mapping::new();
    plain.append_map(m.clone());
    plain.append_map(inv.clone());
    assert_eq!(plain.map(6, 1), 8);

    // With the inverse mirrored against the original, position 6 recovers.
    let mut mirrored = Mapping::new();
    mirrored.append_map(m);
    mirrored.append_map_mirrored(inv, 0);
    assert_eq!(
        mirrored.map_result(6, 1).pos,
        6,
        "step + mirrored inverse is position-preserving"
    );
}
