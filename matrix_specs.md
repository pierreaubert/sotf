Channel name

- Currently channels are named ch0, ch1, ch2 etc;
 - Use the MeterGroupSpec defition to name them properly (from @speaker_config.rs).
 - By default that's 2_0
 - If a plugin before the mixer increased the channel count, that plugin should propagate the MeterGroupSpec to plugins on the right of it in the rack. Note that other plugins can change the channel count.

Mute, Solo, Dim

- Use the MeterGroup definition to add mute, dim, solo buttons.
- visually add a new colum to the matrix with a row per group see bellow

out\in | L | R | C |LFE| SR| SL|
+------+---+---+---+---+---+---+--------+
    L  |   |    |    |    |    |        |
+------+---+---+---+---+---+---+  M|S|D |
	R  |   |    |    |    |    |        |
+------+---+---+---+---+---+---+--------+
	C  |   |    |    |    |    |  M|S|D |
+------+---+---+---+---+---+---+--------+
	LFE|   |    |    |    |    |  M|S|D |
+------+---+---+---+---+---+---+--------+
	SR |   |    |    |    |    |        |
+------+---+---+---+---+---+---+  M|S|D |
	SL |   |    |    |    |    |        |
+------+---+---+---+---+---+---+--------+

- Button M will mute the group (toggle behaviour)
- Button D will dim the group (-20dB, toggle behaviour)
- Button S will solo the group (toggle behaviour)
