# UI design for audio plugins

First we want a consistent look for all the plugins that support theming, midi interfaces, keyboard shortcuts and mouse interactions.

## General guidelines

We optimise for space and consistency. We want to have a consistent look and feel.
in general plugins will optimise to fit in 1 row with a menu at the top.
The top menu is for general parameters and is always visible.

### Controls

We support 3 kinds of control:
- slider
- potentiometer
- buttons

They can be combined together:
- 1 potentiometer + 1 button
- group of (N potentiometer + 1 button)
- group of (N buttons+1 slider+ P potentiometers)

The overall interface should be compact and / or could match a midi controller.

### midi controller

We support 2 midi controllers but we will add more in the future. They are define by a grid where elements are either potentiometer, sliders, buttons.
We also have a map of midi control (see soft-audio-midi).

For ex: Allen&Health xone k3 looks like this

+----------------------------------------------------------+
|     pot     |      pot     |      pot     |      pot     |
|    button   |    button    |    button    |    button    |
|     pot     |      pot     |      pot     |      pot     |
|    button   |    button    |    button    |    button    |
|     pot     |      pot     |      pot     |      pot     |
|    button   |    button    |    button    |    button    |
|     pot     |      pot     |      pot     |      pot     |
|    button   |    button    |    button    |    button    |
+----------------------------------------------------------+
|             |              |              |              |
|     slider  |      slider  |      slider  |      slider  |
|             |              |              |              |
+----------------------------------------------------------+
|    button   |    button    |    button    |    button    |
|    button   |    button    |    button    |    button    |
|    button   |    button    |    button    |    button    |
|    button   |    button    |    button    |    button    |
|    button   |      pot     |      pot     |    button    |
+----------------------------------------------------------+

For ex: Launchcontrol XL looks like this

+--------+--------------------------------------------------------------------------------------+
| screen |   but   |    but   |    but   |    but   |    but   |    but   |    but   |    but   |
|  but   |   but   |    but   |    but   |    but   |    but   |    but   |    but   |    but   |
|  but   |   but   |    but   |    but   |    but   |    but   |    but   |    but   |    but   |
+--------+--------------------------------------------------------------------------------------+
| stop   |         |          |          |          |          |          |          |          |
| play   |  slider |   slider |   slider |   slider |   slider |   slider |   slider |   slider |
| shift  |         |          |          |          |          |          |          |          |
| mode   |         |          |          |          |          |          |          |          |
+--------+--------------------------------------------------------------------------------------+
|  but   |   but   |    but   |    but   |    but   |    but   |    but   |    but   |    but   |
|  but   |   but   |    but   |    but   |    but   |    but   |    but   |    but   |    but   |
+--------+--------------------------------------------------------------------------------------+


### Keyboard shortcuts and mouse control

For each keyboard shortcut we have a mouse control and vice-versa

## Plugins

### Rack

- new features: save/restore a configuration. that's already done either in the library or the TUI player. If in the TUI player, move the code to the library and factorize the code between the players.
- new feature: for each plugin in the rack if the user over the top right angle of a plugin, then a cross appears and if he clicks on it then the plugin is removed.
- new feature: some plugin like Binaural or upmixer can only be added once in the rack.
- add the level meter plugin

### Upmixer

we want to have a menu and everything else in 1 row.
- menu -> speaker layout
- main row:
  - stereo levels
  - sliders
    - front direct gain
	- front ambiant gain
	- rear ambiant gain
	- height gain
	- lfe gain
  - potentiometers
    - make lfe cutoff a potentiometer that can change from 20 to 180 hz with a default of 120hz
    - make bandpass a potentiometer that can change from 150 to 350 hz with a default at 250
    - make stereo width potentiometer that can change from 0 to 1 with a default at 0.5
  - potentiometes + button
    - make subharmonic synth a button that can be on/off
	- make subharmonic gain a potentiometer that can change from 0 to 1 with a defaukt of 0.5
  - potentiometes + button
    - make HR Direct a button that can be on/off
	- make HR Sharpen a potentiometer that can change from 0 to 1 with a defaukt of 1.0
  - potentiometes + button
    - make decorelation a button that can be LFO Phase / Velvet Noise (the later being the default)
    - make safety cap a potentiometer that can change from 0 to 3dB with a default at 2dB
 - N channels levels

 ### EQ

 If we have enough space we will put everything in 1 row if not 2 rows

 [1|2|3|4] is a button set [+] add an eq, default is 3 eqs

vertical mode

 +------------------+
 |                  |
 |      graph       |
 |                  |
 |                  |
 +------------------+
 +------------------+
 | [1|2|3|4]     [+]|
 |                  |
 | pot1 pot2 pot3   |
 |                  |
 +------------------+

horizontal mode

 +------------------+------------------+
 |                  | [1|2|3|4]     [+]|
 |      graph       |                  |
 |                  | pot1 pot2 pot3   |
 |                  |                  |
 +------------------+------------------+


### Other plugins

later on




+---------------+
| ICON | Title  |
|      | Number |
+---------------+


+-------------------------------------------+
|                                           |
|                 Library                   |
|                                           |
+===========================================+
| queue  || playing   ||        LUFS        |
|        ||           ||                    |
|        ||           ||====================+
|        ||           ||        Levels      |
|        ||           ||                    |
+-------------------------------------------+


+-------------------------------------------+
|                                           |
|                 Library                   |
|                                           |
+===========================================+
| queue  || playing   || LUFS || Levels     |
|        ||           ||      ||            |
|        ||           ||      ||            |
+-------------------------------------------+


