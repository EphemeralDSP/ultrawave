-- Renoise 3.5 real-host capture fixture. Installed only in the isolated HOME.
local root = os.getenv("ULTRAWAVE_RENOISE_TEST_ROOT")
if not root then return end

local result_path = root .. "/host-result.txt"
local function report(line)
  local f = assert(io.open(result_path, "a")); f:write(line .. "\n"); f:close()
  print("ULTRAWAVE_TEST " .. line)
end
local function contains(a, b) return string.find(string.lower(a), string.lower(b), 1, true) ~= nil end
local function parameter(device, name)
  for _, p in ipairs(device.parameters) do if p.name == name then return p end end
  error("parameter missing: " .. name)
end

-- Run each setup and render serially. Catch asynchronous callback assertions
-- too, so a failure is reported rather than waiting for the shell timeout.
local function render_sequence(song, steps, done)
  local options = {start_pos=renoise.SongPos(1,1), end_pos=renoise.SongPos(1,32),
    sample_rate=48000, bit_depth=32, interpolation="precise", priority="high"}
  local index = 0
  local function advance()
    local ok, message = pcall(function()
      index = index + 1
      local step = steps[index]
      if not step then done(); return end
      step[2]()
      local started, err = song:render(options, root .. "/" .. step[1], function()
        report("render_done=" .. step[1])
        advance()
      end)
      assert(started, tostring(err))
    end)
    if not ok then report("fatal=" .. tostring(message)) end
  end
  advance()
end

