<!-- markdownlint-disable-file MD013 -->

# References

- <https://en.wikipedia.org/wiki/Differential_evolution>
- <https://www.sfu.ca/~ssurjano/optimization.html>
- <https://ch.mathworks.com/matlabcentral/fileexchange/23147-test-functions-for-global-optimization-algorithms>

## Scipy

```bibtex
@ARTICLE{2020SciPy-NMeth,
  author  = {Virtanen, Pauli and Gommers, Ralf and Oliphant, Travis E. and
            Haberland, Matt and Reddy, Tyler and Cournapeau, David and
            Burovski, Evgeni and Peterson, Pearu and Weckesser, Warren and
            Bright, Jonathan and {van der Walt}, St{\'e}fan J. and
            Brett, Matthew and Wilson, Joshua and Millman, K. Jarrod and
            Mayorov, Nikolay and Nelson, Andrew R. J. and Jones, Eric and
            Kern, Robert and Larson, Eric and Carey, C J and
            Polat, {\.I}lhan and Feng, Yu and Moore, Eric W. and
            {VanderPlays}, Jake and Laxalde, Denis and Perktold, Josef and
            Cimrman, Robert and Henriksen, Ian and Quintero, E. A. and
            Harris, Charles R. and Archibald, Anne M. and
            Ribeiro, Ant{\^o}nio H. and Pedregosa, Fabian and
            {van Mulbregt}, Paul and {SciPy 1.0 Contributors}},
  title   = {{{SciPy} 1.0: Fundamental Algorithms for Scientific Computing in Python}},
  journal = {Nature Methods},
  year    = {2020},
  volume  = {17},
  pages   = {261--272},
  adsurl  = {https://rdcu.be/b08Wh},
  doi     = {10.1038/s41592-019-0686-2},
}
```

## A large review in 2020 of the DE space

```bibtex
@article{BILAL2020103479,
    title = {Differential Evolution: A review of more than two decades of research},
    journal = {Engineering Applications of Artificial Intelligence},
    volume = {90},
    pages = {103479},
    year = {2020},
    issn = {0952-1976},
    doi = {https://doi.org/10.1016/j.engappai.2020.103479},
    url = {https://www.sciencedirect.com/science/article/pii/S095219762030004X},
    author = { Bilal and Millie Pant and Hira Zaheer and Laura Garcia-Hernandez and Ajith Abraham},
    keywords = {Meta-heuristics, Differential evolution, Mutation, Crossover, Selection},
    abstract = {Since its inception in 1995, Differential Evolution (DE) has emerged as one of the most frequently used algorithms for solving complex optimization problems. Its flexibility and versatility have prompted several customized variants of DE for solving a variety of real life and test problems. The present study, surveys the near 25 years of existence of DE. In this extensive survey, 283 research articles have been covered and the journey of DE is shown through its basic aspects like population generation, mutation schemes, crossover schemes, variation in parameters and hybridized variants along with various successful applications of DE. This study also provides some key bibliometric indicators like highly cited papers having citations more than 500, publication trend since 1996, journal citations etc. The main aim of the present document is to serve as an extended summary of 25 years of existence of DE, intended for dissemination to interested parties. It is expected that the present survey would generate interest among the new users towards the philosophy of DE and would also guide the experience researchers.}
}
```

## JADE

```bibtex
@ARTICLE{5208221,
  author={Zhang, Jingqiao and Sanderson, Arthur C.},
  journal={IEEE Transactions on Evolutionary Computation},
  title={JADE: Adaptive Differential Evolution With Optional External Archive},
  year={2009},
  volume={13},
  number={5},
  pages={945-958},
  keywords={Genetic mutations;Programmable control;Adaptive control;Convergence;Automatic control;Evolutionary computation;Feedback;Robustness;Particle swarm optimization;Performance analysis;Adaptive parameter control;differential evolution;evolutionary optimization;external archive},
  doi={10.1109/TEVC.2009.2014613}}
```

## Levenberg-Marquardt

Damped least-squares method for nonlinear least-squares problems. Levenberg's 1944 formulation and Marquardt's 1963 algorithmic refinement; Moré's 1978 paper documents the trust-region implementation that became the basis for MINPACK and most modern solvers.

```bibtex
@article{levenberg1944method,
  author  = {Levenberg, Kenneth},
  title   = {A Method for the Solution of Certain Non-Linear Problems in Least Squares},
  journal = {Quarterly of Applied Mathematics},
  volume  = {2},
  number  = {2},
  pages   = {164--168},
  year    = {1944},
  doi     = {10.1090/qam/10666}
}

@article{marquardt1963algorithm,
  author  = {Marquardt, Donald W.},
  title   = {An Algorithm for Least-Squares Estimation of Nonlinear Parameters},
  journal = {Journal of the Society for Industrial and Applied Mathematics},
  volume  = {11},
  number  = {2},
  pages   = {431--441},
  year    = {1963},
  doi     = {10.1137/0111030}
}

@incollection{more1978levenberg,
  author    = {Mor{\'e}, Jorge J.},
  title     = {The Levenberg-Marquardt algorithm: Implementation and theory},
  booktitle = {Numerical Analysis},
  editor    = {Watson, G. A.},
  series    = {Lecture Notes in Mathematics},
  volume    = {630},
  pages     = {105--116},
  publisher = {Springer Berlin Heidelberg},
  year      = {1978},
  doi       = {10.1007/BFb0067700}
}
```

## COBYLA

Constrained Optimization BY Linear Approximation — derivative-free trust-region method by M. J. D. Powell.

```bibtex
@incollection{powell1994cobyla,
  author    = {Powell, M. J. D.},
  title     = {A Direct Search Optimization Method That Models the Objective and Constraint Functions by Linear Interpolation},
  booktitle = {Advances in Optimization and Numerical Analysis},
  editor    = {Gomez, Susana and Hennart, Jean-Pierre},
  series    = {Mathematics and Its Applications},
  volume    = {275},
  pages     = {51--67},
  publisher = {Springer Netherlands},
  address   = {Dordrecht},
  year      = {1994},
  doi       = {10.1007/978-94-015-8330-5_4},
  isbn      = {978-94-015-8330-5}
}

@techreport{powell1998cobyla,
  author      = {Powell, M. J. D.},
  title       = {Direct search algorithms for optimization calculations},
  institution = {Department of Applied Mathematics and Theoretical Physics, University of Cambridge},
  number      = {DAMTP 1998/NA04},
  year        = {1998},
  note        = {Acta Numerica 7, 287--336}
}
```

## ISRES

Improved Stochastic Ranking Evolution Strategy — constrained nonlinear optimization via stochastic ranking by Runarsson and Yao.

```bibtex
@ARTICLE{1255569,
  author  = {Runarsson, Thomas P. and Yao, Xin},
  journal = {IEEE Transactions on Systems, Man, and Cybernetics, Part C (Applications and Reviews)},
  title   = {Search Biases in Constrained Evolutionary Optimization},
  year    = {2005},
  volume  = {35},
  number  = {2},
  pages   = {233--243},
  doi     = {10.1109/TSMCC.2004.841906}
}

@ARTICLE{873238,
  author  = {Runarsson, Thomas P. and Yao, Xin},
  journal = {IEEE Transactions on Evolutionary Computation},
  title   = {Stochastic ranking for constrained evolutionary optimization},
  year    = {2000},
  volume  = {4},
  number  = {3},
  pages   = {284--294},
  doi     = {10.1109/4235.873238}
}
```
