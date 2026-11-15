# Principle sheet: ball-screw servo axis

Words of the person: "It is a single servo axis that we built on our transfer station. A 200 W servo motor sits on a bracket at the left end and drives a 16 mm ball screw with 10 mm lead through a clamp coupling; the screw has a fixed bearing block at the motor end and a support block at the other end. The carriage plate runs on two 15 mm profile rails with two blocks each and carries a nest with one part. Stroke is 300 mm. There is a slot sensor for home and two over-travel sensors, normally closed, all on one sensor rail with one flag on the carriage, and urethane stops at both ends. A cable chain in a trough goes from the carriage to the base. The PLC sends servo on and a target position in 0.1 mm and reads busy, home and the two limits."

1. Machine: a horizontal single-axis ball-screw actuator built from parts (base plate on two aluminium profiles, two profile rails, ball screw with fixed-side and support-side bearing blocks, clamp coupling, servo motor on a motor bracket, carriage plate on four blocks, cable chain in a trough, slot sensors, hard stops). Evidence: the words of the person. Certainty: sure. Questions: none open; the mount sizes and heights are `guess` (ask for a drawing or one measured height).
2. Parts:
   - `frame` base plate 625 x 15 x 220 on two 40 x 40 profiles, rail risers, rails, hard stops, chain trough with the fixed lower chain run (fixed).
   - `screw` bearing blocks, motor bracket and servo motor (fixed); screw, lock nut and coupling (turn with the motor, joint `motor`); carriage plate, four rail blocks, nut housing with the ball nut, sensor flag (slide, frame `carriage`).
   - `sensors` sensor rail with three slot sensors: over-travel minus, home, over-travel plus (fixed).
   - `chain` moving bend of the cable chain with its upper and lower runs (slides at half the carriage speed, joint `chain`).
   - `load` tooling plate, white nest, orange part, chain mounting bracket and the upper chain run near the carriage (slide with the carriage).
3. Motion path:
   - servo amplifier to the commanded position (driver `follow`, word `target`, 10 counts for each mm, enable `servo_on`, busy `busy`) → carriage (joint `form`).
   - carriage → motor and screw angle (coupling `ratio`, `lead_mm` 10: 36 degrees for each mm) → joint `motor` (revolute about the screw axis, `wrap`).
   - carriage → bend of the cable chain (coupling `ratio`, `k` 0.5) → joint `chain` (prismatic along the axis, 0 to 150 mm): the bend of a chain with one fixed end moves at half the speed of the moving end.
   - This is the motion of the preset `servo_screw` written out, plus the joint `chain` (a preset takes no extra joint) and the motor joint placed on the screw axis.
4. Signals:
   - `screw.servo_on` out bit: servo on (enable).
   - `screw.target` out word: commanded position in 0.1 mm.
   - `screw.busy` in bit: the carriage moves.
   - `screw.home_sw` in bit: home slot sensor sees the flag.
   - `screw.ot_minus` in bit, `active: low` (normally closed): over-travel minus.
   - `screw.ot_plus` in bit, `active: low` (normally closed): over-travel plus.
5. Timing: no lag; constant 250 mm/s stand-in speed, so 300 mm takes 1.2 s. In the cycle of `sim.json`: busy from 310 ms to 1500 ms (0 to 300 mm), 2010 ms to 2600 ms (300 to 150 mm), 3010 ms to 3600 ms (150 to 0 mm); `home_sw` off at 330 ms and on again at 3580 ms; both over-travel inputs stay ON (closed) for the whole cycle.
6. Limits and sensors:
   - carriage 0 to 300 mm (soft range); over-travel switches at -20 mm and 320 mm (window 2 mm); urethane stops touch the carriage plate at -28 mm and 328 mm.
   - `home_sw` at 0 mm, window 6 mm (the flag is 16 mm long; the window is the switching range, not the flag length).
   - sensor places on the sensor rail: -210 mm (over-travel minus), -190 mm (home), 130 mm (over-travel plus), flag 40 mm behind the carriage centre.
   - chain bend from 90 mm to 240 mm (base x); the chain is 450 mm between its ends plus the bend, fixed end at -60 mm, bend radius 28 mm.
7. Numbers:
   | name | value | unit | tag | source |
   |---|---|---|---|---|
   | stroke | 300 | mm | user | words of the person |
   | screw diameter, lead | 16, 10 | mm | user | words of the person |
   | counts for each mm | 10 | count/mm | user | target in 0.1 mm |
   | speed | 250 | mm/s | guess | stand-in, 1500 rpm at 10 mm lead |
   | rail size, rail pitch | 15, 110 | mm | user, guess | words of the person; pitch by eye |
   | rail block | 56 x 34 x 20 | mm | guess | typical 15 mm block |
   | bearing blocks fixed, support | 25 x 43 x 60, 20 x 43 x 60; centre height 25 | mm | guess | typical blocks for a 12 mm journal |
   | motor | flange 60, body 64 + 22 long | mm | guess | typical 200 W servo |
   | coupling | 25 x 27 | mm | guess | |
   | carriage plate | 140 x 12 x 170 | mm | guess | |
   | base plate | 625 x 15 x 220; profiles 40 x 40 x 260 | mm | guess | |
   | home window, over-travel window | 6, 2 | mm | guess | |
   | over-travel places | -20, 320 | mm | guess | |
   | hard stops | -28, 328 | mm | guess | |
   | chain | 15 x 30 links, bend radius 28 | mm | guess | small plastic chain |
   | chain ratio | 0.5 | - | ref | geometry of a chain with one fixed end |
8. Not modelled: acceleration and deceleration (constant speed), following error, home position return of the amplifier, servo alarms and the encoder, the stop behaviour at an over-travel input (the joint cannot pass 0 to 300 mm, so the over-travel and hard-stop positions are never reached in the simulation), the cables inside the chain and the tooling cables, the coupling slip, the part on the nest (drawn only, it never leaves the nest). The chain runs are rigid pieces that overlap; the real chain links bend.
