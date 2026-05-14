<!-- markdownlint-disable-file MD013 -->

# References

Foundational papers and standards behind the algorithms in `math-dsp`.

## Welch's method (FFT-based PSD)

```bibtex
@ARTICLE{1161901,
  author  = {Welch, Peter D.},
  journal = {IEEE Transactions on Audio and Electroacoustics},
  title   = {The use of fast {Fourier} transform for the estimation of power spectra: A method based on time averaging over short, modified periodograms},
  year    = {1967},
  volume  = {15},
  number  = {2},
  pages   = {70--73},
  doi     = {10.1109/TAU.1967.1161901}
}
```

## Acoustic metrics (RT60, C50/C80, EDT)

```bibtex
@article{schroeder1965new,
  author  = {Schroeder, Manfred R.},
  title   = {New Method of Measuring Reverberation Time},
  journal = {The Journal of the Acoustical Society of America},
  volume  = {37},
  number  = {3},
  pages   = {409--412},
  year    = {1965},
  doi     = {10.1121/1.1909343}
}

@techreport{iso3382,
  author      = {{International Organization for Standardization}},
  title       = {{ISO 3382-1:2009 — Acoustics — Measurement of room acoustic parameters — Part 1: Performance spaces}},
  institution = {ISO},
  year        = {2009}
}
```

## Log sweep / exponential sine sweep

```bibtex
@inproceedings{farina2000sweep,
  author    = {Farina, Angelo},
  title     = {Simultaneous Measurement of Impulse Response and Distortion with a Swept-Sine Technique},
  booktitle = {Audio Engineering Society Convention 108},
  year      = {2000},
  month     = {February},
  url       = {https://www.aes.org/e-lib/browse.cfm?elib=10211}
}
```

## EBU R128 loudness (LUFS) and Replay Gain

```bibtex
@techreport{ebu_r128,
  author      = {{European Broadcasting Union}},
  title       = {{EBU R 128 — Loudness normalisation and permitted maximum level of audio signals}},
  institution = {EBU},
  year        = {2020}
}

@techreport{ebu_tech3341,
  author      = {{European Broadcasting Union}},
  title       = {{EBU Tech 3341 — Loudness Metering: 'EBU Mode' metering to supplement EBU R 128 loudness normalisation}},
  institution = {EBU},
  year        = {2016}
}

@techreport{itu_bs1770,
  author      = {{ITU-R}},
  title       = {{Recommendation ITU-R BS.1770-4 — Algorithms to measure audio programme loudness and true-peak audio level}},
  institution = {International Telecommunication Union},
  year        = {2015}
}

@misc{robinson2001replaygain,
  author       = {Robinson, David},
  title        = {{Replay Gain — A Proposed Standard}},
  year         = {2001},
  howpublished = {\url{https://replaygain.hydrogenaud.io/}}
}
```

## Binaural loudness and surround→stereo downmix

```bibtex
@techreport{itu_bs775,
  author      = {{ITU-R}},
  title       = {{Recommendation ITU-R BS.775-4 — Multichannel stereophonic sound system with and without accompanying picture}},
  institution = {International Telecommunication Union},
  year        = {2022},
  note        = {Defines the L/R/C/Ls/Rs stereo downmix coefficients
                 (centre and surrounds at -3 dB) used by
                 \texttt{BinauralDownmix::bs775} as a level-only proxy
                 for binaural rendering when measuring multichannel
                 programme loudness.}
}

@techreport{itu_bs2051,
  author      = {{ITU-R}},
  title       = {{Recommendation ITU-R BS.2051-3 — Advanced sound system for programme production}},
  institution = {International Telecommunication Union},
  year        = {2022},
  note        = {Defines the standard channel orderings
                 (5.0, 5.1, 7.1, etc.) used by \texttt{SurroundLayout}.}
}

@techreport{itu_bs2127,
  author      = {{ITU-R}},
  title       = {{Recommendation ITU-R BS.2127-1 — Audio Definition Model renderer for advanced sound systems}},
  institution = {International Telecommunication Union},
  year        = {2023},
  note        = {Reference for object-based/ADM binaural rendering. Out
                 of scope for \texttt{math-dsp}; cited because true HRTF
                 binaural loudness should ideally pre-render via a
                 BS.2127-style renderer before applying BS.1770-4.}
}
```

## ESPRIT (frequency estimation)

