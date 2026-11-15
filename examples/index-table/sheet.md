# Principle sheet: eight-station index table

Words of the person: "A round index table with 8 stations. Each pulse on its run input turns it one station. It has a home sensor and an in-position sensor. Make a twin model."

1. Machine: a rotary index table (indexer). Evidence: the words of the person. Certainty: sure.
2. Parts: base (fixed), table plate with 8 nests (turns).
3. Motion path: run pulse (driver `step`, one station for each pulse) -> table (joint `revolute`, `wrap`, on the top face). Map: kind `index_table`, preset `index_table`.
4. Signals: `table.run` out bit (one pulse = one station); `table.home` in bit (station 1 at the start place); `table.in_pos` in bit (any station in place).
5. Timing: lag 50 ms, one station in 600 ms.
6. Limits and sensors: no end (the table turns round); `home` at 0 degrees, `in_pos` every 45 degrees, window 1 degree.
7. Numbers: stations 8 (`user`); move 600 ms (`guess`); lag 50 ms (`guess`); base 400 x 150 x 400 mm (default of the kind); plate diameter 360 mm (`guess`, skin only).
8. Not modelled: the cam profile of the indexer (the plant turns at constant speed), the lock pin, the drive motor, parts on the nests.
