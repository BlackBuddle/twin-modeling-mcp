# Principle sheet: pneumatic pick and place

Words of the person: "A pick-and-place unit: a horizontal air cylinder with 200 mm stroke carries a vertical air cylinder with 100 mm stroke. A suction cup sits at the end of the vertical rod. Both cylinders have two air tubes and a switch at each end. Make a twin model."

1. Machine: a two-axis pneumatic pick-and-place unit. Evidence: the words of the person. Certainty: sure (for the kind). Questions asked: none (bore sizes and times are guesses, see part 7).
2. Parts:
   - `stand`: column and foot plate, fixed.
   - `x_cyl`: horizontal double-acting cylinder on the stand, its rod moves.
   - `z_cyl`: vertical double-acting cylinder on the rod end of `x_cyl`, it moves with that rod; its own rod moves down.
   - `pad`: suction cup on the rod end of `z_cyl`, moves with that rod.
3. Motion path:
   - Valve coils `x_cyl.coil_fwd` / `coil_back` -> `x_cyl` rod (kind `cylinder_double`, 5/2 double valve) -> carries `z_cyl` (`on: x_cyl.out`).
   - Valve coils `z_cyl.coil_fwd` / `coil_back` -> `z_cyl` rod, pointing down (`pose.rot` z -90) -> carries `pad` (`on: z_cyl.out`).
   - Vacuum valve `pad.coil` -> suction at the cup (kind `vacuum_pad`).
4. Signals (pins; direction seen from the controller):
   - `x_cyl.coil_fwd`, `x_cyl.coil_back`: out, bit, valve coils of the horizontal cylinder.
   - `x_cyl.sensor_back`, `x_cyl.sensor_fwd`: in, bit, end switches.
   - `z_cyl.coil_fwd` (down), `z_cyl.coil_back` (up): out, bit.
   - `z_cyl.sensor_back` (up), `z_cyl.sensor_fwd` (down): in, bit.
   - `pad.coil`: out, bit, vacuum on.
5. Timing: `x_cyl` lag 30 ms, forward 500 ms, back 450 ms; `z_cyl` lag 30 ms, down 300 ms, up 280 ms.
6. Limits and sensors: `x_cyl` 0 to 200 mm, `z_cyl` 0 to 100 mm; each end switch is on within 2 mm of its end (default window).
7. Numbers:
   - Stroke of `x_cyl`: 200 mm, `user`. Stroke of `z_cyl`: 100 mm, `user`.
   - Bore of `x_cyl`: 25 mm, `guess`. Bore of `z_cyl`: 16 mm, `guess`.
   - Height of the `x_cyl` axis: 330 mm above the floor, `guess`. Stand size, `guess`.
   - All times of part 5: `guess`.
8. Not modelled: the vacuum level and a vacuum switch, the part on the cup (no workpiece in this model), air pressure, cushions, the speed curve of a cylinder (the plant moves at constant speed after a lag).
