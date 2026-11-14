## Part twin-modeling-mcp: check, simulate and draw the twin model

This part follows the core of the procedure (steps 1 to 6) when the program is the server twin-modeling-mcp. That server has no PLC. The core and this part together are `guide.modeling` in the result of `model_catalog`.

- Write only drafts, with `model_draft`. Do not write model files. The person confirms the draft on the page.
- Do not trust your own arithmetic. Read `facts` in the reply of the server.

### Step 0. Check the version

If you read this text in the skill folder, call `model_catalog` first. Compare `guide.modeling_version` with the number of the `Version:` line at the top of the core (the first line of `guide.modeling`). If they differ, use `guide.modeling` from the server and tell the person that the skill is old. The catalog of the core is the result of `model_catalog`: `catalog` (kinds, presets, valve types), `geometry_note`, `frames`, `mounts`, `tones`, `model_rules` (the keys that a twin model leaves out), and `guide.workpieces`, `guide.skin_rules`, `guide.skin_recipe`.

### Step 7. Save the model and read the facts

1. Call `model_draft` with `name` (1 to 32 of `A-Z a-z 0-9 - _`), `model` (the twin model) and `sheet` (the principle sheet of step 3 as text). The server stores the sheet beside the model for the person. First the skeleton, then again with the skins.
2. An error names the first wrong key and the reason. Correct the model and draft again.
3. Read `facts`, `ask`, `notes` and `not_simulated` in the reply. `facts.pins` lists every pin of the model with its direction and type. Compare it with part 4 of the sheet. Put each device of `not_simulated` in part 8 of the sheet: the plant does not move it.
4. Compare `facts` with the sheet: the place of each device (a negative y in `at_world` is below the floor), the direction words of each axis, the ends of each travel (`tip_at_0` and `tip_at_max` include `rod_out_mm`). Place a device by these numbers, not by your own sums. If a value is not the same as the sheet, correct the model and draft again.
5. Put the `ask` items to the person. Keep each answer with the tag `user`.

### Step 8. Simulate one cycle

1. Call `model_sim` with `name`, `events` (`{"t_ms", "pin", "value"}`; an event sets a pin with direction `out`), `watch` (the pins to report) and `total_ms`. Example for Example 1: events `[{"t_ms": 0, "pin": "screw.servo_on", "value": 1}, {"t_ms": 100, "pin": "screw.target", "value": 2000}]`, watch `["screw.home_sw", "screw.busy"]`, `total_ms` 3000.
2. Compare the sensor times in `changes` and the values in `joints` with parts 5 and 6 of the sheet. If a time is not the same, find the cause in the sheet or in the model, and correct it.
3. A simulation pass is not a test on real equipment.

### Step 9. Draw and compare with the photo

Ask the person to open the page address of the reply. Then call `model_render` with `name` and a `view` (`iso`, `front`, `back`, `left`, `right`, `top`). Compare the picture with the photo. The boxes come from the render, not from the photo.

### Step 10. The person confirms

The person looks at the draft on the page, plays the last simulation, and confirms it. You write only the draft. To hand the model on, give the person the model folder or the JSON. To run it on a PLC, load it in plc-mcp: there the twin model is saved as it is and its pins are bound to PLC addresses.