```bibtex
@ARTICLE{32276,
  author  = {Roy, Richard and Kailath, Thomas},
  journal = {IEEE Transactions on Acoustics, Speech, and Signal Processing},
  title   = {{ESPRIT} — Estimation of signal parameters via rotational invariance techniques},
  year    = {1989},
  volume  = {37},
  number  = {7},
  pages   = {984--995},
  doi     = {10.1109/29.32276}
}
```

## STFT / phase reconstruction (RTPGHI)

```bibtex
@article{prusa2017rtpghi,
  author  = {Pr{\r{u}}{\v{s}}a, Zden{\v{e}}k and S{\o}ndergaard, Peter L. and Rajmic, Pavel},
  title   = {Real-Time Spectrogram Inversion Using Phase Gradient Heap Integration},
  journal = {Proceedings of the 20th International Conference on Digital Audio Effects (DAFx-17)},
  year    = {2017},
  pages   = {17--21}
}

@article{griffin1984signal,
  author  = {Griffin, Daniel W. and Lim, Jae S.},
  title   = {Signal estimation from modified short-time {Fourier} transform},
  journal = {IEEE Transactions on Acoustics, Speech, and Signal Processing},
  volume  = {32},
  number  = {2},
  pages   = {236--243},
  year    = {1984},
  doi     = {10.1109/TASSP.1984.1164317}
}

@article{portnoff1976implementation,
  author  = {Portnoff, Michael R.},
  title   = {Implementation of the digital phase vocoder using the fast {Fourier} transform},
  journal = {IEEE Transactions on Acoustics, Speech, and Signal Processing},
  volume  = {24},
  number  = {3},
  pages   = {243--248},
  year    = {1976},
  doi     = {10.1109/TASSP.1976.1162810}
}
```

## Instantaneous frequency

```bibtex
@article{boashash1992estimating,
  author  = {Boashash, Boualem},
  title   = {Estimating and interpreting the instantaneous frequency of a signal — Parts 1 \& 2},
  journal = {Proceedings of the IEEE},
  volume  = {80},
  number  = {4},
  pages   = {520--568},
  year    = {1992},
  doi     = {10.1109/5.135376}
}
```

## Tonal / transient separation

```bibtex
@inproceedings{fitzgerald2010harmonic,
  author    = {Fitzgerald, Derry},
  title     = {Harmonic/Percussive Separation Using Median Filtering},
  booktitle = {Proceedings of the 13th International Conference on Digital Audio Effects (DAFx-10)},
  year      = {2010},
  pages     = {246--253}
}

@inproceedings{driedger2014extending,
  author    = {Driedger, Jonathan and M{\"u}ller, Meinard and Disch, Sascha},
  title     = {Extending Harmonic-Percussive Separation of Audio Signals},
  booktitle = {Proceedings of the 15th International Society for Music Information Retrieval Conference (ISMIR)},
  year      = {2014},
  pages     = {611--616}
}
```

## Feedback Delay Network (FDN) reverb

```bibtex
@inproceedings{jot1991digital,
  author    = {Jot, Jean-Marc and Chaigne, Antoine},
  title     = {Digital Delay Networks for Designing Artificial Reverberators},
  booktitle = {Audio Engineering Society Convention 90},
  year      = {1991},
  url       = {https://www.aes.org/e-lib/browse.cfm?elib=5663}
}

@inproceedings{stautner1982designing,
  author    = {Stautner, John and Puckette, Miller},
  title     = {Designing Multi-Channel Reverberators},
  booktitle = {Computer Music Journal},
  volume    = {6},
  number    = {1},
  pages     = {52--65},
  year      = {1982},
  doi       = {10.2307/3680358}
}
```

## Audio features (chroma, tempo, MIR)

```bibtex
@book{muller2015fundamentals,
  author    = {M{\"u}ller, Meinard},
  title     = {Fundamentals of Music Processing: Audio, Analysis, Algorithms, Applications},
  publisher = {Springer},
  year      = {2015},
  doi       = {10.1007/978-3-319-21945-5}
}

@article{ellis2007beat,
  author  = {Ellis, Daniel P. W.},
  title   = {Beat Tracking by Dynamic Programming},
  journal = {Journal of New Music Research},
  volume  = {36},
  number  = {1},
  pages   = {51--60},
  year    = {2007},
  doi     = {10.1080/09298210701653344}
}
```

## Pink noise (Voss-McCartney)

```bibtex
@misc{mccartney_pink,
  author       = {McCartney, James and Voss, Richard F.},
  title        = {{A New Shade of Pink — algorithm for pink noise generation by summing octave-band white noise}},
  howpublished = {\url{https://www.firstpr.com.au/dsp/pink-noise/}},
  note         = {Voss-McCartney algorithm}
}
```
