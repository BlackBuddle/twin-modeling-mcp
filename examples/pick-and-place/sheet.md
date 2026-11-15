# Principle sheet: pneumatic pick and place station

Words of the person: "It is a pneumatic pick-and-place station on an aluminium base plate. Two 45 mm profile posts carry a 45 x 90 cross beam. On the front of the beam a guided rodless cylinder moves the head 200 mm in X; on its carriage a guided cylinder with a 100 mm stroke lowers a vacuum pad. A small inline ejector and a vacuum switch sit on the carriage bracket. Parts (orange plastic housings, about 40 x 40 x 25) come in on a short belt conveyor and stop against an end stop, where a diffuse photo sensor sees them. The head picks a part and puts it into a nest on the right, which has an inductive sensor. All cylinders have two air tubes and a reed switch at each end; the valves sit on a terminal at the back of the plate. There is a tower light on the beam and a box with a start button and an emergency stop at the front. Make a twin model of one cycle."

## 1. Machine

A two-axis pneumatic pick-and-place station (gantry type) with an infeed conveyor and an outfeed nest. Evidence: the words of the person; no photo, no label. Certainty: sure (for the kind of machine). Questions asked: the bore sizes, the times and the height of the beam (all kept as `guess` until the person answers; see part 7).

## 2. Parts

- `frame`: base plate on four levelling feet, two 45 x 45 profile posts, a 45 x 90 cross beam, angle brackets, the fixed run of the cable chain, the tube harness. Fixed.
- `x_cyl`: guided rodless cylinder on the front face of the beam (X). Body fixed; carriage, Z bracket, ejector and the moving run of the cable chain move.
- `z_cyl`: guided cylinder (two guide rods and a plate) on the Z bracket (Z). Body moves with X; plate and rods move down.
- `pad`: vacuum pad with a spring buffer on the Z plate. Moves with Z.
- `vac_sw`: vacuum switch on the Z bracket. Moves with X.
- `conv`: infeed belt conveyor with side guides, end stop and a gear motor. Belt moves.
- `pick_sen`: diffuse photo sensor at the end stop. Fixed.
- `nest`: outfeed nest (plastic insert with four locating blocks) on a profile pedestal. Fixed.
- `nest_sen`: inductive proximity sensor in the nest insert. Fixed.
- `valves`: valve terminal (two 5/2 double-solenoid valves, one 3/2 vacuum valve, one blank station). Fixed.
- `tower`: tower light (green, yellow, red) on the beam. Fixed.
- `panel`, `start`, `estop`: operator box with a start button and an emergency stop. Fixed.

## 3. Motion path

- Valve coils `x_cyl.coil_fwd` / `coil_back` (5/2 double) -> rodless piston -> carriage (kind `cylinder_double`, frame `out` moves +x) -> carries the Z bracket, `z_cyl`, `vac_sw` and the ejector (`on: x_cyl.out`).
- Valve coils `z_cyl.coil_fwd` / `coil_back` (5/2 double) -> piston rod and guide rods -> Z plate (kind `cylinder_double`, `pose.rot` z -90, so `fwd` is down) -> carries `pad` (`on: z_cyl.out`).
- Vacuum valve `pad.coil` -> ejector -> suction at the pad (kind `vacuum_pad`, rules R5 and R6: a pad with its coil on picks the nearest part whose top is within `reach_mm`; R2: when the coil goes off, the part goes to the highest point below it).
- Motor contactor `conv.run_fwd` -> gear motor -> belt (kind `conveyor`, carries along +x). The end stop (`stop` at point `pick`) holds the first part; the next parts queue behind it.

## 4. Signals

Direction seen from the controller.

