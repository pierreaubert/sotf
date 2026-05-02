<!-- markdownlint-disable-file MD013 -->

# References

Foundational papers, cookbooks, and standards behind the filter designs in `math-iir-fir`. The "Source-file index" below maps each `src/` module to the references it implements.

## Source-file index

| Source file | References used |
|---|---|
| `iir/biquad.rs` (RBJ recipes) | [RBJ cookbook](#biquad-cookbook-rbj) |
| `iir/biquad.rs` (`LowshelfOrf` / `HighshelfOrf`) | [Orfanidis prescribed-Nyquist-gain shelves](#orfanidis-shelves-with-prescribed-nyquist-gain) |
| `iir/biquad.rs` (`PeakMatched`) | [Vicanek matched analog peak](#vicanek-matched-analog-biquads) |
| `iir/peq.rs`, `lr4_crossover.rs`, `lr8_crossover.rs` | [Butterworth / Linkwitz-Riley](#butterworth--linkwitz-riley-crossovers) |
| `iir/warped_biquad.rs` | [Warped digital filters](#warped-digital-filters-frequency-warped--laguerre) (esp. Smith & Abel for `bark_lambda`) |
| `iir/kautz.rs` | [Kautz filters](#kautz-filters-generalized-orthonormal-iir) |
| `svf.rs` | [ZDF / TPT SVF](#state-variable-filter--zero-delay-feedback--topology-preserving-transform) |
| `fir.rs` (windows) | [Harris / Kaiser / Rabiner & Gold](#fir-design--windowed-sinc-and-frequency-sampling) |
| `fir_design.rs` (Kirkeby + pre-ringing) | [Kirkeby inverse filtering](#kirkeby-inverse-filter-correction-regularized-inversion), [Brännmark & Sternad pre-ringing](#pre-ringing-suppression) |
| `fir_crossover.rs` | [Harris / Kaiser](#fir-design--windowed-sinc-and-frequency-sampling) |
| `filtfilt.rs` | [Gustafsson zero-phase filtering](#zero-phase-forwardbackward-filtering-filtfilt) |
| `phase_smooth.rs` | [Phase unwrapping & group delay](#phase-unwrapping-and-group-delay) |
| `iir.rs::loudness_compensation` | [ITU-R BS.1770 / IEC 61672](#loudness-weighting-peq-loudness-compensation) |
| `denormals.rs` | [Bencina / Intel optimization manual](#denormal-handling) |

## Biquad cookbook (RBJ)

The canonical recipe set for the Peak / Lowshelf / Highshelf / Lowpass / Highpass / Bandpass / Notch / Allpass biquads in `iir.rs`.

```bibtex
@misc{rbj_cookbook,
  author       = {Bristow-Johnson, Robert},
  title        = {{Cookbook formulae for audio EQ biquad filter coefficients}},
  howpublished = {\url{https://www.w3.org/TR/audio-eq-cookbook/}},
  note         = {Originally circulated on \texttt{music-dsp}, ca. 1995; W3C Audio EQ Cookbook is the maintained reference}
}
```

## Orfanidis shelves with prescribed Nyquist gain

Used by `BiquadFilterType::LowshelfOrf` / `HighshelfOrf` in `iir/biquad.rs`. Standard RBJ shelves drift from the prescribed shelf gain near Nyquist; Orfanidis derives a closed-form correction that pins the Nyquist response.

```bibtex
@article{orfanidis1997digital,
  author  = {Orfanidis, Sophocles J.},
  title   = {Digital Parametric Equalizer Design with Prescribed {Nyquist}-Frequency Gain},
  journal = {Journal of the Audio Engineering Society},
  volume  = {45},
  number  = {6},
  pages   = {444--455},
  year    = {1997},
  url     = {https://www.aes.org/e-lib/browse.cfm?elib=10010}
}

@book{orfanidis1996introduction,
  author    = {Orfanidis, Sophocles J.},
  title     = {Introduction to Signal Processing},
  publisher = {Prentice Hall},
  year      = {1996},
  isbn      = {978-0132091725},
  note      = {Free reissue: \url{https://www.ece.rutgers.edu/~orfanidi/intro2sp/}}
}
```

## Vicanek matched analog biquads

Used by `BiquadFilterType::PeakMatched` in `iir/biquad.rs`. Matches the digital biquad's magnitude response to the analog prototype across the full band (not just at the center frequency), avoiding the high-frequency droop of standard RBJ peak filters.

```bibtex
@misc{vicanek2016matched,
  author       = {Vicanek, Martin},
  title        = {{Matched Second Order Digital Filters}},
  year         = {2016},
  howpublished = {\url{https://vicanek.de/articles/BiquadFits.pdf}},
  note         = {Also: \emph{Matched One-Pole Digital Shelving Filters} (2019) — companion paper for shelf variants}
}
```

## Butterworth / Linkwitz-Riley crossovers

```bibtex
@article{butterworth1930theory,
  author  = {Butterworth, Stephen},
  title   = {On the Theory of Filter Amplifiers},
  journal = {Experimental Wireless and the Wireless Engineer},
  volume  = {7},
  pages   = {536--541},
  year    = {1930}
}

@article{linkwitz1976active,
  author  = {Linkwitz, Siegfried H.},
  title   = {Active Crossover Networks for Noncoincident Drivers},
  journal = {Journal of the Audio Engineering Society},
  volume  = {24},
  number  = {1},
  pages   = {2--8},
  year    = {1976}
}

@article{riley1983active,
  author  = {Riley, Russ and Linkwitz, Siegfried H.},
  title   = {A Subjective and Objective Comparison of Active Crossover Networks for Loudspeakers},
  journal = {Journal of the Audio Engineering Society},
  volume  = {31},
  number  = {1/2},
  pages   = {2--12},
  year    = {1983}
}
```

## Bilinear transform / digital filter design

```bibtex
@book{oppenheim2010discrete,
  author    = {Oppenheim, Alan V. and Schafer, Ronald W.},
  title     = {Discrete-Time Signal Processing},
  edition   = {3},
  publisher = {Prentice Hall},
  year      = {2010},
  isbn      = {978-0131988422}
}

@book{smith2007introduction,
  author    = {Smith, Julius O.},
  title     = {Introduction to Digital Filters with Audio Applications},
  publisher = {W3K Publishing},
  year      = {2007},
  url       = {https://ccrma.stanford.edu/~jos/filters/}
}
```

## State Variable Filter — Zero-Delay Feedback / Topology-Preserving Transform

The ZDF / TPT SVF in `svf.rs`. The lineage runs from Chamberlin's classic two-integrator-loop digital SVF, through Stilson & Smith's analysis of the analog topology, to Zavalishin's TPT formulation (which collapses the implicit zero-delay feedback algebraically) and Simper's trapezoidal-integrator state-space derivation used by most modern VA implementations.

```bibtex
@book{chamberlin1985musical,
  author    = {Chamberlin, Hal},
  title     = {Musical Applications of Microprocessors},
  edition   = {2},
  publisher = {Hayden Books},
  year      = {1985},
  isbn      = {978-0810457683},
  note      = {Original digital state-variable filter (two-integrator loop) — Chapter 4}
}

@inproceedings{stilson1996analyzing,
  author    = {Stilson, Tim and Smith, Julius O.},
  title     = {Analyzing the {Moog VCF} with Considerations for Digital Implementation},
  booktitle = {Proceedings of the International Computer Music Conference (ICMC)},
  year      = {1996},
  url       = {https://ccrma.stanford.edu/~stilti/papers/moogvcf.pdf}
}

@book{zavalishin2018art,
  author    = {Zavalishin, Vadim},
  title     = {The Art of {VA} Filter Design},
  edition   = {2.1.2},
  year      = {2020},
  url       = {https://www.native-instruments.com/fileadmin/ni_media/downloads/pdf/VAFilterDesign_2.1.2.pdf},
  note      = {Original 2012 first edition; canonical reference for zero-delay-feedback (ZDF) and topology-preserving transforms (TPT). Chapters 3--5 cover the trapezoidal-integrator SVF used in svf.rs}
}

@misc{simper2013linear,
  author       = {Simper, Andrew},
  title        = {{Linear Trapezoidal Integrated State Variable Filter}},
  howpublished = {Cytomic technical paper, \url{https://cytomic.com/files/dsp/SvfLinearTrapezoidal.pdf}},
  year         = {2013},
  note         = {State-space derivation with closed-form coefficients; also: \emph{SvfLinearTrapezoidalSin.pdf}, \emph{SvfLinearTrapOptimised.pdf}}
}

@misc{simper2014andy,
  author       = {Simper, Andrew},
  title        = {{Solving the continuous SVF equations using trapezoidal integration and equivalent currents}},
  howpublished = {Cytomic technical paper, \url{https://cytomic.com/files/dsp/SvfInputMixing.pdf}},
  year         = {2014}
}

@book{pirkle2019designing,
  author    = {Pirkle, Will C.},
  title     = {Designing Audio Effect Plugins in {C++}: For {AAX}, {AU}, and {VST3} with {DSP} Theory},
  edition   = {2},
  publisher = {Routledge},
  year      = {2019},
  isbn      = {978-1138591899},
  note      = {Chapter on Virtual Analog filters — practical ZDF/TPT implementation walkthroughs}
}

@misc{harma2003implementation,
  author       = {H{\"a}rm{\"a}, Aki},
  title        = {Implementation of frequency-warped recursive filters},
  journal      = {Signal Processing},
  volume       = {80},
  number       = {3},
  pages        = {543--548},
  year         = {2000},
  doi          = {10.1016/S0165-1684(99)00150-X},
  note         = {Discusses delay-free loops in warped filters — same algebraic problem ZDF formulations solve for the SVF}
}
```

## FIR design — windowed-sinc and frequency-sampling

```bibtex
@article{harris1978use,
  author  = {Harris, Frederic J.},
  title   = {On the use of windows for harmonic analysis with the discrete {Fourier} transform},
  journal = {Proceedings of the IEEE},
  volume  = {66},
  number  = {1},
  pages   = {51--83},
  year    = {1978},
  doi     = {10.1109/PROC.1978.10837}
}

@article{kaiser1980simple,
  author  = {Kaiser, James F.},
  title   = {On a simple algorithm to calculate the 'optimum' {FIR} low pass filter coefficients},
  journal = {IEEE Transactions on Acoustics, Speech, and Signal Processing},
  volume  = {28},
  number  = {1},
  pages   = {105--107},
  year    = {1980},
  doi     = {10.1109/TASSP.1980.1163348}
}

@book{rabiner1975theory,
  author    = {Rabiner, Lawrence R. and Gold, Bernard},
  title     = {Theory and Application of Digital Signal Processing},
  publisher = {Prentice-Hall},
  year      = {1975}
}
```

## Kirkeby inverse-filter correction (regularized inversion)

Used by `generate_kirkeby_correction` for pre-ringing-controlled inverse filtering.

```bibtex
@article{kirkeby1999digital,
  author  = {Kirkeby, Ole and Nelson, Philip A.},
  title   = {Digital Filter Design for Inversion Problems in Sound Reproduction},
  journal = {Journal of the Audio Engineering Society},
  volume  = {47},
  number  = {7/8},
  pages   = {583--595},
  year    = {1999}
}

@article{norcross2006inverse,
  author  = {Norcross, Scott G. and Bouchard, Martin and Soulodre, Gilbert A.},
  title   = {Inverse Filtering Design Using a Minimal-Phase Target Function from Regularization},
  journal = {Audio Engineering Society Convention 121},
  year    = {2006},
  url     = {https://www.aes.org/e-lib/browse.cfm?elib=13778}
}
```

## Warped digital filters (frequency-warped / Laguerre)

Bilinear-conformal frequency warping (replacing each unit delay with a first-order all-pass) gives non-uniform — typically Bark- or ERB-aligned — frequency resolution. Useful for perceptually-aligned EQ, modeling, and inverse filtering with a small filter order.

```bibtex
@article{strube1980linear,
  author  = {Strube, Hans Werner},
  title   = {Linear prediction on a warped frequency scale},
  journal = {The Journal of the Acoustical Society of America},
  volume  = {68},
  number  = {4},
  pages   = {1071--1076},
  year    = {1980},
  doi     = {10.1121/1.384992}
}

@article{oppenheim1972discrete,
  author  = {Oppenheim, Alan V. and Johnson, Don H. and Steiglitz, Kenneth},
  title   = {Computation of spectra with unequal resolution using the fast {Fourier} transform},
  journal = {Proceedings of the IEEE},
  volume  = {59},
  number  = {2},
  pages   = {299--301},
  year    = {1971},
  doi     = {10.1109/PROC.1971.8146}
}

@article{harma2000frequency,
  author  = {H{\"a}rm{\"a}, Aki and Karjalainen, Matti and Savioja, Lauri and V{\"a}lim{\"a}ki, Vesa and Laine, Unto K. and Huopaniemi, Jyri},
  title   = {Frequency-Warped Signal Processing for Audio Applications},
  journal = {Journal of the Audio Engineering Society},
  volume  = {48},
  number  = {11},
  pages   = {1011--1031},
  year    = {2000},
  url     = {https://www.aes.org/e-lib/browse.cfm?elib=12028}
}

@inproceedings{karjalainen1996realizable,
  author    = {Karjalainen, Matti and Piiril{\"a}, Erkki and J{\"a}rvinen, Aki and Huopaniemi, Jyri},
  title     = {Comparison of Loudspeaker Equalization Methods Based on {DSP} Techniques},
  booktitle = {Audio Engineering Society Convention 102},
  year      = {1997},
  url       = {https://www.aes.org/e-lib/browse.cfm?elib=7327}
}

@article{smith1999bark,
  author  = {Smith, Julius O. and Abel, Jonathan S.},
  title   = {Bark and {ERB} Bilinear Transforms},
  journal = {IEEE Transactions on Speech and Audio Processing},
  volume  = {7},
  number  = {6},
  pages   = {697--708},
  year    = {1999},
  doi     = {10.1109/89.799695}
}
```

## Kautz filters (generalized orthonormal IIR)

Generalization of FIR (Laguerre, Kautz) basis filters: an orthonormal expansion in stable IIR basis functions whose poles are placed where modeling fidelity is most needed. Used for fixed-pole IIR modeling of long impulse responses (e.g. modal reverberation, loudspeaker/room responses) at much lower order than direct-form IIR or FIR.

```bibtex
@article{kautz1954transient,
  author  = {Kautz, William H.},
  title   = {Transient Synthesis in the Time Domain},
  journal = {IRE Transactions on Circuit Theory},
  volume  = {1},
  number  = {3},
  pages   = {29--39},
  year    = {1954},
  doi     = {10.1109/TCT.1954.1083588}
}

@article{broome1965discrete,
  author  = {Broome, Paul W.},
  title   = {Discrete Orthonormal Sequences},
  journal = {Journal of the ACM},
  volume  = {12},
  number  = {2},
  pages   = {151--168},
  year    = {1965},
  doi     = {10.1145/321264.321265}
}

@book{heuberger2005modelling,
  editor    = {Heuberger, Peter S. C. and Van den Hof, Paul M. J. and Wahlberg, Bo},
  title     = {Modelling and Identification with Rational Orthogonal Basis Functions},
  publisher = {Springer},
  year      = {2005},
  doi       = {10.1007/1-84628-178-4}
}

@article{paatero2003kautz,
  author  = {Paatero, Tuomas and Karjalainen, Matti},
  title   = {Kautz Filters and Generalized Frequency Resolution: Theory and Audio Applications},
  journal = {Journal of the Audio Engineering Society},
  volume  = {51},
  number  = {1/2},
  pages   = {27--44},
  year    = {2003},
  url     = {https://www.aes.org/e-lib/browse.cfm?elib=12098}
}

@article{bank2007warped,
  author  = {Bank, Balazs},
  title   = {Warped, {Kautz}, and Fixed-Pole Parallel Filters: A Review},
  journal = {Journal of the Audio Engineering Society},
  volume  = {61},
  number  = {7/8},
  pages   = {555--566},
  year    = {2013},
  url     = {https://www.aes.org/e-lib/browse.cfm?elib=16830}
}

@article{bank2008direct,
  author  = {Bank, Balazs},
  title   = {Direct Design of Parallel Second-Order Filters for Instrument Body Modeling},
  journal = {Proceedings of the International Computer Music Conference (ICMC)},
  year    = {2007},
  url     = {https://www.mit.bme.hu/eng/projects/parfilt}
}
```

## Pre-ringing suppression

Used by `fir_design.rs::PreRingingConfig` and `suppress_pre_ringing`. The patent describes the envelope-constrained pre-ringing limiter applied after Kirkeby inverse design.

```bibtex
@misc{brannmark2009preringing,
  author       = {Br{\"a}nnmark, Lars-Johan and Sternad, Mikael},
  title        = {{Method and apparatus for designing low-pre-ringing inverse filters}},
  howpublished = {European Patent EP2104374B1},
  year         = {2009},
  note         = {Pre-ringing envelope constraint for room-correction inverse filters}
}

@article{brannmark2008compensation,
  author  = {Br{\"a}nnmark, Lars-Johan and Ahlen, Anders},
  title   = {Compensation of Loudspeaker-Room Responses in a Robust {MIMO} Control Framework},
  journal = {IEEE Transactions on Audio, Speech, and Language Processing},
  volume  = {17},
  number  = {6},
  pages   = {1201--1216},
  year    = {2009},
  doi     = {10.1109/TASL.2009.2020413}
}
```

## Zero-phase forward-backward filtering (filtfilt)

Used by `filtfilt.rs::filtfilt` / `sosfilt`. The steady-state initial-condition computation matches Gustafsson's approach (the basis for MATLAB `filtfilt` and `scipy.signal.sosfiltfilt`).

```bibtex
@article{gustafsson1996determining,
  author  = {Gustafsson, Fredrik},
  title   = {Determining the initial states in forward-backward filtering},
  journal = {IEEE Transactions on Signal Processing},
  volume  = {44},
  number  = {4},
  pages   = {988--992},
  year    = {1996},
  doi     = {10.1109/78.492552}
}
```

## Phase unwrapping and group delay

```bibtex
@article{tribolet1977new,
  author  = {Tribolet, Jose M.},
  title   = {A new phase unwrapping algorithm},
  journal = {IEEE Transactions on Acoustics, Speech, and Signal Processing},
  volume  = {25},
  number  = {2},
  pages   = {170--177},
  year    = {1977},
  doi     = {10.1109/TASSP.1977.1162923}
}

@article{smith2007spectral,
  author  = {Smith, Julius O.},
  title   = {{Spectral Audio Signal Processing}},
  publisher = {W3K Publishing},
  year      = {2011},
  url       = {https://ccrma.stanford.edu/~jos/sasp/},
  note      = {Chapters on group delay and minimum-phase reconstruction}
}
```

## Loudness weighting (PEQ loudness compensation)

K-weighting and A-weighting filter coefficients used by `iir.rs::loudness_compensation`.

```bibtex
@techreport{itu_bs1770,
  author      = {{ITU-R}},
  title       = {{Recommendation ITU-R BS.1770-4 — Algorithms to measure audio programme loudness and true-peak audio level}},
  institution = {International Telecommunication Union},
  year        = {2015},
  note        = {K-weighting filter definition}
}

@techreport{iec61672,
  author      = {{International Electrotechnical Commission}},
  title       = {{IEC 61672-1:2013 — Electroacoustics — Sound level meters — Part 1: Specifications}},
  institution = {IEC},
  year        = {2013},
  note        = {A-weighting filter definition}
}
```

## Denormal handling

```bibtex
@misc{rossbencina_denormals,
  author       = {Bencina, Ross},
  title        = {{Real-time audio programming 101: time waits for nothing — Denormals}},
  howpublished = {\url{http://www.rossbencina.com/code/real-time-audio-programming-101-time-waits-for-nothing}},
  note         = {Practical guide to FTZ/DAZ and denormal mitigation in audio code}
}

@article{intel_denormals,
  author  = {{Intel Corporation}},
  title   = {{Intel 64 and IA-32 Architectures Optimization Reference Manual}},
  note    = {SSE/AVX MXCSR FTZ and DAZ flag semantics}
}
```

## Q / bandwidth conversion

The `bw2q` / `q2bw` helpers follow the standard relation given in the RBJ cookbook:

> Q = 1 / (2 · sinh(ln(2)/2 · BW · ω₀ / sin(ω₀)))

See the RBJ cookbook entry above.
