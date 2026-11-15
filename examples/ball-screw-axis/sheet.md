# Principle sheet: ball-screw servo axis

Words of the person: "A horizontal linear axis: a servo motor at one end turns a ball screw with 16 mm lead, the nut moves a carriage over 300 mm. The controller sends a target position; the axis reports busy and a home switch. Make a twin model."

1. Machine: a linear servo axis with a ball screw. Evidence: the words of the person. Certainty: sure.
2. Parts: body (fixed), screw with motor (turns), carriage (slides).
3. Motion path: servo amplifier, to the commanded position (driver `follow`); carriage (joint `form`); motor angle by the screw (coupling `ratio`, `lead_mm` 16). Map: kind `servo_axis`, preset `servo_screw`.
4. Signals: `screw.servo_on` out bit (enable); `screw.target` out word (position in 0.1 mm); `screw.busy` in bit; `screw.home_sw` in bit.
5. Timing: no lag; 100 mm/s (stand-in speed).
6. Limits and sensors: carriage 0 to 300 mm; `home_sw` at 0 mm, window 1.5 mm.
7. Numbers: stroke 300 mm (`guess`); lead 16 mm (`label`, the words of the person); speed 100 mm/s (`guess`); 10 counts for each mm (`guess`); body 500 x 60 x 80 mm (default of the kind).
8. Not modelled: acceleration, following error, home position return, servo alarms, the encoder.