| Pin | Dir | Type | Meaning |
|---|---|---|---|
| `x_cyl.coil_fwd` | out | bit | X to the nest |
| `x_cyl.coil_back` | out | bit | X to the conveyor |
| `x_cyl.sensor_back` | in | bit | X at the conveyor (reed switch) |
| `x_cyl.sensor_fwd` | in | bit | X at the nest (reed switch) |
| `z_cyl.coil_fwd` | out | bit | Z down |
| `z_cyl.coil_back` | out | bit | Z up |
| `z_cyl.sensor_back` | in | bit | Z up (reed switch) |
| `z_cyl.sensor_fwd` | in | bit | Z down (reed switch) |
| `pad.coil` | out | bit | vacuum on |
| `vac_sw.output` | in | bit | vacuum reached (a part is held) |
| `conv.run_fwd` | out | bit | conveyor runs forward |
| `conv.run_back` | out | bit | conveyor runs backward (not used in the cycle) |
| `pick_sen.output` | in | bit | a part is at the end stop |
| `nest_sen.output` | in | bit | a part is in the nest |
| `tower.coil_green`, `coil_yellow`, `coil_red`, `coil_buzzer` | out | bit | tower light |
| `start.contact` | in | bit | start button (NO) |
| `estop.contact` | in | bit | emergency stop (NC) |

## 5. Timing

- `x_cyl`: lag 40 ms, forward 600 ms, back 600 ms (200 mm: about 330 mm/s).
- `z_cyl`: lag 30 ms, down 350 ms, up 300 ms.
- `conv`: 150 mm/s, no lag.
- One cycle in the simulation: part at the stop 300 ms; pick 960 ms; Z up 1380 ms; X at the nest 2090 ms; Z down 2530 ms; release 2600 ms; X back 3740 ms.

## 6. Limits and sensors

- `x_cyl`: 0 to 200 mm; the pad is above the pick place at 0 mm and above the nest at 200 mm. Reed switches at both ends, window 2 mm.
- `z_cyl`: 0 to 100 mm down; pad lip at 300 mm above the floor (up) and 200 mm (down). Reed switches at both ends, window 2 mm.
- `pad`: picks a part whose top is within 4 mm of the lip.
- Belt surface 175 mm above the floor; nest seat 170 mm (5 mm lower, so that a part held up to 4 mm low still lands on the seat).
- `pick_sen`: on while a part is within 5 mm of point `pick` (the end stop).
- `nest_sen`: on while a part is within 5 mm of point `place`.
- `vac_sw`: on while the pad holds a part.

## 7. Numbers

| Name | Value | Unit | Tag | Source |
|---|---|---|---|---|
| X stroke | 200 | mm | user | words |
| Z stroke | 100 | mm | user | words |
| Part size | 40 x 40 x 25 | mm | guess | words ("about") |
| Part material | plastic | - | guess | words; not measured |
| X bore | 25 | mm | guess | |
| X body length (heads included) | 400 | mm | guess | stroke + carriage + heads |
| Z bore | 16 | mm | guess | |
| Z body length | 120 | mm | guess | |
| Pad reach | 4 | mm | guess | |
| Beam centre height | 470 | mm | guess | |
| Belt length, width, height | 380, 60, 100 | mm | guess | |
| Belt speed | 150 | mm/s | guess | |
| All times of part 5 | | ms | guess | |
| Frame, posts, plate, nest, panel sizes | | mm | guess | |

## 8. Not modelled

- Air pressure, cushions and the speed curve of a cylinder: the plant moves each axis at a constant speed after a lag.
- The vacuum level: the vacuum switch is on exactly while the pad holds a part (a `prox_sensor` with `sense.held_by`; the catalog has no pressure switch kind).
- Tubes between the valve terminal and the cylinders, and the tube to the pad, are drawn only as short stubs and a harness.
- The cable chain is drawn as a rigid shape: its bend moves with the carriage (a real chain bend moves at half the carriage speed).
- The spring buffer of the pad does not compress; a part is placed up to 5 mm above the seat and drops onto it.
- The start button, the emergency stop and the tower light are inputs and lamps only: the twin has no control program and no safety circuit.
- No collision between the head and the parts or the frame; no physics of the parts.
