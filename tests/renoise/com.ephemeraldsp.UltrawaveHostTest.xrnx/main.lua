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

local ran, idle_count = false, 0
local function run()
  if ran then return end
  idle_count = idle_count + 1
  local song = renoise.song()
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
      buffer:set_sample_data(1, frame, 0.4 * math.sin(2 * math.pi * 440 * (frame - 1) / 48000))
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
    parameter(record, "Capture").value = 1 -- rising edge on first rendered audio block
    parameter(record, "Main Level").value_string = "127"
    parameter(record, "Sidechain Level").value_string = "0"
    parameter(record, "Dry Monitor").value_string = "0"
    parameter(record, "Capture Monitor").value_string = "0"
    parameter(record, "Rec Length").value_string = "48" -- one bar

    song:insert_track_at(2)
    song:insert_instrument_at(2)
    assert(song.instruments[2].plugin_properties:load_plugin(play_path))
    local play = assert(song.instruments[2].plugin_properties.plugin_device)
    for _, p in ipairs(play.parameters) do report("play_parameter=" .. p.name .. ":" .. p.value_string) end
    local play_buffer = parameter(play, "Buffer")

    local options = {start_pos=renoise.SongPos(1,1), end_pos=renoise.SongPos(1,32),
      sample_rate=48000, bit_depth=32, interpolation="precise", priority="high"}
    renoise.app():save_song_as(root .. "/ultrawave-host-test.xrns")
    local started, err = song:render(options, root .. "/capture-pass.wav", function()
      report("capture_render=done")
      parameter(record, "Capture").value = 0
      source_note:clear()
      record.is_active = false
      local play_note = song.patterns[1].tracks[2].lines[1].note_columns[1]
      play_note.note_string, play_note.instrument_value, play_note.volume_value = "C-4", 1, 127
      play_buffer.value_string = "0"
      local started_r1, err_r1 = song:render(options, root .. "/play-r1.wav", function()
        report("render_r1=done")
        play_buffer.value_string = "1"
        local started_r2, err_r2 = song:render(options, root .. "/play-r2.wav", function()
          report("render_r2=done")
          local step = 0
          local function cycle_editors()
            local success, message = pcall(function()
              step = step + 1
              record.external_editor_visible = step % 2 == 1
              play.external_editor_visible = step % 2 == 1
              if step == 10 then
                renoise.tool():remove_timer(cycle_editors)
                report("editor_open_close_cycles_per_plugin=5")
                play_buffer.value_string = "0"
                play.external_editor_visible = true
                report("complete=true")
              end
            end)
            if not success then
              renoise.tool():remove_timer(cycle_editors)
              report("fatal=Editor lifecycle: " .. tostring(message))
            end
          end
          renoise.tool():add_timer(cycle_editors, 250)
        end)
        if not started_r2 then report("fatal=R2 render failed: " .. tostring(err_r2)) end
      end)
      if not started_r1 then report("fatal=R1 render failed: " .. tostring(err_r1)) end
    end)
    if not started then report("fatal=capture render failed: " .. tostring(err)) end
  end)
  if not ok then report("fatal=" .. tostring(error_message)) end
end

renoise.tool().app_idle_observable:add_notifier(run)
