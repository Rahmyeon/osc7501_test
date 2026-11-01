local wezterm = require 'wezterm'
local config = wezterm.config_builder()

config.initial_cols = 110
config.initial_rows = 30
config.font_size = 13
config.enable_tab_bar = true
config.hide_tab_bar_if_only_one_tab = false
config.use_fancy_tab_bar = false
config.tab_max_width = 80
config.exit_behavior = 'Hold'
config.scrollback_lines = 5000
config.colors = { foreground = '#e8f1f8', background = '#101b24' }

-- This example summarizes every split in a tab. Full records stay in the model.
local priorities = { blocked = 5, error = 4, working = 3, done = 2, idle = 1 }
local colors = {
  blocked = '#f6cc7d', error = '#ffbac5', working = '#9ecbed',
  done = '#94dfbe', idle = '#b5ccdc',
}
local formatted_records = {}
local notification_arms = {}
local result_dir = os.getenv 'OSC7501_ALPHA_RESULT_DIR'

local function safe_label(text)
  local chars = {}
  for _, cp in utf8.codes(text) do
    -- Reports are plain text. Disarm invisible direction/formatting characters
    -- before showing a program-provided label outside the terminal grid.
    if not ((cp >= 0x202a and cp <= 0x202e)
        or (cp >= 0x2066 and cp <= 0x2069)
        or (cp >= 0x200b and cp <= 0x200f)
        or cp == 0x061c or cp == 0xfeff) then
      table.insert(chars, utf8.char(cp))
    end
  end
  return wezterm.truncate_right(table.concat(chars), 24)
end

wezterm.on('format-tab-title', function(tab)
  local selected, selected_pane
  local count = 0
  for _, pane in ipairs(tab.panes) do
    local records = pane.program_status or {}
    if result_dir then
      formatted_records[pane.pane_id] = records
      -- Arm only after the SetUserVar-triggered title refresh has reached us.
      -- The shell sends the new report after the acknowledgement timer fires.
      local arm = notification_arms[pane.pane_id]
      if arm then
        notification_arms[pane.pane_id] = nil
        wezterm.time.call_after(0.2, arm)
      end
    end
    for _, record in ipairs(records) do
      count = count + 1
      if not selected or priorities[record.state] > priorities[selected.state] then
        selected, selected_pane = record, pane
      end
    end
  end
  local title = tab.tab_title
  if not title or title == '' then title = tab.active_pane.title end
  if not selected then return ' ALPHA ' .. (tab.tab_index + 1) .. ': ' .. title .. ' ' end
  local label = selected.title or selected.effective_app
  if not label then label = selected.id ~= '' and selected.id or 'task' end
  local detail = selected.kind and ':' .. selected.kind or ''
  if selected.progress ~= nil then detail = detail .. ' ' .. selected.progress .. '%' end
  return {
    { Foreground = { Color = colors[selected.state] } },
    { Text = string.format(' ALPHA %d | P%d %s%s | %s | %d records ',
        tab.tab_index + 1, selected_pane.pane_index + 1,
        selected.state:upper(), detail, safe_label(label), count) },
  }
end)

wezterm.on('format-window-title', function() return 'WezTerm OSC 7501 ALPHA' end)

wezterm.on('osc7501-alpha-inspect', function(window, pane)
  wezterm.log_info('OSC 7501 records: ' .. wezterm.serde.json_encode(pane:get_program_status()))
  window:perform_action(wezterm.action.ShowDebugOverlay, pane)
end)

config.keys = {
  { key = 'U', mods = 'CTRL|SHIFT', action = wezterm.action.EmitEvent 'osc7501-alpha-inspect' },
}

-- Test-only bridge. The script emits an ordinary SetUserVar after each report
-- batch. Assertions read both the live Lua API and title-formatting snapshots.
-- Results are written to this run's local directory, never back to the PTY.
if result_dir then
  local received_checks = {}
  local function compare(actual, expected)
    if #actual ~= #expected then return 'record count differs' end
    for index, want in ipairs(expected) do
      local got = actual[index]
      for key, value in pairs(want.values) do
        if got[key] ~= value then return 'record ' .. index .. ' field ' .. key .. ' differs' end
      end
      for _, key in ipairs(want.absent or {}) do
        if got[key] ~= nil then return 'record ' .. index .. ' field ' .. key .. ' should be absent' end
      end
      if index > 1 and actual[index - 1].id > got.id then return 'records are not sorted by id' end
    end
  end

  wezterm.on('user-var-changed', function(_, pane, name, value)
    if name ~= 'osc7501_alpha_check' then return end
    local request = wezterm.serde.json_decode(value)
    if not request.case:match('^[a-z0-9_-]+$') then return end
    if received_checks[request.case] then return end
    received_checks[request.case] = true
    local attempts = 0
    local function check()
      attempts = attempts + 1
      local title_records = formatted_records[pane:pane_id()]
      -- The notification case reads only what the formatter already observed.
      -- It must not trigger a title refresh or send another user-variable event.
      local actual = request.notification_only and (title_records or {}) or pane:get_program_status()
      local problem = compare(actual, request.expected)
      if request.notification_only and not title_records then
        problem = 'no title-formatting snapshot observed'
      elseif not problem and not request.notification_only then
        if title_records then problem = compare(title_records, request.expected)
        else problem = 'no title-formatting snapshot observed' end
      end
      if problem and attempts < 50 then
        wezterm.time.call_after(0.1, check)
        return
      end
      local result = { ok = problem == nil, case = request.case, problem = problem, actual = actual,
        source = request.notification_only and 'formatter_cache_only' or 'pane_and_formatter' }
      local file, err = io.open(result_dir .. '/' .. request.case .. '.json', 'w')
      if not file then wezterm.log_error(err); return end
      file:write(wezterm.serde.json_encode(result))
      file:close()
      if request.complete then
        local complete = assert(io.open(result_dir .. '/complete.json', 'w'))
        complete:write(wezterm.serde.json_encode({ ok = result.ok, checks = 12, error = problem }))
        complete:close()
      end
    end
    if request.notification_only then
      notification_arms[pane:pane_id()] = function()
        local armed = assert(io.open(result_dir .. '/' .. request.case .. '-armed.json', 'w'))
        armed:write(wezterm.serde.json_encode({ armed = true }))
        armed:close()
        wezterm.time.call_after(0.2, check)
      end
    else
      wezterm.time.call_after(0.2, check)
    end
  end)
end

return config
