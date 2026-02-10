# roomeq optimsisation

## input data

roomeq get its data from a json file that has various sections:

- speakers: a list of speakers
- optimiser: configuration for the optimiser
- crossovers: a list of crossovers
- group_delay: configuration for group_delay optimisation
- target_curve: configuration for target_curve optimisation

add a new section caller system that allow to configure
- stereo vs homecinema
- how the speakers are mapped (left|right|subwoofers = "key in the speaker section")
- how the subwoofers are treated (mso, dba, single)
- which subwoofer is pair with which main speaker

example : a simple 2.1 would looks like

"system": {
	model: stereo,
	speakers: {
		"L": "left",
		"R": "right",
		"LFE": "sub0",
	},
	subwoofers: {
        "config": "single",
		"sub0: "L",
	},
}

## processing

We have for each configuration stereo and home cinema 3 possible modes.

1. IIR only
2. FIR only
3. Mixed mode with both IIR and FIR

# Algorithm

## single speaker v.s. group

When we have a group, we optimise the group first. The result of that optimisation become a single speaker/measurement.

## single subwoofer v.s. multi-subwoofer

When we have more than one subwoofer in a group:
- if the configuration is MSO, optimise the subwoofer as a group and then the result is a single subwoofer.
- if the configuration is DBA, optimise the subwoofer as a group and then the result is a single subwoofer.
- if the configuration is Cardiod, optimise the subwoofer as a group and then the result is a single subwoofer.

## 2.0: stereo case without subwoofer

1. Find the average SPL of the left and right speaker from 100hz to 2kHz
2. Normalize down the highest one such that it match the SPL of the lowest one. (Warn user if the difference is too large (lets stay more than 10dB)
3. Find the optimal EQ for left and right

## 2.1: stereo case with 1 subwoofer

1. Find the average SPL of the left and right speaker from min_freq of the crossover to 2kHz
2. Find the average SPL of the LFE from 20Hz up to the max_freq of the crossover (20Hz + 2kHz)
3. Normalize down the highest ones such that they match the SPL of the lowest one. (Warn user if the difference is too large (lets stay more than 10dB)
4. Find the optimal EQ for L and R with a min_freq that match the min_freq-20Hz of the crossover zone.
5. Find the optimal crossover for average of L+R with LFE
6. Apply the crossover to all channels we have a new L_post R_post LFE_post
7. Find the optimal EQ for L_post and R_post with a min_freq that match the computed freq of the crossover +20hz
8. Find the optimal EQ for LEF_post with a max_freq that match the computed freq of the crossover -20hz

## 2.2: stereo case with 2 subwoofers

We have 3 options.

1. we attach 1 subwoofer to 1 main: we should have 2 groups in the input file that we solve with the 2.0 algo
2. we do 1 subwoofer first and then optimise for the second one.
 - Apply the 2.1 optimisation first.
 - Treat the result as a 2.0 system
 - Optimise again the previous system with a subwoofer (the second one of course)
 - Total: 2 loops of 2.1
3. we create a cardiod subwoofer via a group of subwoofers and then do a classical 2.1 optimisation

## subwoofers

### cardioid subwoofer

Optimise to maximise cancellation between the subwoofers with a delay, phase switch and an EQ.

### MSO subwoofers

Use the current MSO optimisation

### DBA subwoofers

Use the current DNA optimisation

## speaker group

Use the current algo for a group

## 5.0 and 5.1
