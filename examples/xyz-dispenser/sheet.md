# Principle sheet: XYZ food dispenser for fly vials

Evidence: one photo (`photo.png`, a crop of "Figure 3 - System overview" of Wayland and Landgraf 2018, CC BY 4.0) and the description text of that figure on Wikimedia Commons: "The cartesian coordinate robot has three axes of motion (x, y and z). A Raspberry Pi (r) generates and streams G-code instructions to an Arduino (not shown) which drives stepper motors (s) via the gShield. [...] visible on this circuit board are the capacitors used to filter noise on the limit switch lines, and the two optocouplers which are the interface to the peristaltic pump (p). [...] Fly food prepared in the kettle is propelled through neoprene tubing (t) by the peristaltic pump (p) to the nozzle (n) from where it is dispensed. Boxes (b) of vials are loaded on to the platform of the robot for filling." No person spoke, the photo shows no label and no rating plate, so every number is `ref` (scaled from the reference part) or `guess`.

## 1. Machine

A cartesian (gantry) robot that fills vials with fly food. A gantry moves along the long base (y), a carriage moves along the cross beam (x), and a vertical column with the nozzle moves up and down (z). A peristaltic pump pushes the food through a tube to the nozzle. Stepper motors drive the three axes; a Raspberry Pi with a touch screen sends G-code to an Arduino with a gShield in the electronics box. Certainty: likely (the kind of machine and the three axes are sure from the text; the build details below are read from one photo).

Reference part: the stepper motor `s` on top of the z column, taken as a NEMA 17 frame with a face of 42.3 mm. Why: the text says stepper motors driven by a gShield, which drives small bipolar steppers; the motor looks square with the usual flange; and it is the only part whose size follows from a standard. In the original figure (640 x 450 px) its face is about 20 px wide, so about 2.1 mm per px at the depth of the z column. Only parts at about the same depth are scaled from it (`ref`). The long base and the cross direction are seen at a strong angle (the near end looks much larger than the far end), so their lengths cannot be scaled from one photo: they are `guess`.

Read from the photo (the person must confirm, see the questions):
- The gantry has one upright on the front rail and a cantilever cross beam; no second upright is visible at the back.
- The z column itself moves: the motor `s` sits on its top and the nozzle holder at its bottom; the z carriage block on the x carriage is fixed.
- The axes run on V-wheels on slotted aluminium profiles (black plates with wheels at the foot of the upright and on the beam).
- The y motor sits on the gantry plate at the foot of the upright, the x motor on the x carriage plate.

## 2. Parts

- `frame`: base of slotted aluminium profiles (two long rails 20 x 60, two end rails 20 x 80), platform plate with cross bars, corner brackets, pump plate, y home switch, the tube from the pump head along the floor. Fixed.
- `y_axis`: gantry (`servo_axis`): gantry plate with four V-wheels on the front rail, upright 20 x 40, y stepper motor with its pulley, cable ties, the tube up the upright. Moves along the base (frame `carriage`).
- `x_axis`: cross beam 20 x 40 with brackets and the x home switch (moves with the gantry); x carriage plate with four V-wheels, x stepper motor with its pulley, z carriage block with the z home switch, the tube arc to the z column (frame `carriage`, moves along the beam).
- `z_axis`: column 20 x 20 with the z stepper motor `s` on its top, nozzle holder (disc and bar), nozzle `n`, blue clips, the tube down the column, and the food stream while the pump runs (frame `carriage`, moves up and down).
- `ebox`: electronics box `e` (Arduino, gShield, protoshield) at the top of the upright. Moves with the gantry.
- `pump`: peristaltic pump `p` (white housing, control panel, black pump head on the far end). Fixed.
- `pi`: Raspberry Pi touch screen `r` on the pump. Fixed.
- `box1`, `box1b`, `box2`, `box2b`: two boxes `b` of 10 x 10 vials on the platform (each box is two devices, because one device draws at most 64 skin items). Fixed.

## 3. Motion path

