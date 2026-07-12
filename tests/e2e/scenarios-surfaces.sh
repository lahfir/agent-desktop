note "Sheet surface discovery and interaction"
"$bin" focus-window --app "$app" >/dev/null 2>&1
require_value sheet_before sheet-status
act_target "$open_sheet" scroll-to >/dev/null 2>&1
act_target "$open_sheet" click >/dev/null 2>&1
sleep 0.6
surface_list="$("$bin" list-surfaces --app "$app" 2>&1)"
surface_has_sheet="$(printf '%s' "$surface_list" | grep -q '"type":"sheet"' && echo 1 || echo 0)"
assert "list-surfaces reports the opened sheet" "$surface_has_sheet" \
    "surfaces=$(json_field "$surface_list" data.surfaces)"
require_target confirm_sheet button confirm-sheet
act_target "$confirm_sheet" click >/dev/null 2>&1
sleep 0.5
require_value sheet_after sheet-status
assert "a ref found inside the sheet performs its action" \
    "$([ "$sheet_after" = "confirmed" ] && echo 1 || echo 0)" \
    "before=$sheet_before after=$sheet_after"

note "Context menu discovery and action"
"$bin" focus-window --app "$app" >/dev/null 2>&1
require_value context_before right-status
require_target context_target button context-target
act_target "$context_target" scroll-to >/dev/null 2>&1
require_target context_target button context-target
context_output="$(act_target "$context_target" right-click 2>&1)"
sleep 0.5
context_menu="$(json_field "$context_output" data.menu)"
require_target context_choice menuitem context-choice
act_target "$context_choice" click >/dev/null 2>&1
sleep 0.4
require_value context_after right-status
assert "right-click returns a verifiable context-menu snapshot" \
    "$([ "$(json_field "$context_output" ok)" = "True" ] && [ -n "$context_menu" ] && echo 1 || echo 0)" \
    "menu_snapshot=$(json_field "$context_output" data.menu_snapshot_id)"
assert "context-menu item action is independently observed" \
    "$([ "$context_after" = "context-picked" ] && echo 1 || echo 0)" \
    "before=$context_before after=$context_after"

note "Menu bar enumeration"
menu_snapshot="$("$bin" snapshot --app "$app" --surface menubar --max-depth 5 2>/dev/null)"
menu_items="$(printf '%s' "$menu_snapshot" | grep -o '"role":"menuitem"' | wc -l | tr -d ' ')"
has_fixture_menu="$(printf '%s' "$menu_snapshot" | grep -q '"name":"Fixture"' && echo 1 || echo 0)"
assert "menu bar exposes the custom Fixture menu" \
    "$([ "$has_fixture_menu" = "1" ] && [ "$menu_items" -gt 0 ] && echo 1 || echo 0)" \
    "menuitems=$menu_items fixture_menu=$has_fixture_menu"

note "Source-tracked drag gesture"
require_value drag_before drag-canvas-status
drag_anchor="$(find_target_by_id group drag-canvas || find_target group drag-canvas || true)"
if [ -n "$drag_anchor" ]; then
    act_target "$drag_anchor" scroll-to >/dev/null 2>&1
    sleep 0.3
fi
drag_snapshot="$("$bin" snapshot --app "$app" --include-bounds --max-depth 30 2>/dev/null)"
drag_points="$(printf '%s' "$drag_snapshot" | python3 "$json_tool" tree drag-canvas drag 2>/dev/null)" || drag_points=""
if [ -z "$drag_points" ]; then
    abort_suite "required drag-canvas bounds are missing"
fi
drag_from="${drag_points%% *}"
drag_to="${drag_points#* }"
drag_output="$("$bin" --headed drag --from-xy "$drag_from" --to-xy "$drag_to" \
    --duration 400 --drop-delay 300 2>&1)"
sleep 0.4
require_value drag_after drag-canvas-status
drag_effect="$(printf '%s' "$drag_after" | grep -q '^dragged-' && echo 1 || echo 0)"
assert "headed drag delivers one source-tracked gesture" \
    "$([ "$(json_field "$drag_output" ok)" = "True" ] && [ "$drag_effect" = "1" ] && echo 1 || echo 0)" \
    "from=$drag_from to=$drag_to before=$drag_before after=$drag_after"

note "Disclosure collapse and expand"
require_target disclosure disclosure disclosure-section
act_target "$disclosure" scroll-to >/dev/null 2>&1
require_target disclosure disclosure disclosure-section
collapse_output="$(act_target "$disclosure" collapse 2>&1)"
sleep 0.4
require_target disclosure disclosure disclosure-section
collapsed_value="$(json_field "$(get_target "$disclosure" --property value 2>&1)" data.value)"
require_target disclosure disclosure disclosure-section
expand_output="$(act_target "$disclosure" expand 2>&1)"
sleep 0.4
require_target disclosure disclosure disclosure-section
expanded_value="$(json_field "$(get_target "$disclosure" --property value 2>&1)" data.value)"
assert "collapse establishes the false precondition" \
    "$([ "$(json_field "$collapse_output" ok)" = "True" ] && [ "$collapsed_value" = "false" ] && echo 1 || echo 0)" \
    "value=$collapsed_value error=$(json_field "$collapse_output" error.code)"
assert "expand flips the disclosure from false to true" \
    "$([ "$(json_field "$expand_output" ok)" = "True" ] && [ "$expanded_value" = "true" ] && echo 1 || echo 0)" \
    "before=$collapsed_value after=$expanded_value error=$(json_field "$expand_output" error.code)"
