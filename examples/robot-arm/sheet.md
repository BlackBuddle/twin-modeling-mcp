# Principle sheet: robot cell with vacuum tool

Words of the person: "A small robot cell. A six-axis robot (about 700 mm reach) stands on a steel pedestal 400 mm high. It has a vacuum tool on the flange: an ISO adapter, a bracket with a small ejector, a vacuum switch and one bellows pad. On a table in front of it is a fixture with six plastic caps, about 50 mm across and 30 mm high. The robot picks one cap and puts it on an outfeed belt on its right, which carries it out of the cell. The robot controller has three programs that the PLC selects by number and starts with a pulse: home, pick and place; it answers busy and done. The cell has a yellow fence with clear panels at the back and on the left, a tower light on the back fence, and a box with a start button and an emergency stop on the front post. Make a twin model of one cycle."

## 1. Machine

A robot cell: a six-axis articulated robot on a pedestal, a pick table with a part fixture, an outfeed belt conveyor, a safety fence, a tower light and an operator box. Evidence: the words of the person; no photo, no label. Certainty: sure (for the kind of cell). Questions asked: the link lengths of the robot, the taught poses, the heights of the table and the belt (all kept as `guess` until the person answers; only the teach pendant gives `measured` poses).

## 2. Parts

- `pedestal`: base plate with four anchor bolts, square column, gussets, machined top plate, the robot cable to the controller. Fixed.
- `robot`: six-axis arm (preset `robot_6r`): base, turret (J1), upper arm (J2), forearm (J3), wrist roll (J4), wrist bend (J5), flange (J6). Moves.
- `pad`: vacuum tool on the flange (adapter, tool changer, bracket, ejector with silencer, spring buffer, bellows pad). Moves with the flange.
- `vac_sw`: vacuum switch on the tool bracket. Moves with the flange.
- `table`: profile table with a top plate and a plastic fixture with six locating rings. Fixed.
- `outconv`: outfeed belt conveyor with side guides, legs and a gear motor. Belt moves.
- `fence`: five yellow posts with clear panels on the back and the left side. Fixed.
- `tower`: tower light (green, yellow, red) on the middle post of the back fence. Fixed.
- `pbox`, `start`, `estop`: operator box with a start button and an emergency stop on the front left post. Fixed.

## 3. Motion path

- Robot controller -> program number `robot.prog` and pulse `robot.start` -> joint playback (driver `playback` of the preset `robot_6r`) -> joints `j1` to `j6` (joint `revolute`) -> flange (joint `fixed`) -> carries `pad` and `vac_sw` (`on: robot.flange`).
- Joint chain of the preset: `j1` about the vertical axis at the base; `j2` at 330 mm above the base; `j3` 300 mm along the upper arm; `j4` (roll) and `j5` (bend) 320 mm along the forearm; `j6` and the flange 80 mm beyond `j5`. No link offsets.
- Vacuum valve `pad.coil` -> ejector -> suction at the pad (kind `vacuum_pad`, rules R5 and R6: a pad with its coil on picks the nearest part whose top is within `reach_mm`; R2: when the coil goes off, the part goes to the highest point below it within `reach_mm` horizontally).
- Motor contactor `outconv.run_fwd` -> gear motor -> belt (kind `conveyor`, carries along +x, out of the cell).

## 4. Signals

Direction seen from the controller (the PLC).

| Pin | Dir | Type | Meaning |
|---|---|---|---|
| `robot.prog` | out | word | program number: 1 home, 2 pick, 3 place, 4 back to home |
| `robot.start` | out | bit | start pulse |
| `robot.busy` | in | bit | the robot runs a program |
| `robot.done` | in | bit | the last program ended |
| `pad.coil` | out | bit | vacuum on |
| `vac_sw.output` | in | bit | vacuum reached (a part is held) |
| `outconv.run_fwd` | out | bit | outfeed conveyor runs forward |
| `outconv.run_back` | out | bit | outfeed conveyor runs backward (not used in the cycle) |
| `tower.coil_green`, `coil_yellow`, `coil_red`, `coil_buzzer` | out | bit | tower light |
| `start.contact` | in | bit | start button (NO) |
| `estop.contact` | in | bit | emergency stop (NC) |

