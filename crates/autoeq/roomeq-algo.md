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

When we have a group, we optimise the group first. The result of that optimisation become a single speaker.

## single subwoofer v.s. multi-subwoofer

When we have more than one subwoofer, if the configuration is MSO or DBA, optimise the subwoofer as a group and then the result is a single subwoofer.

## stereo case without subwoofer

1. Find the average SPL of the left and right speaker from 100hz to 2kHz
2. Normalize down the highest one such that it match the SPL of the lowest one.
3. Find the optimal EQ for left and right

## stereo case with subwoofer

1. Find the average SPL of the left and right speaker from min_freq of the crossover to 2kHz
2. Find the average SPL of the LFE up to he max_freq of the crossover
3. Normalize down the 2 highest ones such that they match the SPL of the lowest one.
4. Find the optimal EQ for L and R with a min_freq that match the min_freq of the crossover zone.
5. Find the optimal crossover for average of L+R with LFE
6. Apply the crossover to all channels we have a new L_post R_post LFE_post
7. Find the optimal EQ for L_post and R_post with a min_freq that match the computed freq of the crossover +20hz
8. Find the optimal EQ for LEF_post with a max_freq that match the computed freq of the crossover -20hz




