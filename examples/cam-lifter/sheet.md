# Principle sheet: cam lifter

This is Example 2 of `skills/twin-modeling/SKILL.md`, run through the server as it is.

1. Machine: a motor turns a disc cam. A roller follower lifts a plate by the cam profile. Certainty: sure.
2. Parts: frame (fixed), shaft with cam (turns), lift plate with follower (slides up and down).
3. Motion path: motor contactor (driver `coils`, `valve: none`); shaft (joint `revolute`, `wrap`); lift (coupling `table`, period 360); lift plate (joint `prismatic`, up).
4. Signals: `cam.run` out bit; `cam.cam_home` in bit; `cam.up_sw` in bit; `cam.down_sw` in bit.
5. Timing: no lag; 100 degrees/s, so one turn takes 3.6 s.
6. Limits and sensors: lift 0 to 40 mm; `cam_home` at 0 degrees, window 1.5; `up_sw` at 40 mm, window 1; `down_sw` at 0 mm, window 1.
7. Numbers: profile 0 to 60 degrees at 0 mm, 60 to 150 rise to 40 mm, 150 to 210 at 40 mm, 210 to 300 fall, 300 to 360 at 0 mm (`guess`; a cam drawing gives `label`); speed (`guess`).
8. Not modelled: the real curve between the table points, follower lift-off, the run-down of the motor.
