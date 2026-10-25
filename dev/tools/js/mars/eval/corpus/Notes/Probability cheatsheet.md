# Probability cheatsheet

## Bayes' theorem

$$
P(A \mid B) = \frac{P(B \mid A)\,P(A)}{P(B)}
$$

## Expectation and variance

$$
\mathbb{E}[X] = \sum_x x\,P(X = x), \qquad \operatorname{Var}(X) = \mathbb{E}[X^2] - \mathbb{E}[X]^2
$$

## Common distributions

| Distribution | Mean | Variance |
|--------------|------|----------|
| Bernoulli($p$) | $p$ | $p(1-p)$ |
| Binomial($n, p$) | $np$ | $np(1-p)$ |
| Poisson($\lambda$) | $\lambda$ | $\lambda$ |
| Normal($\mu, \sigma^2$) | $\mu$ | $\sigma^2$ |

A test that is 99% accurate for a condition that 1 in 1,000 people have still gives mostly
false positives: Bayes' theorem puts the chance of having it after a positive test near 9%.
