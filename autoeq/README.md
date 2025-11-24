<!-- markdownlint-disable-file MD013 -->

# AutoEQ : an automatic eq for your speaker or headset

## Introduction

The software can find the best EQ for you based on your measurements. There are extensive options to configure the optimiser via the command line.

**Note:** A graphical desktop application is available in a separate repository: [autoeq-app](https://github.com/pierreaubert/autoeq-app)

## Setting up a build developement

### Rust

Install [rustup](https://rustup.rs/) first.

If you already have cargo / rustup:

```shell
cargo install just
```

### Packages

#### MacOS

Install [brew](https://brew.sh/) first.

## Using just

```shell
just
```

will give you the list of possible commands:

- Build everything:

```shell
just prod
```

- Build the demos:

```shell
just demo
```

## Building cross platform

See the [BUILD.md](./BUILD.md) file for details.

### GitHub Actions (Automated Builds)

The repository includes a GitHub Actions workflow (`.github/workflows/build.yml`) that automatically builds binaries for all platforms on:

- Push to main/master branch
- Pull requests
- Git tags (creates releases)

### Testing Binaries

```bash
just test
```

### Development Setup

For testing and development, you need to set the `AUTOEQ_DIR` environment variable to point to your project root directory:

```bash
# Set the environment variable
export AUTOEQ_DIR=/path/to/your/autoeq/project

# Now you can run tests
cargo test --workspace --release
```

This environment variable is used by the test infrastructure to determine where to write CSV trace files and other generated data.

## Using the optimiser

There are a lot of options, so we will go after them one by one. You can either provide your own data or use spinorama.org data.

Let's select one speaker reviewed on audiosciencereview.com:

```shell
cargo run --bin autoeq --release -- --speaker="MAG Theatron S6" --version asr --measurement CEA2034 --algo cobyla
```

You need to specify the name of the speaker, the version of the measurement and the type of measurement; we also provided one optimisation algorithm which is fast and has ok performance.

### How do I find the list of speakers, the possible version or measurements?

1. `curl http://api.spinorama.org/v1/speakers` returns the list of speakers.
2. for a given speaker `curl http://api.spinorama.org/v1/speakers/{speaker}/versions` returns the list of versions.
3. for a given speaker and a given version `curl http://api.spinorama.org/v1/speakers/{speaker}/versions/{version}/measurements` returns the list of possible measurements for this speaker / version pair.
4. How do I find the list of algorithm?

```shell
cargo run --bin autoeq --release -- --help
```

### Parameters: --min-q --max-q

The minimum and maximum Q of the filter.

When Q increases, then the filter is sharper and can match the curve more precisely. At the same time, the curve reprepresenting the speaker or the headset has limited precision. When frequency increases, the curve is less and less relevant since it would change a lot with tiny movement of your head. Current default is 6 which is already high. After 2k Hz, the default maximum is 3.

### Parameters: --min-db --max-db

The minimum and maximum amplitude of the filter.

When the gain increases, you can compensate larger errors but sensitivity and thus efficiency of your speaker will decrease. The default is 3dB. Depending on your application, you can increase or decrease it. The filters are not limited on the negative side.

You can increase the minimum if you do not want small filters which are likely inaudible. Default is 1 dB.

### Parameters: --min-freq --max-freq

The minimum and maximum frequency of your filters (the center frequency).

At lower frequency, below 300-500Hz, the room dominate the response. If you have anechoic measurements, then fine but you will still need to modify the eq to take the room into account.

At high frequency, a low Q make sense but a sharp one does not. You may want to do broad corrections but likely not small ones. A classical example, is that some speakers are designed to be behind a screen and have a boost at high frequency to compensate for it. If you do not have a screen, then removing the boost with eq is fine.

Your mileage may vary. A good strategy is to listen, remove filters that do not have a clear positive impact.

### Parameter: -n

The number of IIR filters.

It may depend on your hardware and how many PEQ it can handle or if in software, then use a sensible number, 5-7 usually works well. If you have a subwoofer, it will need his own EQ.

### Parameter: --smooth --smooth-n

Do we smooth the target curve and if so how much?

The curve you want to optimise is usally very noisy from the measurement. You can smooth it and usually the optimisation algorithm performs better.
The second parameter control how much you want to smooth in terms of octave. A lower value means more smoothing. 3 means smooth over 1/3 octave.

It is activated by default.

### Parameter: --loss

Specify a loss function.

Currently autoEQ support 3 kind of loss functions:

1. `flat`: You want a smooth curve: it will make your ON or LW very flat.
2. `score`: You want to optimise the harman/olive score: it will boost the bass and flatten the PIR.
3. `mixed`: A mixed mode of 1. and 2.

The first one is good for near field listening. The second one is likely good for a medium/far listening distance typical of a home.

### Parameter: --algo

Optimising the above loss functions is not easy. The functions are not convex (and not quasi convex) and a global optimisation function is required to find the best solution.

AutoEQ use the algorithms from a few libraries which provides global and local algorithms.

Global and supporting constraints:

- autoeq:DE
- ISRES
- AGS
- ORIGDIRECT

Global with support for lower and upper bounds

- PSO
- STOGO
- GMLSL

Local optimisation without a derivative:

- cobyla
- neldermead

A few constraits are necessary to contol the behaviour of the various algorithms. If the algorithms support the constraints, then we use that. If the algorithm does not support the constraints, then we use either multiple objective functions or we add the constraints as a penalty to the function you want to optimise.

### Advanced Differential Evolution Parameters

When using the `autoeq:de` algorithm, you can fine-tune the Differential Evolution optimizer with additional parameters:

#### Parameter: --strategy

Select the DE mutation strategy (default: `currenttobest1bin`):

**Classic Strategies:**

- `best1bin`: Use best individual + 1 random difference (fast convergence)
- `rand1bin`: Use random individual + 1 random difference (good diversity)
- `currenttobest1bin`: Blend current with best + random difference (**recommended**)
- `best2bin`, `rand2bin`: Use 2 random differences (more exploration)

**Adaptive Strategies (experimental):**

- `adaptivebin`: Self-adaptive mutation with top-w% selection
- `adaptiveexp`: Adaptive strategy with exponential crossover

Example:

```bash
cargo run --bin autoeq --release -- --algo autoeq:de --strategy rand1bin --speaker="KEF R3" --version asr --measurement CEA2034
```

#### Parameter: --strategy-list

Display all available DE strategies with descriptions:

```bash
cargo run --bin autoeq --release -- --strategy-list
```

#### Parameters: --tolerance and --atolerance

Control convergence criteria for the DE optimizer:

- `--tolerance`: Relative tolerance (default: 0.001)
- `--atolerance`: Absolute tolerance (default: 0.0001)

Lower values = stricter convergence, higher values = faster but less precise optimization.

#### Parameter: --recombination

Recombination probability (0.0 to 1.0, default: 0.9):
Higher values increase information exchange between population members.

#### Parameters: --adaptive-weight-f and --adaptive-weight-cr

For adaptive strategies only:

- `--adaptive-weight-f`: Adaptive weight for mutation factor F (0.0 to 1.0, default: 0.9)
- `--adaptive-weight-cr`: Adaptive weight for crossover rate CR (0.0 to 1.0, default: 0.9)

Example with adaptive strategy:

```bash
cargo run --bin autoeq --release -- --algo autoeq:de --strategy adaptivebin \
  --adaptive-weight-f 0.8 --adaptive-weight-cr 0.7 \
  --speaker="KEF R3" --version asr --measurement CEA2034
```

### Parameter: --refine

If you have use a global optimiser they are good at exploring the search space but they are slow to converge. You should stop them early and finish with a local algorithm.

## Improving the optimiser

Finding the correct parameters or the most useful algorithm is not easy. The code below is here to help answer this questions.

### Download test data

```shell
cargo run --bin download --release
```

This will download all the datas from [spinorana.org](https://spinorama.org) and usually takes around 3-5 minutes. It is used to benchmark the algorithm.

### Run the benchmark

```shell
cargo run --bin benchmark --release -- --algo cobyla
```

It takes a few minutes on a Mac Mini M4 to get all the eq computed. You can use the same parameters as for autoEQ.
The benchmark will generate a csv file with results for each speaker than you can load in your favorite excel clone.

### Improving the performance

A list of ideas:

- the global algorithm are all very sensitive to upper/lower bounds. Tight bounds means less space to look into and weak bounds yield random results or take a very long time. Find a good compromise.
- The `autoeq:de` algorithm now supports advanced adaptive strategies based on recent research, which may perform better than traditional methods.
- results are highly unpredictable: sometimes you need to optimise the low frequency, sometime the midrange etc. It make it hard to reason about the problem.
- test other strategies:
  - start with 3 iirs, optimise, add 3.
  - optimise around a lot of small EQs centered on + / - and the merge them.

## Contributions are very welcome

- Open a ticket on [Github](github.com/pierreaubert/autoeq)
- Send a PR :)
