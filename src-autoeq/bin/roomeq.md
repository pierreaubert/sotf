# in src-autoeq i want to add a new binary roomeq. It will take a json file that describe a room with

1. a set of measurements per speaker (with the name of the channel and the name of the measurement)
2. some speaker have multiple measurements and we will call them a group
3. potentially a configuration for crossovers
4. potentially a target curve
5. potentially a configuration for the optimiser

# The algorithm will:

1. for each group, compute an optimal cross over (similar to what autoeq is doing) : ouput is a freq/spl/phase file
2. for each speaker and each group, we want to find the optimal eq that optimise for a score (flat or score)

# Example

## 2.0 speaker

measurements: l, r

1. compute l+r
2. compute the average over 100hz-10khz
3. normalized l wrt to l+r (keep the 2 gain values in memory)
4. find the optimsal eq for l
5. find the optimal eq for r
6. ouput the filter chain

l -> gain1 -> eq1
r -> gain1 -> eq2


## 2.1 speaker

measurements: l, r, lfe

1. compute l+r
2. compute the average over 100hz-10khz
3. normalized l wrt to l+r (keep the 2 gain values in memory)
6. compute the average for the subwoofer from 40hz to 80hz
7. normalize the subwoofer wrt to the l+r average
8. compute the optimal crossover and apply it to left, right and lfe
9. compute the optimal eq for the subwoofer+left
10. compute the optimal eq for the subwoofer+right
11. compute the optimal eq for the subwoofer+(l+r)/2
12. ouput the result

l -> gain1 -> keep above crossover freq -> eq1
r -> gain2 -> keep above crossover freq -> eq2
lfe -> gain3 -> keep below crossover freq -> eq3

## 2.0 with crossovers

m1  -\
m2  -- group 1 (left) -> crossover -> eq1 --> level normalisation          -> left channel
m3  -/

m4  -\
m5  -- group 2 (left) -> crossover -> eq2 --> correct level wrt to group 1 -> right channel
m6  -/

here we do similar things except that (m1, m2, m3) and (m4,m5,m6) have 2 crossovers

## 5.1.4

measurements l,r,c,lfe,sl,sr,tfl,tfr,tsl,tsr

same as 2.1 except that the average is with respect to l+r only

# output

the output is the full DSP chain which is a graph of gain, eq, crossovers in a json file


