# M10 Test Checklist — Preset Save / Load

## Automated (CI)

- [x] `cargo test --workspace` passes — includes `round_trip_map` and `disk_round_trip`
- [x] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [x] `cargo fmt --all --check` clean

## Manual

### Save
- [x] Header bar shows "Patch:" label, name text field (default "Untitled"), Save button, Load button
- [x] Edit the patch name to e.g. "TestPad"
- [x] Click Save — native file dialog appears with the preset name pre-filled and `.tsmith` filter
- [x] Navigate to a folder and save — dialog closes without error
- [x] Verify the `.tsmith` file was created on disk
- [x] Open the file in a text editor — confirm it is valid RON with a `parameters` block containing human-readable keys

### Load
- [x] Click Load — native file dialog appears with `.tsmith` filter
- [x] Select the preset saved in the Save step
- [x] Dialog closes; all knobs and controls update immediately to the saved values
- [x] Patch name in header bar shows the loaded preset name
- [x] Play a note — audio sounds correct (filter cutoff, amp envelope, etc.)

### Round-trip fidelity
- [x] Set a distinctive patch: e.g. filter cutoff 500 Hz, amp attack 1 s, reverb on, arp enabled at 140 BPM
- [x] Save the preset
- [x] Reset several params to different values by hand
- [x] Load the preset back — all changed params return to the saved values
- [x] Play through the patch and confirm it sounds as originally dialled in

### Error cases
- [x] Cancel the Save dialog — no file is created, no error shown
- [x] Cancel the Load dialog — nothing changes, no error shown
- [x] Load a corrupted / invalid RON file — error message appears in header bar with a dismiss button
- [x] Dismiss the error — message disappears

### Schema version
- [x] Saved files have `version: 1` at the top of the RON structure

### Additional fix
- [x] Typing in the patch name field does not trigger piano notes or octave shifts