- Y: stepper driver (driver `follow`, word `y_axis.target`, 10 counts for each mm, enable `y_axis.enable`, busy `y_axis.busy`) -> gantry (joint `form`, along world +x) -> motor and pulley angle (coupling `ratio`, `pinion_d_mm` 12.7: a GT2 belt with a 20-tooth pulley, about 40 mm for each turn) -> joint `motor` (revolute on the carriage, `wrap`).
- X: the same with `x_axis.*`: x carriage (joint `form`, along the beam, from the front to the back) -> motor (coupling `ratio`, `pinion_d_mm` 12.7).
- Z: the same with `z_axis.*`: column (joint `form`, downward) -> motor on the column top (coupling `ratio`, `lead_mm` 8: a lead screw).
- The three axes sit on each other: `x_axis` on `y_axis.carriage`, `z_axis` on `x_axis.carriage`, so the nozzle follows all three.
- Pump: `pump.run` (one of the two optocoupler outputs of the text) -> pump on (kind `pump`, state only). The food stream under the nozzle is drawn while `pump.run` is on.
- How the steppers are modelled: the catalog has no stepper or G-code kind, so each axis is a `servo_axis` whose carriage goes to a commanded position at a constant speed (driver `follow`, the motion of the preset `servo_screw` written out with the motor joint on the carriage). A G-code move of the real machine maps to new `target` words; `busy` stands for "the move is not done". The y and x motors are on their carriages, so their joints are on `carriage`; the preset would put the motor on the fixed body.

## 4. Signals

Direction seen from the controller (the Arduino with the gShield). No address.

| Pin | Dir | Type | Meaning |
|---|---|---|---|
| `y_axis.enable` | out | bit | y stepper driver enabled |
| `y_axis.target` | out | word | y position in 0.1 mm from home |
| `y_axis.busy` | in | bit | the gantry moves (stand-in for "move not done") |
| `y_axis.home_sw` | in | bit | y home / limit switch at the pump end |
| `x_axis.enable` | out | bit | x stepper driver enabled |
| `x_axis.target` | out | word | x position in 0.1 mm from home (home at the front) |
| `x_axis.busy` | in | bit | the x carriage moves |
| `x_axis.home_sw` | in | bit | x home / limit switch at the front end of the beam |
| `z_axis.enable` | out | bit | z stepper driver enabled |
| `z_axis.target` | out | word | z position in 0.1 mm down from home (home at the top) |
| `z_axis.busy` | in | bit | the column moves |
| `z_axis.home_sw` | in | bit | z home / limit switch at the top |
| `pump.run` | out | bit | peristaltic pump on |

## 5. Timing

- No lag on any axis; constant speeds: y 250 mm/s, x 200 mm/s, z 150 mm/s (stand-in speeds; no acceleration).
- One cycle in the simulation (`sim.json`): targets y 4260 and x 1080 at 100 ms (nozzle over vial (136, 42), the 5th column of box 2 from its pump-side end and the 7th row from the back); x arrives at 640 ms, y at 1810 ms; z down 100 mm from 1900 ms, arrives at 2570 ms (nozzle tip 5 mm inside the vial mouth); pump on from 2650 ms to 3150 ms; z up from 3200 ms, home at 3860 ms; y to the next vial (+28 mm) from 3950 ms to 4070 ms; z down 4150 ms to 4820 ms; pump 4900 ms to 5400 ms; z up from 5450 ms, home at 6110 ms.

## 6. Limits and sensors

- `y_axis`: 0 to 700 mm; the nozzle is at world x -290 mm at 0 and at +410 mm at 700 (the first vial column is at -276 mm, the last at +276 mm).
- `x_axis`: 0 to 300 mm; the nozzle is at world z +150 mm (front) at 0 and -150 mm (back) at 300 (vial rows from +126 mm to -126 mm).
- `z_axis`: 0 to 150 mm down; the nozzle tip is 260 mm above the table at 0 and 110 mm at 150; the vial tops are at 165 mm.
- Home switches: one for each axis at 0 mm, window 2 mm. The text names "limit switch lines", but not how many switches or where; the places are a guess (a switch on the front rail at the pump end, on the beam at the front end, on the z carriage block at the top).
- Vial grid: 10 x 10 in each box, pitch 28 mm, vial 24 mm wide and 95 mm high, standing on the tray floor 70 mm above the table.

