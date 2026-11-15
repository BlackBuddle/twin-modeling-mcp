# Principle sheet: eight-station index table

Words of the person: "Our press-in station is an 8-station rotary table on a small profile stand. A cam indexer with a geared motor on its input shaft turns a 520 mm aluminium dial; each start pulse moves it one station and it stops on the cam dwell. The dial carries eight nests; the operator loads a part at the front station, at the back station a guided air cylinder (32 bore, 40 stroke) on a column presses a pin into the part, and two stations later the part is taken out. A small diffuse sensor on the column checks that a part is in the nest under the press. Under the dial rim there are dogs: one for each station for the in-position switch and one long dog for home, both read by M12 proximity switches on a bracket."

1. Machine: an eight-station rotary indexing table (cam indexer with a geared motor, dial with eight nests) with one press-in station. Evidence: the words of the person. Certainty: sure. Questions: the index time and the dwell are not known (`move_ms` is a guess); the mounting heights are guesses.
2. Parts:
   - `stand` profile stand 700 x 700, height 195 to the plate top (fixed).
   - `table` cam indexer housing 240 x 170 x 240 (fixed); output flange, dial 520 x 16, centre cap and the nine dogs under the rim (turn, joint `table`).
   - `nests` eight nests, each a base plate with four white locators (turn with the dial).
   - `drive` right-angle geared motor on the input shaft with its terminal box and cable (fixed; the input shaft turn is not drawn).
   - `column` profile column with foot plate and gussets, the arm with the guide bushings and the cylinder flange, the two-station valve manifold with its tubes, the sensor bracket (fixed).
   - `press` guided press cylinder: body with two auto switches (fixed); rod, rod-end nut, tool plate, two guide rods, punch holder and punch (move, frame `out`).
   - `psens` diffuse photo sensor that looks at the nest under the press (fixed).
   - `refsens` bracket with the in-position and home proximity switches (fixed).
   - parts `blank`: round 40 x 20 parts in the nests (move with the dial).
3. Motion path:
   - controller pulse `table.run` → cam indexer (driver `step`: one pulse turns one station, 360 / 8 = 45 degrees in `move_ms` after `lag_ms`) → dial (joint `table`, revolute about +y, `wrap`). Map: kind `index_table`, preset `index_table`.
   - valve coils `press.coil_fwd`, `press.coil_back` (5/2 double solenoid) → cylinder rod (kind `cylinder_double`, mount `flange_front`, rod down) → tool plate and punch on the frame `out`.
   - parts: points `n0` to `n7` on the dial frame `table.table`, so a part in a nest turns with the dial; the point `press_st` on the stand is the place under the punch.
4. Signals:
   - `table.run` out bit: index start pulse.
   - `table.home` in bit: home dog at the switch (dial at 0 degrees).
   - `table.in_pos` in bit: a station dog at the switch (dial at a station).
   - `press.coil_fwd` out bit: press down; `press.coil_back` out bit: press up.
   - `press.sensor_back` in bit: rod up; `press.sensor_fwd` in bit: rod down.
   - `psens.output` in bit: a part is in the nest under the press.
5. Timing:
   - index: lag 50 ms, move 600 ms for 45 degrees. In the cycle of `sim.json`: pulses at 0, 1700 and 3400 ms; `in_pos` off at 70 ms and on at 640 ms (then 1770 to 2340 ms, 3470 to 4040 ms); `home` off at 70 ms (the dial leaves 0 degrees and is not back at 0 in three steps).
   - press: lag 60 ms, down 250 ms, up 250 ms. `sensor_back` off at 830 ms, `sensor_fwd` on at 1050 ms; after the up command at 1150 ms `sensor_fwd` off at 1230 ms and `sensor_back` on at 1450 ms (the same for the next two cycles, +1700 ms each).
   - part sensor: off at 100 ms (the part leaves the press place), on at 620 ms (the next part arrives), the same in each cycle.
   - parts: at 700, 2400 and 4100 ms the part at the 180-degree station is taken out and a new part is put in the nest at the 0-degree station (`part_events`).
6. Limits and sensors:
   - dial: endless turn, 8 stations of 45 degrees; `home` at 0 degrees, window 1 degree; `in_pos` every 45 degrees, window 1 degree.
   - press: stroke 0 to 40 mm, rod down; rod tip 530 mm above the floor at stroke 0 and 490 mm at full stroke (from `facts`); the punch end is 54 mm below the tip, 2 mm above the part top at full stroke.
   - part sensor: sees a part within 10 mm of `press_st` (point form of `sense`).
   - stations (dial angles at rest): load 0, press 90, unload 180 degrees.
7. Numbers:
   | name | value | unit | tag | source |
   |---|---|---|---|---|
   | stations | 8 | - | user | words of the person |
   | dial diameter, thickness | 520, 16 | mm | user, guess | words of the person; thickness by eye |
   | indexer housing | 240 x 170 x 240 | mm | guess | |
   | index time, lag | 600, 50 | ms | guess | |
   | in-position and home windows | 1 | degree | guess | |
   | cylinder bore, stroke | 32, 40 | mm | user | words of the person |
   | rod out at stroke 0 | 130 | mm | guess | |
   | cylinder lag, down, up | 60, 250, 250 | ms | guess | |
   | nest radius | 200 | mm | guess | |
   | part | round 40 x 20, plastic | mm | guess | |
   | stand | 700 x 700, plate top 195 | mm | guess | |
   | column, arm | 80 x 80 x 455, arm 225 long | mm | guess | |
   | part sensor window | 10 | mm | guess | |
8. Not modelled: the cam curve of the indexer (the dial turns at constant speed between stations), the input shaft and the motor turning, the geared motor brake and overrun, the press force and the pin that the punch puts in (the part does not change), the operator who loads and unloads (the parts appear and go by `part_events`), the air pressure and the tubes, the valve terminal as a device (its LEDs show the coil pins), a guard or a light curtain.
