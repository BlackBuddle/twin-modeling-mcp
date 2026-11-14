## Part plc-mcp: load, bind and run the twin model in plc-mcp

This part follows the core of the procedure (steps 1 to 6) when the program is plc-mcp, the MCP server of a MELSEC PLC with a 3D process twin. The core and this part together are `guide.modeling` in the result of `twin_catalog`.

- Write only drafts, with `twin_scene_draft`. Do not write scene files. The person confirms the draft on the page.
- This procedure does not write to the PLC. Only step 8 uses PLC addresses.
- Do not trust your own arithmetic. Read `facts` in the reply of the server.

### Step 0. Check the version

If you read this text in the skill folder, call `twin_catalog` first. Compare `guide.modeling_version` with the number of the `Version:` line at the top of the core (the first line of `guide.modeling`). If they differ, use `guide.modeling` from the server and tell the person that the skill is old. If the result has no `guide.modeling`, the server is older than this text: use this text. The catalog of the core is the result of `twin_catalog`: `catalog` (kinds, presets, valve types), `geometry_note`, `frames`, `mounts`, `tones`, and `guide.order`, `guide.workpieces`, `guide.skin_rules`, `guide.skin_recipe`.

### Step 7. Save the twin model as it is

1. Call `twin_scene_draft` with `scene` (a name: 1 to 32 of `A-Z a-z 0-9 - _`) and `draft` (the twin model). First the skeleton, then again with the skins.
2. `show`, `hide` and the `pins` of an `indicators` row may name pins. plc-mcp shows them by the address that step 8 gives to that pin.
3. A signal with no `device` gives the note `<id>: signals: <name> has no device yet (bind it in step 8 of guide.modeling)`. This is correct before step 8.
4. Read `facts`, `ask`, `notes` and `not_simulated` in the reply. Put each device of `not_simulated` in part 8 of the sheet: the plant does not move it.
5. Compare `facts` with the sheet: the place of each device (a negative y in `at_world` is below the floor), the direction words of each axis, the ends of each travel (`tip_at_0` and `tip_at_max` include `rod_out_mm`). Place a device by these numbers, not by your own sums. If a value is not the same as the sheet, correct the model and draft again.
6. The reply has `saved`. `applied: false` is correct: a draft is not the scene until the person confirms it.

### Step 8. Bind the pins to PLC addresses

1. Get the address of each pin of part 4 from the person, from the I/O list, or from the comments of the PLC program.
2. Do not guess an address. If you do not know an address, leave it out and tell the person.
3. A pin of an io slot (`cylA.coil_fwd`): write the address in the `io` key of the device (`"io": {"coil_fwd": "Y10"}`). Out slots take Y or M, in slots take X or M, word slots take D.
4. A pin of a signal (`screw.target`): write the address in `device` of that signal. A bit takes X, Y or M; a word or `bcd` takes D.
5. `show`, `hide` and `indicators` keep their pins: the binding of the pins resolves them. An `indicators` row may also take the address form (`from` and `count`, `guide.skin_rules`), never together with `pins`. Add, when the person wants them: `position` (a W, R, ZR or `U<n>\G<n>` word of a servo axis, `guide.workpieces`), and the scene key `modules` (a QD77MS model).
6. Draft again. After this step the file is a plc-mcp scene, not a twin model.

### Step 9. Run one cycle

Run one cycle with `twin_sim_run`: use events on the output addresses, or a short IL program. Compare the sensor times in `changes` and the values in `joints` with parts 5 and 6 of the sheet. If a time is not the same, find the cause in the sheet or in the scene, and correct it. `twin_sim_run` refuses a `mech` that uses a signal with no `device`: bind it first. A simulation pass is not a test on real equipment.

### Step 10. Compare with the photo

When the 3D page of the dashboard is open and the scene has a photo with a camera, call `twin_render`. Compare the picture and `boxes` with the photo.

### Step 11. The person confirms

The person looks at the draft on the 3D page, adjusts places and travels, and confirms it. You write only the draft. After the confirm, the live view of the page shows the real PLC bits on the scene.
