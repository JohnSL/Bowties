//! Saved signal IDs win: editing an existing facility keeps the IDs its
//! signals were saved under, matched by protected block.

use bowties_core::signaling::{
    preview_block_signals, BlockSignalsRequest, SavedSignalIdentity, SelectedBlock, TravelDirection,
};

fn block(id: &str) -> SelectedBlock {
    SelectedBlock { block_id: id.into(), name: id.into(), joined_to_previous: false, turnouts: vec![] }
}

fn one_block(direction: TravelDirection, saved: Vec<SavedSignalIdentity>) -> BlockSignalsRequest {
    BlockSignalsRequest {
        facility_id: "abs:f".into(),
        direction,
        blocks: vec![block("block:a")],
        saved_signals: saved,
        ..Default::default()
    }
}

fn saved(signal_id: &str, block: &str) -> SavedSignalIdentity {
    SavedSignalIdentity { signal_id: signal_id.into(), protected_block: block.into() }
}

fn ids(request: BlockSignalsRequest) -> Vec<String> {
    preview_block_signals(request).signals.into_iter().map(|s| s.signal_id).collect()
}

#[test]
fn reversed_one_block_line_keeps_the_saved_signal_id() {
    let saved_id = "signal:abs:f:block:a:eastward";
    let request = one_block(TravelDirection::Westward, vec![saved(saved_id, "block:a")]);
    assert_eq!(ids(request), [saved_id]);
}

#[test]
fn unmatched_signal_keeps_the_derived_id() {
    let request = one_block(TravelDirection::Westward, vec![saved("signal:old", "block:other")]);
    assert_eq!(ids(request), ["signal:abs:f:block:a:westward"]);
}

#[test]
fn no_saved_signals_derives_ids() {
    let request = one_block(TravelDirection::Eastward, vec![]);
    assert_eq!(ids(request), ["signal:abs:f:block:a:eastward"]);
}

#[test]
fn a_saved_signal_is_claimed_by_at_most_one_derived_signal() {
    let mut request = one_block(TravelDirection::Eastward, vec![saved("signal:old", "block:a")]);
    request.movements = bowties_core::signaling::Movements::BothDirections;
    let got = ids(request);
    assert_eq!(got.len(), 2);
    assert_eq!(got.iter().filter(|id| id.as_str() == "signal:old").count(), 1);
}

#[test]
fn saved_id_carries_names_keyed_by_it() {
    let mut request = one_block(TravelDirection::Westward, vec![saved("signal:old", "block:a")]);
    request.signal_names.insert("signal:old".into(), "Main Home".into());
    let preview = preview_block_signals(request);
    assert_eq!(preview.signals[0].name, "Main Home");
}

#[test]
fn reordering_a_saved_three_block_line_keeps_each_signal_id_by_its_protected_block() {
    let line = |order: &[&str], saved: Vec<SavedSignalIdentity>| BlockSignalsRequest {
        facility_id: "abs:f".into(),
        blocks: order.iter().map(|b| block(&format!("block:{b}"))).collect(),
        saved_signals: saved,
        ..Default::default()
    };
    let original = ids(line(&["a", "b", "c"], vec![]));
    let saved_all: Vec<SavedSignalIdentity> =
        original.iter().zip(["a", "b", "c"]).map(|(id, b)| saved(id, &format!("block:{b}"))).collect();
    let reordered = ids(line(&["c", "a", "b"], saved_all));
    assert_eq!(reordered, [original[2].clone(), original[0].clone(), original[1].clone()]);
}
