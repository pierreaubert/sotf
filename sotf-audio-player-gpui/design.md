# GPUI Audio player

we have a prototype of an audio player. we want to make it ergomic with a professional look.
The player will have a menu, a library view, a queue view and a sofisticated configuration view.

## Functionality

The functions are already defined in the sotf-audio-player-crate. All non graphic elements are defined in the audio-player-crate.

## The library view

It will show a list of all the songs in the library presented as a tree.
The user can
- sort by album, author, popularity etc.
- filter by properties (number of track), music category ...
- search by keywords
Actions:
- clicking on an Album allow to expend it, close it and add it to the queue

+-------------------------------------------------------------------------------+
| Sort: [artist] [album] [title] [popularity]  Filter: [2.0] [5.0] [5.1] [7.1]  |
| Search: [...]                                                                 |
+-------------------------------------------------------------------------------+
| A -
|   | Album A2                                                               #3 |
| B -
|   | Album B1                                                              #23 |
|   | Album B2                                                              #17 |
+-------------------------------------------------------------------------------+


+-------------------------------------------------------------------------------+
| Sort: [artist] [album] [title] [popularity]  Filter: [2.0] [5.0] [5.1] [7.1]  |
| Search: [...]                                                                 |
+-------------------------------------------------------------------------------+
| [      ] [      ]  [      ]  [      ]  [      ]
| [ pict ] [ pict ]  [ pict ]  [ pict ]  [ pict ]
| [      ] [      ]  [      ]  [      ]  [      ]
| Title    Title     Title     Title     Title
| Album
| Flac - ReplayGain
|
| [      ] [      ]  [      ]  [      ]  [      ]
| [ pict ] [ pict ]  [ pict ]  [ pict ]  [ pict ]
| [      ] [      ]  [      ]  [      ]  [      ]
| Title    Title     Title     Title     Title
| Album
| Flac - ReplayGain
|
+-------------------------------------------------------------------------------+


## The Queue view

+---------------------------------+-----------------------------+---------------+
| List                            | Art part                    | Loudness      |
|                                 | Song information            +-------------- +
|                                 |                             | Levels        |
|                                 |                             +-------------- +
|                                 |                             | Volume        |
+---------------------------------------------------------------+---------------+

Loudness, levels and volume are similar to the TUI version but instead of characters
they are proper meters. The logic is defined in the TUI crate.

Art and song information are also in the TUI app.

## The menubar

- Config (command-, on macos)
- About (generate a pop with some info: open source code, link to repo on GH)
- Help (popup with all the key shortcuts)
- Quit

## The configuration window

The configuration has a set of tabs.

+-------------------------------------------------------------------------------+
|                     Devices | Plugins | RoomEQ | Others                       |
+-------------------------------------------------------------------------------+
|                                                                               |
|                              content                                          |
|                                                                               |
+-------------------------------------------------------------------------------+

### Device configuration

Should be a list of devices. Choices are exclusive.

[] Device 1 with information
[] Device 2 with information
[] Device 3 with information
[] Device 4 with information

### Plugin Configuration

The host has 2 parts:
1. a top level bar where you can add / remove / change plugins; drag and drop is
also possible to reorder them
2. a window down where you can configure the plugin. Each plugin has is own set
of parameters. See TUI code.

+-------------------------------------------------------------------------------+
|
| +------+  +------+
| | P1   |  | P2   |     +
| |      |  |      |
| +------+  +------+
|
+-------------------------------------------------------------------------------+
| Param 1 val1 val2 ...
| Param 2 slider 0 .. 1
+-------------------------------------------------------------------------------+


### RoomEQ Configuration

will be done later

## The plugins

Each plugin has a configuration and a custom UI.

They will be define one by one later.

### The EQ plugin

Each IIR filter is represented as a curve freq v.s spl.
The user can select an eq by clicking the top of it, he then get a popup
that allow to change the type of IIR and/or configure each value. He can also
move the o and change freq/gain. you can change the Q by shift dragin it.

+------------------------------------------------------------------------------+
|
|             o
|            / \
|           /   \
| ---------------------------------------------------------------------------- |
|
|
|
|
+------------------------------------------------------------------------------+
