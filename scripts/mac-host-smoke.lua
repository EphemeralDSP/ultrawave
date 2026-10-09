-- Run inside the private REAPER instance from mac-host-smoke.sh.
local root = assert(os.getenv("ULTRAWAVE_MAC_OUT"))
local phase = assert(os.getenv("ULTRAWAVE_MAC_PHASE"))
local log = assert(io.open(root .. "/" .. phase .. ".txt", "w"))
local function report(key, value)
  log:write(key .. "=" .. tostring(value) .. "\n")
  log:flush()
end
local function param(track, fx, name)
  for i = 0, reaper.TrackFX_GetNumParams(track, fx) - 1 do
    local _, label = reaper.TrackFX_GetParamName(track, fx, i, "")
    if label == name then return i end
  end
  error("missing parameter: " .. name)
end
local function render(file)
  reaper.GetSetProjectInfo_String(0, "RENDER_FILE", root, true)
  reaper.GetSetProjectInfo_String(0, "RENDER_PATTERN", file, true)
  reaper.GetSetProjectInfo_String(0, "RENDER_FORMAT", "evaw", true)
  reaper.GetSetProjectInfo(0, "RENDER_BOUNDSFLAG", 1, true) -- project time selection
  reaper.GetSetProjectInfo(0, "RENDER_SETTINGS", 0, true) -- master mix
  reaper.GetSetProjectInfo(0, "RENDER_SRATE", 48000, true)
  reaper.GetSet_LoopTimeRange(true, false, 0, 2, false)
  reaper.Main_OnCommand(41824, 0) -- render project using current settings
  local output = root .. "/" .. file .. ".wav"
  local handle = io.open(output, "rb")
  assert(handle, "render did not produce " .. output)
  report("render", output)
  handle:close()
end
local ok, failure = xpcall(function()
  if phase == "capture" then
    for i = reaper.CountTracks(0) - 1, 0, -1 do reaper.DeleteTrack(reaper.GetTrack(0, i)) end
    reaper.InsertTrackAtIndex(0, true)
    local track = reaper.GetTrack(0, 0)
    local fx = reaper.TrackFX_AddByName(track, "VST3: Ultrawave (EphemeralDSP)", false, 1)
    assert(fx >= 0, "unified VST3 not found")
    local _, fx_name = reaper.TrackFX_GetFXName(track, fx, "")
    report("fx", fx_name)
    assert(fx_name == "VST3: Ultrawave (EphemeralDSP)")
    assert(reaper.TrackFX_SetParamNormalized(track, fx, param(track, fx, "Resample"), 0))
    assert(reaper.TrackFX_SetParamNormalized(track, fx, param(track, fx, "Record Once"), 1))
    assert(reaper.TrackFX_SetParamNormalized(track, fx, param(track, fx, "Main Capture Level"), 91 / 127))
    assert(reaper.TrackFX_SetParamNormalized(track, fx, param(track, fx, "Capture Length"), 0))

    local source = assert(reaper.PCM_Source_CreateFromFile(root .. "/tone.wav"))
    local audio = reaper.AddMediaItemToTrack(track)
    reaper.SetMediaItemInfo_Value(audio, "D_POSITION", 0)
    reaper.SetMediaItemInfo_Value(audio, "D_LENGTH", 1.7)
    local take = reaper.AddTakeToMediaItem(audio)
    assert(reaper.SetMediaItemTake_Source(take, source))
    local midi = reaper.CreateNewMIDIItemInProj(track, 0, 2, false)
    local midi_take = reaper.GetActiveTake(midi)
    local function note(at, pitch)
      local start = reaper.MIDI_GetPPQPosFromProjTime(midi_take, at)
      local finish = reaper.MIDI_GetPPQPosFromProjTime(midi_take, at + 0.05)
      assert(reaper.MIDI_InsertNote(midi_take, false, false, start, finish, 0, pitch, 100, false))
    end
    note(0.1, 48)
    note(1.0, 50)
    reaper.MIDI_Sort(midi_take)
    render("captured-and-played")
    reaper.Main_SaveProjectEx(0, root .. "/saved.rpp", 0)
    report("project", root .. "/saved.rpp")
    reaper.TrackFX_Show(track, fx, 3)
    report("editor_open", reaper.TrackFX_GetOpen(track, fx))
    reaper.TrackFX_Show(track, fx, 2)
    report("editor_closed", not reaper.TrackFX_GetOpen(track, fx))
  elseif phase == "reopen" then
    local track = assert(reaper.GetTrack(0, 0), "saved track missing")
    local _, fx_name = reaper.TrackFX_GetFXName(track, 0, "")
    assert(fx_name == "VST3: Ultrawave (EphemeralDSP)", "saved VST3 missing: " .. fx_name)
    report("fx", fx_name)
    local level_param = param(track, 0, "Main Capture Level")
    local _, input_level = reaper.TrackFX_GetFormattedParamValue(track, 0, level_param, "")
    local _, once = reaper.TrackFX_GetFormattedParamValue(track, 0, param(track, 0, "Record Once"), "")
    report("saved_input_level", input_level)
    report("saved_record_once", once)
    assert(math.abs(reaper.TrackFX_GetParamNormalized(track, 0, level_param) - 91 / 127) < 0.000001,
      "input level did not survive reopen")
    assert(reaper.TrackFX_GetParamNormalized(track, 0, param(track, 0, "Record Once")) > 0.5,
      "Record Once did not survive reopen")
    local midi
    for i = reaper.CountTrackMediaItems(track) - 1, 0, -1 do
      local item = reaper.GetTrackMediaItem(track, i)
      if reaper.TakeIsMIDI(reaper.GetActiveTake(item)) then
        midi = item
      else
        reaper.DeleteTrackMediaItem(track, item) -- source must be gone before render
      end
    end
    assert(midi, "saved MIDI item missing")
    local take = reaper.GetActiveTake(midi)
    local _, count = reaper.MIDI_CountEvts(take)
    for i = count - 1, 0, -1 do
      local _, _, _, _, _, _, pitch = reaper.MIDI_GetNote(take, i)
      if pitch == 48 then reaper.MIDI_DeleteNote(take, i) end
    end
    reaper.MIDI_Sort(take)
    local _, remaining = reaper.MIDI_CountEvts(take)
    local _, _, _, _, _, _, remaining_pitch = reaper.MIDI_GetNote(take, 0)
    assert(remaining == 1 and remaining_pitch == 50, "expected only saved playback MIDI note")
    render("play-after-reopen")
    reaper.TrackFX_Show(track, 0, 3)
    report("editor_open_after_reopen", reaper.TrackFX_GetOpen(track, 0))
    reaper.TrackFX_Show(track, 0, 2)
    report("editor_closed_after_reopen", not reaper.TrackFX_GetOpen(track, 0))
  else error("unknown phase " .. phase) end
end, debug.traceback)
if not ok then report("fatal", failure) else report("complete", true) end
log:close()