local ran, idle_count = false, 0
local function run()
  if ran then return end
  idle_count = idle_count + 1
  local song = renoise.song()
  if os.getenv("ULTRAWAVE_RENOISE_PHASE") == "reopen" then
    if song.file_name ~= root .. "/ultrawave-host-test.xrns" then return end
    ran = true
    local ok, message = pcall(function()
      local record = song.tracks[1].devices[2]
      local play = assert(song.instruments[2].plugin_properties.plugin_device)
      assert(parameter(record, "Bank").value_string == "1", "Record bank not restored")
      assert(parameter(play, "Bank").value_string == "1", "Play bank not restored")
      assert(parameter(record, "One Shot").value == 1, "One Shot not restored")
      assert(parameter(record, "Capture").value == 0, "Capture not restored")
      assert(tonumber(parameter(record, "Main Level").value_string) == 91, "MLEV not restored")
      assert(parameter(record, "Main Balance").value_string:match("L32"), "MBAL not restored")
      assert(tonumber(parameter(play, "Buffer").value_string) == 0, "Play buffer not restored")
      assert(parameter(play, "Level").value_string:match("%-3%.0"), "Play level not restored")
      report("project_controls_restored=true")
      local source_note = song.patterns[1].tracks[1].lines[1].note_columns[1]
      local play_note = song.patterns[1].tracks[2].lines[1].note_columns[1]
      local capture = parameter(record, "Capture")
      render_sequence(song, {
        {"play-after-reopen.wav", function() end}, -- saved Play note, no capture
        {"recapture-after-reopen.wav", function()
          play_note:clear()
          source_note.note_string, source_note.instrument_value, source_note.volume_value = "C-4", 0, 127
          local automation = song.patterns[1].tracks[1]:create_automation(capture)
          automation:add_point_at(1, 1)
          automation:add_point_at(2, 0)
        end},
        {"play-recaptured.wav", function()
          song.patterns[1].tracks[1]:delete_automation(capture)
          capture.value = 0
          source_note:clear()
          play_note.note_string, play_note.instrument_value, play_note.volume_value = "C-4", 1, 127
        end},
      }, function()
        record.external_editor_visible = true
        report("complete=true")
      end)
    end)
    if not ok then report("fatal=" .. tostring(message)) end
    return
  end
  -- Tools are initialized before the startup plugin scan has populated these
  -- lists. Keep idling until both bundles have actually been classified.
  local play_path, record_effect_path, record_instrument_path
  for _, info in ipairs(song.instruments[1].plugin_properties.available_plugin_infos) do
    if contains(info.name, "Ultrawave-Play") then play_path = info.path end
    if contains(info.name, "Ultrawave-Record") then record_instrument_path = info.path end
  end
  for _, info in ipairs(song.tracks[1].available_device_infos) do
    if contains(info.name, "Ultrawave-Record") then record_effect_path = info.path end
  end
  if idle_count == 20 then
    report("instrument_plugin_count=" .. #song.instruments[1].plugin_properties.available_plugins)
    report("effect_plugin_count=" .. #song.tracks[1].available_devices)
  end
  if not play_path or not record_effect_path then return end
  ran = true
  local ok, error_message = pcall(function()
    song.transport.bpm, song.transport.lpb = 120, 4
    song.patterns[1].number_of_lines = 32 -- eight beats / four seconds

    report("record_instrument=" .. tostring(record_instrument_path))
    report("record_effect=" .. tostring(record_effect_path))
    report("play_instrument=" .. tostring(play_path))
    assert(record_effect_path, "Ultrawave-Record was not discovered as a track effect")
    assert(not record_instrument_path, "Ultrawave-Record was incorrectly discovered as an instrument")
    assert(play_path, "Ultrawave-Play was not discovered as an instrument")

    -- Make a stereo source in Renoise itself. Left is a 440 Hz sine; right is
    -- silent, so successful playback also proves real main-input capture rather
    -- than the removed environment-WAV preload path.
    local source = song.instruments[1]
    source.name = "fixture-left-only-440Hz"
    source:insert_sample_at(1)
    local buffer = source.samples[1].sample_buffer
    assert(buffer:create_sample_data(48000, 32, 2, 96000))
    buffer:prepare_sample_data_changes()
    for frame = 1, 96000 do
      -- A taper distinguishes each retrigger from simply continuing the tone;
      -- a constant 440 Hz sine has identical phase at 6000/12000-frame offsets.
      local gain = 0.4 - 0.2 * (frame - 1) / 96000
      buffer:set_sample_data(1, frame, gain * math.sin(2 * math.pi * 440 * (frame - 1) / 48000))
      buffer:set_sample_data(2, frame, 0.0)
    end
    buffer:finalize_sample_data_changes()

    local source_note = song.patterns[1].tracks[1].lines[1].note_columns[1]
    source_note.note_string, source_note.instrument_value, source_note.volume_value = "C-4", 0, 127
    local record = song.tracks[1]:insert_device_at(record_effect_path, 2)
    for _, p in ipairs(record.parameters) do
      report("record_parameter=" .. p.name .. ":" .. p.value_string)
    end
    parameter(record, "Buffer").value_string = "0"
    parameter(record, "Bank").value_string = "1"
    local capture = parameter(record, "Capture")
    local capture_automation = song.patterns[1].tracks[1]:create_automation(capture)
    capture_automation:add_point_at(1, 1)
    capture_automation:add_point_at(2, 0)
    capture_automation:add_point_at(17, 1) -- repeat on the next bar, after LEN
    capture_automation:add_point_at(18, 0)
    parameter(record, "One Shot").value = 1
    parameter(record, "Main Level").value_string = "127"
    parameter(record, "Sidechain Level").value_string = "0"
    parameter(record, "Dry Monitor").value_string = "0"
    parameter(record, "Capture Monitor").value_string = "0"
    parameter(record, "Rec Length").value_string = "48" -- one bar

    song:insert_track_at(2)
    song:insert_instrument_at(2)
    assert(song.instruments[2].plugin_properties:load_plugin(play_path))
    local play = assert(song.instruments[2].plugin_properties.plugin_device)
    parameter(play, "Bank").value_string = "1"
    for _, p in ipairs(play.parameters) do report("play_parameter=" .. p.name .. ":" .. p.value_string) end
    local play_buffer = parameter(play, "Buffer")
    local play_note = song.patterns[1].tracks[2].lines[1].note_columns[1]
    render_sequence(song, {
      {"capture-pass.wav", function() end},
      {"play-r1.wav", function()
        parameter(record, "Capture").value = 0
        source_note:clear()
        play_note.note_string, play_note.instrument_value, play_note.volume_value = "C-4", 1, 127
      end},
      {"second-capture.wav", function()
        play_note:clear()
        source_note.note_string, source_note.instrument_value, source_note.volume_value = "C-5", 0, 127
        -- Automation triggers again while 880 Hz reaches the input. Dry
        -- monitoring proves that source arrived even though capture is locked.
        parameter(record, "Dry Monitor").value_string = "127"
      end},
      {"play-one-shot.wav", function()
        parameter(record, "Capture").value = 0
        parameter(record, "Dry Monitor").value_string = "0"
        source_note:clear()
        play_note.note_string, play_note.instrument_value, play_note.volume_value = "C-4", 1, 127
      end},
      {"play-reverse.wav", function()
        parameter(play, "Start").value_string = "127"
        parameter(play, "End").value_string = "0"
      end},
      {"play-retrig-120.wav", function()
        parameter(play, "Start").value_string = "0"
        parameter(play, "End").value_string = "127"
        parameter(play, "Retrigger").value_string = "2"
        parameter(play, "Retrig Time").value_string = "127"
      end},
      {"play-retrig-60.wav", function()
        -- Renoise restores transport BPM after the render callback returns.
        -- A pattern command makes the next bounce's tempo explicit on its
        -- first tick, rather than relying on a callback-time transport edit.
        local tempo = song.patterns[1].tracks[2].lines[1].effect_columns[1]
        tempo.number_string, tempo.amount_value = "ZT", 60
      end},
      {"play-retrig-infinite.wav", function()
        local tempo = song.patterns[1].tracks[2].lines[1].effect_columns[1]
        tempo.number_string, tempo.amount_value = "ZT", 120
        parameter(play, "Retrigger").value_string = "127"
        parameter(play, "Retrig Time").value_string = "32"
      end},
      {"play-retrig-off.wav", function()
        parameter(play, "Retrig Time").value_string = "0"
      end},
      {"play-r2.wav", function()
        parameter(play, "Retrigger").value_string = "0"
        play_buffer.value_string = "1"
      end},
    }, function()
      local step = 0
      local function cycle_editors()
        local success, message = pcall(function()
          step = step + 1
          record.external_editor_visible = step % 2 == 1
          play.external_editor_visible = step % 2 == 1
          if step == 10 then
            renoise.tool():remove_timer(cycle_editors)
            report("editor_open_close_cycles_per_plugin=5")
            song.patterns[1].tracks[1]:delete_automation(capture)
            capture.value = 0
            -- Save immediately, without rendering or yielding to another idle
            -- callback to deliver pending VST3 parameter updates.
            play_buffer.value_string = "0"
            parameter(record, "Main Level").value_string = "91"
            parameter(record, "Main Balance").value_string = "L32"
            parameter(play, "Level").value_string = "-3.0 dB"
            -- Save while RAM is populated and One Shot is spent. The next
            -- process must restore controls, but NOT audio or the spent latch.
            renoise.app():save_song_as(root .. "/ultrawave-host-test.xrns")
            record.external_editor_visible = true
            report("capture_complete=true")
          end
        end)
        if not success then
          renoise.tool():remove_timer(cycle_editors)
          report("fatal=Editor lifecycle: " .. tostring(message))
        end
      end
      renoise.tool():add_timer(cycle_editors, 250)
    end)
  end)
  if not ok then report("fatal=" .. tostring(error_message)) end
end

renoise.tool().app_idle_observable:add_notifier(run)
