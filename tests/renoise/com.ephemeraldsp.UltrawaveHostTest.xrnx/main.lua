-- Renoise 3.5 real-host fixture. Installed only in the isolated test HOME.
local root = os.getenv("ULTRAWAVE_RENOISE_TEST_ROOT")
if not root then return end

local result_path = root .. "/host-result.txt"
local function report(line)
  local file = assert(io.open(result_path, "a"))
  file:write(line .. "\n")
  file:close()
  print("ULTRAWAVE_TEST " .. line)
end
local function contains(value, part)
  return string.find(string.lower(value), string.lower(part), 1, true) ~= nil
end
local function parameter(device, name)
  for _, candidate in ipairs(device.parameters) do
    if candidate.name == name then return candidate end
  end
  error("parameter missing: " .. name)
end
local function clear_note(column) column:clear() end
local function note(column, note_name, instrument)
  column.note_string = note_name
  column.instrument_value = instrument
  column.volume_value = 127
end
local function make_tone(song, instrument_index, name, left_hz, left_gain, right_hz, right_gain)
  local instrument = song.instruments[instrument_index]
  instrument.name = name
  instrument:insert_sample_at(1)
  local buffer = instrument.samples[1].sample_buffer
  assert(buffer:create_sample_data(48000, 32, 2, 96000))
  buffer:prepare_sample_data_changes()
  for frame = 1, 96000 do
    local taper = 1 - 0.5 * (frame - 1) / 96000
    local time = (frame - 1) / 48000
    buffer:set_sample_data(1, frame, left_gain * taper * math.sin(2 * math.pi * left_hz * time))
    buffer:set_sample_data(2, frame, right_gain * taper * math.sin(2 * math.pi * right_hz * time))
  end
  buffer:finalize_sample_data_changes()
end
local function sidechain_preset(chain_index, effect_index, mute_source)
  return '<FilterDevicePreset doc_version="14"><DeviceSlot type="SideChainDevice">' ..
    '<IsActive><Value>1</Value></IsActive><MuteSource>' .. tostring(mute_source) .. '</MuteSource>' ..
    '<SendAmount><Value>1</Value></SendAmount><SendPan><Value>0.5</Value></SendPan>' ..
    '<DestChainIndex><Value>' .. chain_index .. '</Value></DestChainIndex>' ..
    '<DestEffectIndex><Value>' .. effect_index .. '</Value></DestEffectIndex>' ..
    '</DeviceSlot></FilterDevicePreset>'
end
local function configure_sidechain(device, chain_index, effect_index, mute_source)
  device.active_preset_data = sidechain_preset(chain_index, effect_index, mute_source)
  -- Presets omit song-specific routing, so set the destination explicitly too.
  parameter(device, "Dest. Track").value = chain_index
  parameter(device, "Dest. Effect").value = effect_index
end

-- Run setups and offline renders serially. pcall also covers asynchronous
-- callbacks, ensuring failures reach the shell instead of timing out silently.
local function render_sequence(song, steps, done)
  local options = {
    start_pos=renoise.SongPos(1,1), end_pos=renoise.SongPos(1,32),
    sample_rate=48000, bit_depth=32, interpolation="precise", priority="high"
  }
  local index = 0
  local function advance()
    local ok, message = pcall(function()
      index = index + 1
      local step = steps[index]
      if not step then done(); return end
      step[2]()
      local started, render_error = song:render(options, root .. "/" .. step[1], function()
        report("render_done=" .. step[1])
        advance()
      end)
      assert(started, tostring(render_error))
    end)
    if not ok then report("fatal=" .. tostring(message)) end
  end
  advance()
end

local expected_parameters = {
  "Buffer", "Record Note", "Play Note", "Playback Only", "Playback", "Re-arm",
  "Capture", "One Shot", "Capture Level", "Capture SRR", "Capture Distortion",
  "Main Level", "Main Balance", "Sidechain Level", "Sidechain Balance", "Rec Length", "Rec Rate",
  "Playback Level", "Playback SRR", "Playback Distortion", "Bit Reduction",
  "Start", "End", "Pitch", "Hold", "Decay", "Retrigger", "Retrig Time"
}

