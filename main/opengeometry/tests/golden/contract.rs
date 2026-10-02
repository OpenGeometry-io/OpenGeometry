use crate::extras::{sweep_digest, sweep_outputs, SweepOutputs};
use crate::golden_file::{compare, dump, mode, parse, update_mode, Mode, RECORD_INSTRUCTION};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn keyed(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(key, digest)| (key.to_string(), digest.to_string()))
        .collect()
}

fn flip_first_bit(bytes: &mut [u8]) {
    bytes[0] ^= 1;
}

#[test]
fn hash_changes_when_one_bit_of_any_hashed_output_flips() {
    let outputs = sweep_outputs().unwrap();
    let original = sweep_digest(&outputs);
    let flips: [fn(&mut SweepOutputs); 8] = [
        |outputs| flip_first_bit(&mut outputs.brep_json),
        |outputs| outputs.positions[0] = f64::from_bits(outputs.positions[0].to_bits() ^ 1),
        |outputs| outputs.normals[0] = f32::from_bits(outputs.normals[0].to_bits() ^ 1),
        |outputs| outputs.indices[0] ^= 1,
        |outputs| outputs.face_ids[0] ^= 1,
        |outputs| flip_first_bit(&mut outputs.verdicts[0]),
        |outputs| flip_first_bit(&mut outputs.step),
        |outputs| flip_first_bit(&mut outputs.report),
    ];
    for (index, flip) in flips.iter().enumerate() {
        let mut changed = outputs.clone();
        flip(&mut changed);
        assert_ne!(sweep_digest(&changed), original, "flip {index}");
    }
}

#[test]
fn a_scene_missing_from_the_golden_file_fails_with_the_record_instruction() {
    let expected = keyed(&[("kept", "aa")]);
    let actual = keyed(&[("kept", "aa"), ("added", "bb")]);
    let mismatch = compare(&expected, &actual).unwrap_err();
    assert_eq!(mismatch.missing, ["added"]);
    let message = mismatch.to_string();
    assert!(message.contains("added"), "{message}");
    assert!(message.contains(RECORD_INSTRUCTION), "{message}");
}

#[test]
fn a_golden_key_with_no_scene_fails() {
    let expected = keyed(&[("kept", "aa"), ("removed", "bb")]);
    let actual = keyed(&[("kept", "aa")]);
    let mismatch = compare(&expected, &actual).unwrap_err();
    assert_eq!(mismatch.stale, ["removed"]);
    assert!(mismatch.to_string().contains("removed"));
}

#[test]
fn a_scene_whose_digest_differs_from_the_golden_fails() {
    let expected = keyed(&[("kept", "aa"), ("moved", "bb")]);
    let actual = keyed(&[("kept", "aa"), ("moved", "cc")]);
    let mismatch = compare(&expected, &actual).unwrap_err();
    assert_eq!(mismatch.differing, ["moved"]);
    assert!(mismatch.to_string().contains("moved"));
    assert!(compare(&actual, &actual).is_ok());
}

#[test]
fn an_unrecorded_target_fails_with_the_record_instruction() {
    let expected = parse("{}").unwrap();
    let actual = keyed(&[("scene", "aa")]);
    let message = compare(&expected, &actual).unwrap_err().to_string();
    assert!(message.contains("no golden is recorded"), "{message}");
    assert!(message.contains(RECORD_INSTRUCTION), "{message}");
}

#[test]
fn the_update_path_refuses_on_ci_outside_the_record_job() {
    let table = [
        (Some("1"), Some("true"), Some("push"), None),
        (Some("1"), Some("true"), Some("pull_request"), None),
        (Some("1"), Some("true"), None, None),
        (
            Some("1"),
            Some("true"),
            Some("workflow_dispatch"),
            Some(Mode::Record),
        ),
        (Some("1"), None, None, Some(Mode::Record)),
        (Some("1"), None, Some("push"), Some(Mode::Record)),
        (None, None, None, Some(Mode::Compare)),
        (None, Some("true"), Some("push"), Some(Mode::Compare)),
        (Some("0"), None, None, Some(Mode::Compare)),
        (
            Some("true"),
            Some("true"),
            Some("workflow_dispatch"),
            Some(Mode::Compare),
        ),
        (Some(""), None, None, Some(Mode::Compare)),
    ];
    for (update, github_actions, event_name, allowed) in table {
        let chosen = update_mode(update, github_actions, event_name).ok();
        assert_eq!(
            chosen, allowed,
            "{update:?} {github_actions:?} {event_name:?}"
        );
    }
}

#[test]
fn the_dump_mode_writes_every_case_without_comparing_the_golden() {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("golden-dump-mode");
    let chosen = mode(Some("1"), None, None, Some(directory.clone()));
    assert_eq!(chosen, Ok(Mode::Dump(directory.clone())));
    if directory.exists() {
        fs::remove_dir_all(&directory).unwrap();
    }
    let texts = keyed(&[
        ("first.case", "first text\n"),
        ("second.case", "second text\n"),
    ]);
    dump(&directory, &texts).unwrap();
    let mut written = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    written.sort();
    assert_eq!(written, ["first.case.txt", "second.case.txt"]);
    let first = fs::read_to_string(directory.join("first.case.txt")).unwrap();
    assert_eq!(first, "first text\n");
    assert!(dump(&directory, &texts).is_err());
}