## 7. Numbers

| Name | Value | Unit | Tag | Source |
|---|---|---|---|---|
| Stepper motor face (reference part) | 42.3 | mm | guess | assumed NEMA 17; it sets the scale of the `ref` numbers |
| Scale at the z column | 2.1 | mm/px | ref | 42.3 mm over about 20 px in the 640 x 450 figure |
| Profile series (z column width) | 20 | mm | ref | about 10 px at the z column |
| Cross beam section | 20 x 40 | mm | ref | width as the z column, two slots on its side |
| End rail height (4 slots) | 80 | mm | ref | slots counted on the end face, 20 mm each |
| Long rail section (3 slots) | 20 x 60 | mm | ref | slots counted on the front face |
| Z column length (top of the motor to the nozzle holder) | 400 + 45 | mm | ref | about 190 px plus the motor |
| Vial diameter | 24 | mm | ref | about 11 px near the z column |
| Vial height | 95 | mm | guess | the vials stand above the tray walls |
| Vials in a box, pitch | 10 x 10, 28 | -, mm | guess | rough count in the photo |
| Tray size | 290 x 50 x 290 | mm | guess | |
| Base length and width | 940 x 400 | mm | guess | seen at a strong angle; cannot be scaled |
| Upright height, beam height | 350, 380 (beam centre) | mm | guess | |
| Y, x, z travel | 700, 300, 150 | mm | guess | the arrows of the figure show directions, not travels |
| Y, x, z speed | 250, 200, 150 | mm/s | guess | |
| Counts for each mm (target word) | 10 | 1/mm | guess | |
| Belt pulley pitch diameter (y, x) | 12.7 | mm | guess | a GT2 belt and a 20-tooth pulley are usual on V-wheel builds |
| Z lead screw lead | 8 | mm | guess | |
| Home positions and windows | 0, 2 | mm | guess | |
| Pump body, head, screen, electronics box sizes | | mm | guess | |
| Pump on time for each vial | 500 | ms | guess | |
| All places (`pose`) | | mm | guess | |

## 8. Not modelled

- The G-code stream, the Arduino, the gShield and the protoshield: the twin takes a position word for each axis instead of G-code; the steppers have no steps, no microsteps, no acceleration and no lost steps.
- Homing as a motion: the twin starts at home; the home switches only report the carriage at 0.
- The graphical user interface on the touch screen and the Raspberry Pi itself (drawn only as a screen).
- The kettle and the food preparation (outside the photo crop).
- The food as a fluid: no flow, no volume, no level in the vials; the pump has only an on/off state, and the stream under the nozzle is a picture while `pump.run` is on. The pump head does not turn.
- The tube is drawn in rigid pieces on the frame, the gantry, the x carriage and the column: its loops do not follow the motion, and the pieces meet only near the middle of the travel.
- Collisions (the column and the vials, the gantry and the pump) are not checked.
- The second optocoupler output named in the text (its use is not given).

## Questions for the person

1. One reference length measured with a tape on the real machine, for example the length of a long base rail or the face of a stepper motor (scene key `refs`).
2. The travel of each axis and where each home and limit switch is; how many switches each axis has, and normally open or normally closed.
3. The drive of each axis: belt and pulley teeth (y, x), the lead of the z screw, and the steps for each mm in the controller settings.
4. The speeds and accelerations of the moves.
5. Is there a second upright at the back of the gantry? Does the z column move, or a carriage on a fixed column?
6. The pump on time (or volume) for each vial and the second optocoupler signal.
7. The number of vials in a box, the grid pitch, the vial height and the place of the first vial.