## 5. Timing

Taught programs (joint values in degrees: `j1` to `j6`; each segment is a joint move of the given time):

| Program | Segment | Time (ms) | Joints | Tool place |
|---|---|---|---|---|
| 1 home | 1 | 1200 | -45, 0, 90, 0, 90, 0 | flange 320 mm out, 950 mm high; tool down |
| 2 pick | 1 | 1000 | -90, 25.71, 62.55, 0, 91.74, 0 | 80 mm above the cap at `p1` |
| | 2 | 400 | -90, 27.19, 74.87, 0, 77.94, 0 | pad lip on the top of the cap (740 mm) |
| 3 place | 1 | 400 | -90, 25.71, 62.55, 0, 91.74, 0 | 80 mm above `p1` |
| | 2 | 1000 | 0, 32.65, 51.18, 0, 96.17, 0 | 80 mm above the place point |
| | 3 | 400 | 0, 32.94, 65.10, 0, 81.96, 0 | pad lip 747 mm high (cap 7 mm above the belt) |
| 4 home | 1 | 400 | 0, 32.65, 51.18, 0, 96.17, 0 | 80 mm above the place point |
| | 2 | 900 | -45, 0, 90, 0, 90, 0 | home |

The pick and place poses were found from the link lengths (elbow up, tool vertical): `j2 + j3 + j5 = 180`. Belt speed 200 mm/s.

One cycle in the simulation: home done 1250 ms; pick pose 2850 ms; vacuum on and pick 2900 ms; place pose 4850 ms; release 4950 ms; home again 6400 ms.

## 6. Limits and sensors

- Joints: -180 to 180 degrees each (preset). Reach of the flange from the `j1` axis: up to 620 mm at the height of `j2`.
- Robot base on the pedestal top, 400 mm above the floor; `j2` 730 mm above the floor.
- `pad`: picks a cap whose top is within 6 mm of the lip; places it only within 6 mm horizontally of a point.
- Fixture: six places on a 70 mm grid; the part rests 710 mm above the floor. The robot picks `p1` (450 mm in front of the robot axis).
- Place point: on the belt, 480 mm to the right of the robot axis; belt surface 710 mm above the floor.
- `vac_sw`: on while the pad holds a part.

## 7. Numbers

| Name | Value | Unit | Tag | Source |
|---|---|---|---|---|
| Pedestal height | 400 | mm | user | words |
| Reach (about) | 700 | mm | user | words |
| `base_h_mm`, `upper_mm`, `fore_mm`, `wrist_mm` | 330, 300, 320, 80 | mm | guess | chosen to give about 700 mm reach |
| Taught poses (part 5) | | deg | guess | computed; not from a teach pendant |
| Program times | | ms | guess | |
| Tool length (flange to pad lip) | 110 | mm | guess | |
| Pad reach | 6 | mm | guess | |
| Cap size | 50 x 30 | mm | guess | words ("about") |
| Cap material | plastic | - | user | words |
| Table and belt heights | 710 | mm | guess | |
| Belt length, width, speed | 800, 100, 200 | mm, mm, mm/s | guess | |
| Fence: post height, panel width | 1400, 750 | mm | guess | |

## 8. Not modelled

- Robot dynamics, speed limits, path shape between poses (joint interpolation at a constant rate for each segment), link offsets of a real robot, singularities, collisions and the robot's own safety functions.
- The real cable dress of the robot and the tool hoses: drawn as short pieces only.
- The vacuum level: the vacuum switch is on exactly while the pad holds a part (a `prox_sensor` with `sense.held_by`; the catalog has no pressure switch kind).
- The spring buffer of the pad does not compress; a cap is released up to 7 mm above the belt and drops onto it.
- The fence, the start button, the emergency stop and the tower light are drawn and give pins, but the twin has no safety circuit and no control program.
- No collision between the robot, the tool, the parts, the table and the fence; no physics of the parts.
