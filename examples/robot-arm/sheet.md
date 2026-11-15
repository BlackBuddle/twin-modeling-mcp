# Principle sheet: six-axis robot arm

Words of the person: "An articulated robot arm with six axes and a suction tool on the flange. The controller selects a program number and gives a start pulse; the robot reports busy and done. Program 1 is the rest pose, program 2 reaches down to the front, program 3 is the same pose turned 90 degrees. Make a twin model."

1. Machine: a six-axis articulated robot that plays taught poses. Evidence: the words of the person. Certainty: sure for the kind; the link lengths and the poses are guesses.
2. Parts: base (fixed), column (j1), upper arm (j2), forearm (j3), wrist (j4, j5), flange (j6), suction tool (on the flange).
3. Motion path: start pulse + program number (driver `playback`) -> j1 .. j6 (joints `revolute`) -> flange (joint `fixed`) -> tool (device `tool` on `robot.flange`). Map: kind `robot_arm`, preset `robot_6r`.
4. Signals: `robot.start` out bit; `robot.prog` out word (program number); `robot.busy` in bit; `robot.done` in bit.
5. Timing: program 1 in 1500 ms, program 2 in 1200 ms, program 3 in 1500 ms; no lag.
6. Limits and sensors: each joint -180 to 180 degrees; no sensor other than `busy` and `done`.
7. Numbers: base height 300 mm, upper arm 400 mm, forearm 400 mm, wrist 100 mm (all `guess`); poses: program 2 j2 60, j3 -100, j5 40 degrees; program 3 the same with j1 90 degrees (all `guess`; only a teach pendant gives `measured` poses).
8. Not modelled: the real path between poses (the plant moves the joints together at constant speed), speed override, reach and collision checks, the vacuum of the tool.
