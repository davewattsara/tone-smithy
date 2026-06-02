# M10 Test Checklist — Preset Save / Load

## Automated (CI)

- [ ] `cargo test --workspace` passes — includes `round_trip_map` and `disk_round_trip`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo fmt --all --check` clean

## Manual

### Save
- [ ] Header bar shows "Patch:" label, name text field (default "Untitled"), Save button, Load button
- [ ] Edit the patch name to e.g. "TestPad"
- [ ] Click Save — native file dialog appears with the preset name pre-filled and `.tsmith` filter
- [ ] Navigate to a folder and save — dialog closes without error
- [ ] Verify the `.tsmith` file was created on disk
- [ ] Open the file in a text editor — confirm it is valid RON with a `parameters` block containing human-readable keys

### Load
- [ ] Click Load — native file dialog appears with `.tsmith` filter
- [ ] Select the preset saved in the Save step
- [ ] Dialog closes; all knobs and controls update immediately to the saved values
- [ ] Patch name in header bar shows the loaded preset name
- [ ] Play a note — audio sounds correct (filter cutoff, amp envelope, etc.)

### Round-trip fidelity
- [ ] Set a distinctive patch: e.g. filter cutoff 500 Hz, amp attack 1 s, reverb on, arp enabled at 140 BPM
- [ ] Save the preset
- [ ] Reset several params to different values by hand
- [ ] Load the preset back — all changed params return to the saved values
- [ ] Play through the patch and confirm it sounds as originally dialled in

### Error cases
- [ ] Cancel the Save dialog — no file is created, no error shown
- [ ] Cancel the Load dialog — nothing changes, no error shown
- [ ] Load a corrupted / invalid RON file — error message appears in header bar with a dismiss button
- [ ] Dismiss the error — message disappears

### Schema version
- [ ] Saved files have `version: 1` at the top of the RON structure
