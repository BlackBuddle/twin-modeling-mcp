# Principle sheet: cam lifter

Words of the person: "This is the lift unit under our pallet stop. A small geared motor turns a shaft through a jaw coupling; the shaft runs in two pillow-block bearings on risers and carries a pear-shaped disc cam, 100 mm base circle and a 50 mm nose with its centre 65 mm from the shaft. A 30 mm cam roller in a fork rides on top of the cam and pushes the lift plate up through a push rod; the plate is guided by two 16 mm shafts in flanged linear bushings in a bridge on four posts. One turn lifts the tray 40 mm and puts it down again. Two M12 proximity switches on the bridge see a dog on the plate for up and down, and a slot sensor with a flag on the shaft end gives the home of the turn. The PLC switches the motor on and stops it at home."

This model keeps the principle of Example 2 of the skill (driver `coils` with `valve: none`, coupling `table`, prismatic lift); the profile is the one of the drawn pear cam, not the profile of Example 2.

1. Machine: a motor-driven disc-cam lift unit with a radial (in-line) roller follower. Evidence: the words of the person. Certainty: sure. Questions: the motor speed is not known (`speed` is a guess); the heights are guesses.
2. Parts:
   - `frame` base plate 360 x 15 x 500, four posts 30 x 30, bridge plate 260 x 15 x 120 with two flanged linear bushings and a push-rod bushing, sensor plate on the bridge (fixed).
   - `cam` risers and two pillow-block bearings (fixed); shaft, cam disc and hub, both jaw coupling hubs and the spider, home flag (turn, joint `shaft`); lift plate, white pallet plate, two guide shafts with end stops, push rod, fork with the cam roller, switch dog (slide, joint `lift`).
   - `drive` motor bracket with gussets, gearhead, motor, fan cover, terminal box and cable (fixed).
   - `sensors` up and down proximity switches, slot sensor on its post (fixed).
   - part `tray`: one orange tray on the pallet plate (moves with the lift).
3. Motion path:
   - motor contactor `cam.run` → shaft (driver `coils`, `valve: none`, 120 degrees/s) → joint `shaft` (revolute about z at 115 mm height, `wrap`).
   - shaft → lift (coupling `table`, period 360, 27 points) → joint `lift` (prismatic, up, 0 to 40 mm).
   - the table is the rise of the roller centre over the cam: the roller centre path is the outline of the cam grown by the roller radius (base circle 50 + 15, nose 25 + 15 at 65 mm, joined by the two common tangents); for each shaft angle the lift is the distance from the shaft centre to that outline straight up, minus 65 mm. At 0 degrees the nose points down, so the follower rests on the base circle.
4. Signals:
   - `cam.run` out bit: motor on.
   - `cam.cam_home` in bit: flag in the slot sensor (shaft at 0 degrees).
   - `cam.up_sw` in bit: dog at the up switch.
   - `cam.down_sw` in bit: dog at the down switch.
5. Timing: no lag; 120 degrees/s, so one turn takes 3.0 s. In the cycle of `sim.json` (`run` on at 100 ms, off at 3090 ms): `cam_home` off at 120 ms; `down_sw` off at 1120 ms (lift above 1 mm at about 122 degrees); `up_sw` on from 1540 ms to 1670 ms (lift above 38 mm, 171 to 189 degrees); `down_sw` on at 2090 ms; `cam_home` on at 3090 ms and the shaft stops at 358.8 degrees.
6. Limits and sensors:
   - lift 0 to 40 mm: dwell low from 0 to 112 degrees and from 248 to 360 degrees, rise from 112 to 180 degrees, fall from 180 to 248 degrees, top at 180 degrees (no top dwell: the pear cam has none).
   - `cam_home` at 0 degrees, window 1.5 degrees; `up_sw` at 40 mm, window 2 mm; `down_sw` at 0 mm, window 1 mm.
   - roller centre 180 mm above the floor at lift 0; lift plate bottom 315 mm at lift 0.
7. Numbers:
   | name | value | unit | tag | source |
   |---|---|---|---|---|
   | base circle | 100 | mm | user | words of the person |
   | nose circle, centre distance | 50, 65 | mm | user | words of the person |
   | roller | 30 | mm | user | words of the person |
   | lift | 40 | mm | user | words of the person |
   | lift table | 27 points | mm | ref | computed from the cam and roller sizes above |
   | shaft speed | 120 | degree/s | guess | 20 rpm gearmotor output |
   | shaft height | 115 | mm | guess | |
   | guide shafts, bushings | 16, flange 48 | mm | user, guess | |
   | windows | 1.5 degree, 2 mm, 1 mm | - | guess | |
   | bearing, riser, post, bridge sizes | see parts | mm | guess | |
   | tray | 70 x 70 x 35 | mm | guess | |
8. Not modelled: the real cam curve between the 6-degree table points (linear between them), follower lift-off at speed, the run-down of the motor after `run` goes off, the coupling and gearhead backlash, the motor and gearhead turning (drawn fixed; the coupling hubs turn with the shaft), the conveyor that brings the tray (the tray stays on the plate).
