//! This provides a function that enables the cloning of
//! some heap-data from one heap allocator to another heap allocator.
//!
//! We cannot move data directly as data can always be multiplely referenced.

use alloc::vec::Vec;
use hashbrown::HashMap;
use lepton3::lepton_vm::{
    heap_allocator::{HeapAllocator, HeapItem},
    values::Value,
};

/// This migrates the referred to value between two heap allocators
/// and returns the new value if necessary.
///
/// If no migration is required, the value is not migrated but simply
/// copied.
///
/// It is expected that the `migratee` exists in `a` and wants to be moved
/// to `b`.
pub fn migrate(a: &mut impl HeapAllocator, b: &mut impl HeapAllocator, migratee: Value) -> Value {
    // These values have already been copied from `a` to `b`
    // since we don't want to infinitely recurse or waste time  re-copying things
    // and exploding the heap.
    let mut forwarded: HashMap<usize, usize> = HashMap::new();

    // These values have been copied over from `a` to `b` but may
    // have internal values (object, arrays) which still need to
    // be copied over.
    let mut pending: Vec<usize> = Vec::new();

    // Copy the initial value over
    let migrated = migrate_value(a, b, &mut forwarded, &mut pending, migratee);

    // Migrate all of the pending values
    while let Some(pending_item) = pending.pop() {
        // Same flow as `collect` in `CheneyAllocator`, we pull
        // to migrate all of it's internal fields with &mut access.
        let mut item = core::mem::replace(b.get_item_mut(pending_item), HeapItem::Forwarded(0));

        match &mut item {
            HeapItem::Object { fields, .. } => {
                // Migrate all of the fields over from an object
                for val in fields {
                    *val = migrate_value(a, b, &mut forwarded, &mut pending, *val);
                }
            }
            HeapItem::Array(fields) => {
                // Migrate all of the fields over from an array
                for val in fields {
                    *val = migrate_value(a, b, &mut forwarded, &mut pending, *val);
                }
            }
            HeapItem::Forwarded(_) => {
                unreachable!("The pending queue should never contain a forwarded indicator")
            }
        }

        // Put the item back into it's spot.
        *b.get_item_mut(pending_item) = item;
    }

    migrated
}

/// Migrates a value from `a` to `b`
///
/// This will add the value if it's an object/array to the
/// `forwarded` set with the new position in `b`, and push it
/// to `pending`.
///
/// If it does not need to be migrated and simply can be copied,
/// then it is simply copied in rust.
fn migrate_value(
    a: &impl HeapAllocator,
    b: &mut impl HeapAllocator,
    forwarded: &mut HashMap<usize, usize>,
    pending: &mut Vec<usize>,
    val: Value,
) -> Value {
    match val {
        // These can be simply copied as there's no internal values/
        // they dont exist on the heap
        simple_migration @ (Value::Unit
        | Value::Int(_)
        | Value::UInt(_)
        | Value::Float(_)
        | Value::Bool(_)
        | Value::Tag(_)) => simple_migration,

        Value::Object(old_idx) => Value::Object(copy_item(a, b, forwarded, pending, old_idx)),
        Value::Array(old_idx) => Value::Array(copy_item(a, b, forwarded, pending, old_idx)),
    }
}

/// Copies an item over from `a` to `b` returning the position in `b`
/// this only copies it if it's not already in `forwarded`/
/// see `CheneyAllocator/migrate`.
///
/// Adds the new index to `pending` and a map from the old index
/// in `a` to the new index in `b`.
fn copy_item(
    a: &impl HeapAllocator,
    b: &mut impl HeapAllocator,
    forwarded: &mut HashMap<usize, usize>,
    pending: &mut Vec<usize>,
    old_idx: usize,
) -> usize {
    // This item was already copied earlier in this migration.
    if let Some(&new_idx) = forwarded.get(&old_idx) {
        return new_idx;
    }

    // This should be the first time we try copy it over since
    // we havent seen it in `forwarded`.
    let item = a.get_item(old_idx);
    if let HeapItem::Forwarded(_) = item {
        unreachable!(
            "Should never try to copy over a forwarded item, else gc collection died midway through"
        )
    };

    // Re-allocate it over in `b` to copy it into b
    // and add to forwarded/pending so we don't re-allocate it
    // and migrate its internal stuff
    let new_idx = b.alloc_raw(item.clone());
    forwarded.insert(old_idx, new_idx);
    pending.push(new_idx);

    new_idx
}
