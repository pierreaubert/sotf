# AutoEQ interface implementation

Next steps are complicated, make a detailed plan.

In sotf-audio-player/app-gpui/autoeq we want to add the missing features:
1. spinorama speaker optimisation
2. data acquisition via recording
3. room eq optimisation

In sotf-audio-player/app-gpui/screens/settings:
we want to add the corresponding UI for each:
1. spinorama speaker optimisation
2. data acquisition via recording
3. room eq optimisation

Important Note: All the UI is done with gpui-ui-kit. If features are missing

## Plots

We have a lot of plots to do and they will be implemented with gpui-px (not gpui-d3rs).

Important Note: If there are missing features in gpui-px they will be implemented either in gpui-px or in gpui-d3rs if low level details). The goal is to keep all the plotting issues in the plot library. Use the GPU accelerated graphs by default.

## Spinorama speaker optimisation

This is similar to headphone optimisation except that you get the data from spinorama.org (or cached data).
The optimisation parameters and EQ configuration is the same except the loss functions are the speaker ones (to target and score).

The graphs are also different with the spinorama plots with and without eq on top. We also want to see the impact of the EQ on the
various curves and impact on the tonal balances. All the plots are define in autoeq/src/plot but they are for plotly and not for our
gpui-px plot

## Data acquisition

it used to be implemented in sotf-ui-frontend/modules/data-acquisition-step.ts
we will change a few things: we use an accordeon for steps.
the results of the data acquisition is stored in a directory.


## Room EQ

it used to be implemented in sotf-ui-frontend/modules/roomeq-manager.ts and roomeq_wizard.ts
we will change a few things: we use an accordeon for steps.
it implements autoeq/bin/roomeq with a UI interface similar to what roomeq is doing.

The result will be stored as a graph plugin with multiple eqs, delays, gains and mixers.

