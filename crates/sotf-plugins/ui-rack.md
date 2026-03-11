# Rack

+----------------------------------------------------------------------+--------------------+
| +-------------------+ .....................                          |                    |
| | A    title       X| .                   .                          |  IN    LABEL  OUT  |
| | S     icon        | .        +          .                          |                    |
| | P                 | .                   .                          |  xx       0    yy  |
| +-------------------+ .....................                          |  xx      -6    yy  |
+----------------------------------------------------------------------+  xx     -12    yy  |
|                                                                      |  xx            yy  |
|                                                                      |  xx            yy  |
|                                                                      |  xx            yy  |
|                                                                      |  xx            yy  |
|                                                                      |  xx     -30    yy  |
|                             plugin                                   |  xx            yy  |
|                                                                      |  xx     -60    yy  |
|                                                                      +--------------------+
|                                                                      | Bypass  | AutoGain |
|                                                                      |  Mono   |  M/S     |
|                                                                      |         |          |
+----------------------------------------------------------------------+--------------------+


Description

- each plugin is in the large plugin block
- a box represent each plugin in the rack
- each box has 4 buttons (A is active v.s. bypass, use current icon), S is solo this plugin and mute all the other ones, P is a menu for presets that allows to load and save presets, X allows to remove the plugin (if the plugin is not removable show the locked icon). A, S,P, X are replaced in the real UI by icons.
- We move the input meter to the right box. we have only 1 label column in the middle. On the left we have the input on the right the output. The width of the meter box depends on the number of channel, compute the perfect size and automaticall resize between the plugin and the meter such that the meter are always visible.
- Below the meters, we have 2 rows of buttons. Bypass desactivate the whole chain, autogain is a toggle that activate autogain at the chain level. Mono and M/S are convenience button that do the corresponsing action in the matrix mandatory plugin.

# Upmixer

+----------------------------------------------------------------------------------------+
| UI |                                             | Configuration |  Ouput | Diagnostic |
+----------------------------------------------------------------------------------------+
|
| Channels Gain                        Spatial Control
|
|  Mains Center Surr  Top              Width Spread Bleed Reflect
|
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|    x     y     z     t                 x     y      z     t
|
+----------------------------------------------------------------------------------------+
|
| Configuration row
|
+----------------------------------------------------------------------------------------+

# behavior

The menu allow to configure options. Configuration is also a menu but when triggered it fill
the configuration row with what we want to configure (LFE, Dialogue ...)

## UI Menu

- UI
- Simple
- Controller 1
- Controller 2
- ...

## Output menu

the current menu which is 1 row fown

## Diagnostic menu

The 4 on/off options which are currently inside the plugin

- bypass decorrelation
- bypass transient
- bypass all
- bypass ml detection


## Configuration menu

- LFE&Bass
- Dialogue
- Ambient
- Height
- Decorrelation


## Configuration row

### LFE

+----------------------------------------------------------------------------------------+
|
| LFE & Bass          SubHarmonic ON/OFF
|
| LFE Cut Lfe Gain    Gain Freq Attack Release
|
+----------------------------------------------------------------------------------------+


### Dialogue

+----------------------------------------------------------------------------------------+
|
| Dialogue
|
| Weight Voice low Voice high | Center Variance Coherence
|
+----------------------------------------------------------------------------------------+



# Compressor plugin

+--------------------+-------------------------------------------------+--------------------+
| Setup              | Transfer                                        | Meter              |
+--------------------+-------------------------------------------------| Gain reduction
| Link Ch [on|off]   |                                                 |
|                    |  Dynamic               Timing          Transfer +--------------------+
| SC HPF             |  Threshold Ratio Knee  Attack Release  Curve    | Output
|                    |                                                 | AutoGain off/on
+----------------------------------------------------------------------+--------------------+

# Limiter plugin


# Footer

+------------------------------------------------------------------------------+-------------------------+
| pict |    xx:xx                        transport                       yy:yy |   Menu  Menu     Volume |
|      |    -=-====-=--------------------------------------------------------- |   Tool  Devices         |
+------------------------------------------------------------------------------+-------------------------+