local ran, idle_count = false, 0
local function run()
  if ran then return end
  idle_count = idle_count + 1
  local song = renoise.song()

  if os.getenv("ULTRAWAVE_RENOISE_PHASE") == "reopen" then
    if song.file_name ~= root .. "/ultrawave-host-test.xrns" then return end
    ran = true
    local ok, message = pcall(function()
      local effect = song.tracks[1].devices[2]
      local alias = song.instruments[2].plugin_properties
      assert(alias.alias_fx_track_index == 1 and alias.alias_fx_device_index == 2,
        "FX alias target not restored")
      assert(alias.plugin_device and alias.plugin_device.device_path == effect.device_path,
        "FX alias no longer resolves to the saved effect")
      assert(parameter(effect, "One Shot").value == 1, "One Shot not restored")
      assert(parameter(effect, "Capture").value == 0, "Capture not restored low")
      assert(parameter(effect, "Playback").value == 0, "Playback not restored low")
      assert(parameter(effect, "Re-arm").value == 0, "Re-arm not restored low")
      assert(tonumber(parameter(effect, "Record Note").value_string) == 47, "Record Note not restored")
      assert(tonumber(parameter(effect, "Play Note").value_string) == 51, "Play Note not restored")
      assert(tonumber(parameter(effect, "Main Level").value_string) == 91, "Main Level not restored")
      assert(parameter(effect, "Main Balance").value_string:match("L32"), "Main Balance not restored")
      assert(parameter(effect, "Playback Level").value_string:match("%-3%.0"),
        "Playback Level not restored")
      assert(parameter(effect, "Playback Only").value == 0, "Playback Only not restored")
      report("project_controls_and_fx_alias_restored=true")

      local source_note = song.patterns[1].tracks[1].lines[1].note_columns[1]
      local trigger_note = song.patterns[1].tracks[2].lines[1].note_columns[1]
      render_sequence(song, {
        -- The saved alias sends Play Note 51. Instance RAM must be empty after
        -- actual host teardown even though the project was saved while populated.
        {"play-after-reopen.wav", function() end},
        {"recapture-after-reopen.wav", function()
          clear_note(trigger_note)
          note(source_note, "C-4", 0)
          note(trigger_note, "B-3", 1) -- MIDI 47: saved Record Note
        end},
        {"play-recaptured.wav", function()
          clear_note(source_note)
          note(trigger_note, "D#4", 1) -- MIDI 51: saved Play Note
        end},
      }, function()
        effect.external_editor_visible = true
        report("complete=true")
      end)
    end)
    if not ok then report("fatal=" .. tostring(message)) end
    return
  end

  -- Tools initialize before startup plugin scanning has populated these lists.
  local effect_path
  local instrument_matches = 0
  for _, info in ipairs(song.instruments[1].plugin_properties.available_plugin_infos) do
    if contains(info.name, "Ultrawave") then instrument_matches = instrument_matches + 1 end
  end
  local effect_matches = 0
  for _, info in ipairs(song.tracks[1].available_device_infos) do
    if contains(info.name, "Ultrawave") then
      effect_matches = effect_matches + 1
      effect_path = info.path
    end
  end
  if idle_count == 20 then
    report("instrument_plugin_count=" .. #song.instruments[1].plugin_properties.available_plugins)
    report("effect_plugin_count=" .. #song.tracks[1].available_devices)
  end
  if not effect_path then return end
  ran = true

  local ok, error_message = pcall(function()
    song.transport.bpm, song.transport.lpb = 120, 4
    song.transport.track_headroom = 1
    song.patterns[1].number_of_lines = 32 -- eight beats / four seconds
    report("ultrawave_effect=" .. tostring(effect_path))
    report("ultrawave_effect_matches=" .. effect_matches)
    report("ultrawave_instrument_matches_before_alias=" .. instrument_matches)
    assert(effect_matches == 1, "expected exactly one discovered Ultrawave effect")
    assert(instrument_matches == 0, "Ultrawave was incorrectly discovered as an instrument")

    -- Left-only source makes both exact stereo dry-through and mono capture
    -- independently visible in the rendered output.
    make_tone(song, 1, "fixture-left-only-440Hz", 440, 0.4, 440, 0)
    local source_note = song.patterns[1].tracks[1].lines[1].note_columns[1]
    note(source_note, "C-4", 0)
    local effect = song.tracks[1]:insert_device_at(effect_path, 2)

    local found = {}
    for _, exported in ipairs(effect.parameters) do
      report("parameter=" .. exported.name .. ":" .. exported.value_string)
      found[exported.name] = true
    end
    for _, name in ipairs(expected_parameters) do
      assert(found[name], "missing exported parameter: " .. name)
    end
    for _, removed in ipairs({"Bank", "Dry Monitor", "Capture Monitor"}) do
      assert(not found[removed], "removed parameter still exported: " .. removed)
    end
    assert(tonumber(parameter(effect, "Buffer").value_string) == 0, "Buffer default is not 0")
    assert(tonumber(parameter(effect, "Record Note").value_string) == 48, "Record Note default is not 48")
    assert(tonumber(parameter(effect, "Play Note").value_string) == 50, "Play Note default is not 50")
    assert(parameter(effect, "Playback Only").value == 0, "Playback Only default is not false")

    parameter(effect, "One Shot").value = 1
    parameter(effect, "Main Level").value_string = "127"
    parameter(effect, "Main Balance").value_string = "L64"
    parameter(effect, "Sidechain Level").value_string = "0"
    parameter(effect, "Rec Length").value_string = "48" -- one bar

    -- Renoise creates FX alias instruments by loading the effect's canonical
    -- target in its instrument-plugin UI. The Lua API exposes aliases only as
    -- read-only state, so the shell drives this one isolated UI action.
    song:insert_track_at(2)
    song:insert_instrument_at(2)
    local alias = song.instruments[2].plugin_properties
    local function finish_setup()
      assert(alias.alias_fx_track_index == 1 and alias.alias_fx_device_index == 2,
        "FX alias points to the wrong device")
      assert(alias.plugin_device and alias.plugin_device.device_path == effect.device_path,
        "FX alias did not resolve to the inserted effect")
      report("fx_alias_target=1/2")
      local trigger_note = song.patterns[1].tracks[2].lines[1].note_columns[1]
      note(trigger_note, "C-4", 1) -- MIDI 48: Record Note

    -- Renoise's #Sidechain device is the only send into a VST3 auxiliary bus.
    local sidechain_path
    for _, path in ipairs(song.tracks[1].available_devices) do
      if path:match("#Sidechain$") then sidechain_path = path end
    end
    assert(sidechain_path, "native #Sidechain device unavailable")
    report("sidechain_device_path=" .. sidechain_path)
    song:insert_track_at(3); song:insert_track_at(4)
    song:insert_instrument_at(3); song:insert_instrument_at(4)
    make_tone(song, 3, "fixture-right-only-660Hz", 660, 0, 660, 0.30)
    make_tone(song, 4, "fixture-right-only-1100Hz", 1100, 0, 1100, 0.12)
    local side_note_a = song.patterns[1].tracks[3].lines[1].note_columns[1]
    local side_note_b = song.patterns[1].tracks[4].lines[1].note_columns[1]
    local side_a = song.tracks[3]:insert_device_at(sidechain_path, 2)
    local side_b = song.tracks[4]:insert_device_at(sidechain_path, 2)

    local capture = parameter(effect, "Capture")
    local playback = parameter(effect, "Playback")
    local rearm = parameter(effect, "Re-arm")
    local function clear_notes()
      clear_note(source_note); clear_note(trigger_note); clear_note(side_note_a); clear_note(side_note_b)
    end
    local function capture_buffer(index, main_level, main_balance, side_level, side_balance)
      parameter(effect, "Buffer").value_string = tostring(index)
      parameter(effect, "Main Level").value_string = tostring(main_level)
      parameter(effect, "Main Balance").value_string = main_balance
      parameter(effect, "Sidechain Level").value_string = tostring(side_level)
      parameter(effect, "Sidechain Balance").value_string = side_balance
      note(trigger_note, "C-4", 1) -- a real Record Note edge through the FX alias
    end

      render_sequence(song, {
      -- Both actions use MIDI sent through the FX alias: C-4 records, D-4 plays.
      {"capture-pass.wav", function() end},
      {"play-r1.wav", function()
        clear_notes(); note(trigger_note, "D-4", 1)
      end},
      {"dry-plus-playback.wav", function()
        note(source_note, "C-5", 0)
      end},
      {"playback-only.wav", function()
        parameter(effect, "Playback Only").value = 1
      end},
      {"second-capture.wav", function()
        parameter(effect, "Playback Only").value = 0
        clear_notes(); note(source_note, "C-5", 0); note(trigger_note, "C-4", 1)
      end},
      {"play-one-shot.wav", function()
        clear_notes(); note(trigger_note, "D-4", 1)
      end},
      {"playback-control.wav", function()
        clear_notes()
        local automation = song.patterns[1].tracks[1]:create_automation(playback)
        automation:add_point_at(1, 1); automation:add_point_at(2, 0)
      end},
      {"play-reverse.wav", function()
        song.patterns[1].tracks[1]:delete_automation(playback)
        playback.value = 0
        note(trigger_note, "D-4", 1)
        parameter(effect, "Start").value_string = "127"
        parameter(effect, "End").value_string = "0"
      end},
      {"play-retrig-120.wav", function()
        parameter(effect, "Start").value_string = "0"
        parameter(effect, "End").value_string = "127"
        parameter(effect, "Retrigger").value_string = "2"
        parameter(effect, "Retrig Time").value_string = "127"
      end},
      {"play-retrig-60.wav", function()
        local tempo = song.patterns[1].tracks[2].lines[1].effect_columns[1]
        tempo.number_string, tempo.amount_value = "ZT", 60
      end},
      {"play-retrig-infinite.wav", function()
        local tempo = song.patterns[1].tracks[2].lines[1].effect_columns[1]
        tempo.number_string, tempo.amount_value = "ZT", 120
        parameter(effect, "Retrigger").value_string = "127"
        parameter(effect, "Retrig Time").value_string = "32"
      end},
      {"play-retrig-off.wav", function()
        parameter(effect, "Retrig Time").value_string = "0"
      end},
      {"rearm-capture.wav", function()
        parameter(effect, "Retrigger").value_string = "0"
        clear_notes(); note(source_note, "C-5", 0); note(trigger_note, "C-4", 1)
        rearm.value = 0
        local automation = song.patterns[1].tracks[1]:create_automation(rearm)
        automation:add_point_at(1, 1); automation:add_point_at(2, 0)
      end},
      {"play-rearmed.wav", function()
        song.patterns[1].tracks[1]:delete_automation(rearm)
        rearm.value = 0
        clear_notes(); note(trigger_note, "D-4", 1)
      end},
      {"play-r2.wav", function()
        parameter(effect, "Buffer").value_string = "1"
      end},
      {"side-dry-660.wav", function()
        capture.value = 0; parameter(effect, "One Shot").value = 0
        -- Sidechains can only target later tracks. Move the receiver after both
        -- senders; Renoise updates the FX alias target when the track moves.
        song:swap_tracks_at(1, 5)
        source_note = song.patterns[1].tracks[5].lines[1].note_columns[1]
        clear_notes(); note(side_note_a, "C-4", 2)
        configure_sidechain(side_a, 4, 1, false)
        configure_sidechain(side_b, 4, 1, true)
        assert(alias.alias_fx_track_index == 5 and alias.alias_fx_device_index == 2,
          "FX alias did not follow the moved effect")
        report("sidechain_receiver=" .. parameter(side_a, "Dest. Track").value_string .. "/" ..
          parameter(side_a, "Dest. Effect").value_string)
      end},
      {"side-wrong-route-capture.wav", function()
        clear_notes(); note(side_note_a, "C-4", 2)
        configure_sidechain(side_a, 4, 0, true)
        capture_buffer(2, 0, "R63", 127, "R63")
      end},
      {"side-wrong-route-play.wav", function()
        capture.value = 0; clear_notes(); note(trigger_note, "D-4", 1)
      end},
      {"side-rejected-balance-capture.wav", function()
        clear_notes(); note(side_note_a, "C-4", 2)
        configure_sidechain(side_a, 4, 1, true)
        capture_buffer(2, 0, "R63", 127, "L64")
      end},
      {"side-rejected-balance-play.wav", function()
        capture.value = 0; clear_notes(); note(trigger_note, "D-4", 1)
      end},
      {"side-single-capture.wav", function()
        clear_notes(); note(side_note_a, "C-4", 2); note(source_note, "C-4", 0)
        capture_buffer(2, 0, "L64", 127, "R63")
      end},
      {"side-single-play.wav", function()
        capture.value = 0; clear_notes(); note(trigger_note, "D-4", 1)
      end},
      {"main-only-capture.wav", function()
        clear_notes(); note(source_note, "C-4", 0)
        note(side_note_a, "C-4", 2); note(side_note_b, "C-4", 3)
        capture_buffer(3, 127, "L64", 0, "R63")
      end},
      {"main-only-play.wav", function()
        capture.value = 0; clear_notes(); note(trigger_note, "D-4", 1)
      end},
      {"side-sum-capture.wav", function()
        clear_notes(); note(side_note_a, "C-4", 2); note(side_note_b, "C-4", 3)
        capture_buffer(3, 0, "L64", 127, "R63")
      end},
      {"side-sum-play.wav", function()
        capture.value = 0; clear_notes(); note(trigger_note, "D-4", 1)
      end},
      {"save-one-shot-capture.wav", function()
        -- Finish on a known populated R1 and spend One Shot through the real
        -- Record Note. The next two renders prove this exact final latch state.
        clear_notes(); note(source_note, "C-4", 0); note(trigger_note, "C-4", 1)
        parameter(effect, "Buffer").value_string = "0"
        parameter(effect, "Main Level").value_string = "127"
        parameter(effect, "Main Balance").value_string = "L64"
        parameter(effect, "Sidechain Level").value_string = "0"
        parameter(effect, "Sidechain Balance").value_string = "R63"
        parameter(effect, "One Shot").value = 1
      end},
      {"save-one-shot-denied.wav", function()
        clear_notes(); note(source_note, "C-5", 0); note(trigger_note, "C-4", 1)
      end},
      {"save-one-shot-play.wav", function()
        clear_notes(); note(trigger_note, "D-4", 1)
      end},
      }, function()
        song:swap_tracks_at(1, 5)
        local cycle = 0
        local function cycle_editor()
          local success, message = pcall(function()
            cycle = cycle + 1
            effect.external_editor_visible = cycle % 2 == 1
            if cycle == 10 then
              renoise.tool():remove_timer(cycle_editor)
              report("editor_open_close_cycles=5")
              clear_notes(); note(trigger_note, "D#4", 1)
              capture.value = 0; playback.value = 0; rearm.value = 0
              parameter(effect, "Buffer").value_string = "0"
              parameter(effect, "Record Note").value_string = "47"
              parameter(effect, "Play Note").value_string = "51"
              parameter(effect, "Main Level").value_string = "91"
              parameter(effect, "Main Balance").value_string = "L32"
              parameter(effect, "Playback Only").value = 0
              parameter(effect, "Playback Level").value_string = "-3.0 dB"
              -- The preceding Record Note spent One Shot, its denied overwrite
              -- preserved the populated take, and nothing above re-arms it.
              -- A new process restores controls/alias, but not RAM or the latch.
              renoise.app():save_song_as(root .. "/ultrawave-host-test.xrns")
              effect.external_editor_visible = true
              report("capture_complete=true")
            end
          end)
          if not success then
            if renoise.tool():has_timer(cycle_editor) then renoise.tool():remove_timer(cycle_editor) end
            report("fatal=Editor lifecycle: " .. tostring(message))
          end
        end
        renoise.tool():add_timer(cycle_editor, 250)
      end)
    end

    song.selected_instrument_index = 2
    renoise.app().window.instrument_editor_is_detached = false
    renoise.app().window.active_middle_frame = renoise.ApplicationWindow.MIDDLE_FRAME_INSTRUMENT_PLUGIN_EDITOR
    report("fx_alias_needed=true")
    local alias_wait_count = 0
    local function wait_for_alias()
      local success, message = pcall(function()
        alias_wait_count = alias_wait_count + 1
        if alias.alias_fx_track_index == 1 and alias.alias_fx_device_index == 2 then
          renoise.tool():remove_timer(wait_for_alias)
          finish_setup()
        elseif alias_wait_count == 100 then
          renoise.tool():remove_timer(wait_for_alias)
          error("FX alias was not created through Renoise's instrument UI")
        end
      end)
      if not success then
        if renoise.tool():has_timer(wait_for_alias) then renoise.tool():remove_timer(wait_for_alias) end
        report("fatal=FX alias setup: " .. tostring(message))
      end
    end
    renoise.tool():add_timer(wait_for_alias, 100)
  end)
  if not ok then report("fatal=" .. tostring(error_message)) end
end

renoise.tool().app_idle_observable:add_notifier(run)
