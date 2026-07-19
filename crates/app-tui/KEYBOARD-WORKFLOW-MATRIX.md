# GPUI ↔ TUI Keyboard and Workflow Contract

This matrix is the release contract for shared desktop concepts. GPUI owns its
keymap presets; the TUI owns terminal-friendly keys. A row is equivalent when
both front ends invoke the same player/controller operation or produce the same
persisted engine artifact. The key glyphs do not need to match.

When a binding changes, update this matrix, compact help, detailed help, and the
named dispatch test in the same change. External/OS diagnostics remain verbatim
in both front ends.

## Shared command matrix

| Concept | GPUI registry command | TUI context and keys | Required equivalent outcome | Executable evidence |
| --- | --- | --- | --- | --- |
| Play/pause | `PlayPause` (`Space`, media keys, preset aliases) | Queue: `p` or `Space`; media control | Toggle the current transport without rebuilding the queue | `tests/test_events_navigation/tests.rs::queue_p_and_space_share_the_same_play_pause_outcome`; `events/media_control.rs` |
| Next/previous track | `NextTrack` / `PrevTrack` | Queue: `n`/`>` and `b`/`<` | Advance within the current queue and preserve auto-advance semantics | `tests/test_app/tests/create.rs::test_next_track_removes_finished_album_and_advances`; previous-track tests in the same file |
| Volume | `VolumeUp` / `VolumeDown` | Normal or Configure root: `+`/`=` and `-`/`_` | Apply the shared bounded player volume | `tests/test_app/tests.rs::test_increase_volume`; `test_decrease_volume`; `test_volume_boundary_values` |
| Mute | `ToggleMute` | Normal or Configure root: `u`; media mute key | Toggle engine mute independently of the stored volume | `events/mod.rs::handle_shared_keys`; `events/media_control.rs` |
| Library search | `ToggleSearch` | Library: `/` | Enter search input without allowing global shortcuts to consume text | `tests/test_events_scenario.rs`; `events/search.rs` |
| Browse library | `SelectNext` / `SelectPrev` and arrow variants | Library: `j`/`k` or `Down`/`Up` | Move the visible selection and preserve tree/list selection state | `tests/test_app/tests/create.rs::test_select_next_tree_item`; `test_select_previous_tree_item` |
| Sort/filter library | `CycleSortOrder`, `SetSort*`, `CycleChannelFilter`, `SetFilter*` | Library: `s`, `1`–`4`, `c`, `5`–`9` | Update the shared library query model immediately | `tests/test_app/tests.rs::test_set_library_sort_order`; `test_set_channel_filter`; `test_cycle_channel_filter` |
| Add to queue | `Enter` | Library: `a` or `Enter` | Append the selected album/track without implicitly starting playback | `tests/test_app/tests/create.rs::test_add_album_to_queue_does_not_start_playback`; tree-selection counterpart |
| Browse/remove queue | selection and `RemoveItem` | Queue: arrows/`j`/`k`; `d`/`Delete` | Keep the queue selection valid and preserve playback continuity when removing entries | queue navigation/removal tests in `tests/test_app/tests/create.rs` |
| Screen jumps | `SwitchToLibrary`, `SwitchToQueue`, `SwitchToStudio`, `SwitchToDevices` | Normal/Configure root: `L`, `Q`, `P`, `O` | Change only the active surface/input mode; playback continues | `tests/test_events_navigation/tests.rs::uppercase_q_goes_to_queue`; `uppercase_p_goes_to_plugins`; adjacent navigation tests |
| Plugin selection | rack selection commands | Plugins: arrows or `j`/`k` | Keep selected plugin and visible editor synchronized | `tests/test_app/tests.rs::test_select_next_plugin`; `test_select_previous_plugin` |
| Add/remove plugin | quick-add/picker commands and `RemoveItem` | Plugins: `a`; `d`/`Delete` | Mutate the shared plugin controller and preserve a valid chain | `tests/test_app/tests.rs::test_add_plugin`; `test_remove_plugin` |
| Edit/bypass plugin | parameter commands and `TogglePlugin` | Plugins: `e`/`Enter`; `t` | Edit the same host-visible parameters and persist the bypass state | `tests/test_app/tests.rs::test_enter_exit_plugin_edit_mode`; `test_toggle_plugin`; `tests/tests_parameter_sync.rs` |
| Reorder plugin | `MovePluginUp` / `MovePluginDown` | Plugins: `u`/`U`/`Shift+Up`; `w`/`W`/`Shift+Down` | Reorder the shared chain without changing plugin identity/settings | `tests/test_app/tests.rs::test_move_plugin_up`; `test_move_plugin_down`; boundary test |
| RoomEQ easy workflow | shared `RoomEqScreenModel` and player apply actions | Configure → Room EQ: step tab plus `Left`/`Right`, `Down`/`Enter`, `BackTab`/`Esc` | Produce the same validated 2.0/2.1/5.1 rack or graph artifact and retain advanced handoff state | `events/conf_roomeq/consts.rs::tui_beginner_stereo_21_uses_shared_bass_managed_room_config`; RoomEQ navigation suites |
| Headphone EQ workflow | shared AutoEQ/headphone apply actions | Configure → Headphone EQ: step tab plus `Left`/`Right`, `Down`/`Enter`, `BackTab`/`Esc` | Apply the same filter model and preserve the editable plugin chain | `tests/integration.rs::headphone_eq_full_wizard_navigation`; `events/conf_headphoneeq/headphone.rs` tests |
| Spinorama workflow | shared AutoEQ/speaker apply actions | Configure → Spinorama EQ: step tab plus `Left`/`Right`, `Down`/`Enter`, `BackTab`/`Esc` | Apply the same optimized filters to the shared plugin chain | `tests/integration.rs::spinorama_full_wizard_workflow`; Spinorama navigation tests |
| Help/cancel | `ToggleHelp`, `Cancel` | `?`; overlays close with `Esc`/`?`/`q` | Restore the previous focus/input mode without mutating domain state | `events/mod.rs::handle_help_mode`; overlay navigation tests |
| Language | `CycleLanguage` | `Alt+L` | Cycle English, French, German, Spanish and redraw the same state | `events/mod.rs::handle_shared_keys`; locale/render tests |

## TUI-owned commands and intentional differences

These commands are terminal-shell behavior rather than cross-frontend command
concepts. They remain app-owned and must not be copied into the GPUI registry.

| TUI command | Keys/context | Rationale |
| --- | --- | --- |
| Cycle top-level panes | `Tab` in normal mode | Compact terminal navigation; GPUI uses persistent sidebar/navigation chrome. |
| Enter Configure | `C` or `N` | TUI collapses desktop settings and domain tools into one tabbed Configure surface. |
| Open Playlists | `Y` | The TUI gives Playlists its own single-key screen jump. |
| Focus level meters | `Shift+M` | Terminal panes share one focus ring; GPUI meters are directly focusable. |
| Navigate focused meters | arrows plus `m`/`s`/`c` | TUI-only pane focus model. |
| Configure tab selection | `1`–`8` | Direct terminal access to Directories, Recording, RoomEQ, Headphone EQ, Spinorama EQ, Federation, Servers, and Metadata Services. |
| Quit from main pane | `Esc`; global `Ctrl+C`, `Ctrl+Q`, or `Cmd+Q` | `Esc` is intentionally app exit only in normal mode; in overlays and workflows it backs out one level. |

## Release evidence rule

The matrix is complete only when:

1. compact and detailed help are generated from one binding catalog;
2. each documented key/context pair has an exactly-one-dispatch test;
3. each shared workflow row has a state/artifact equivalence test at the shared
   player/controller boundary; and
4. the same TUI suites pass on native macOS, Linux, and Windows CI runners.

The typed catalog in `ui/keybinding_catalog.rs` now drives runtime dispatch,
compact help, detailed screen help, and global help. Its validation tests prove
that every documented chord resolves to exactly one command in each declared
context, while navigation tests cover the conflicts found during the audit
(plugin `u`/Shift+Up and Queue `p`/Space). Shared-outcome evidence remains the
row-by-row audit above; reviewed native Linux and Windows CI results remain the
open release gate.
