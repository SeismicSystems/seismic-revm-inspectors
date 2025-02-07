# Seismic REVM Inspectors

This repository contains Seismic's fork of REVM Inspectors

The upstream repository lives [here](https://github.com/paradigmxyz/revm-inspectors). This fork is up-to-date with it through commit `3353282`. You can see this by viewing the [main](https://github.com/SeismicSystems/seismic-revm-inspectors/tree/main) branch on this repository

You can view all of our changes vs. upstream on this [pull request](https://github.com/SeismicSystems/seismic-revm-inspectors/pull/1). The sole purpose of this PR is display our diff; it will never be merged in to the main branch of this repo

## Main changes
The purpose of forking this repository is to support Seismic's [modifications](https://github.com/SeismicSystems/seismic-revm) to [revm](https://github.com/bluealloy/revm) within [seismic-reth](https://github.com/SeismicSystems/seismic-reth)

Seismic REVM has a slightly different representation of the state tree: instead of `U256` values, we introduce [`FlaggedStorage`](https://github.com/SeismicSystems/seismic-revm/blob/39b4dea21beda3d9a693023f69c2f6b8a940d29e/crates/primitives/src/state.rs#L174). This struct attaches a boolean to each `U256` value, marking whether it is associated with a shielded type. For more information, see seismic-revm's [README](https://github.com/SeismicSystems/seismic-revm/blob/seismic/README.md#flagged-storage)

## Structure

Seismic's forks of the [reth](https://github.com/paradigmxyz/reth) stack all have the same branch structure:
- `main` or `master`: this branch only consists of commits from the upstream repository. However it will rarely be up-to-date with upstream. The latest commit from this branch reflects how recently Seismic has merged in upstream commits to the seismic branch
- `seismic`: the default and production branch for these repositories. This includes all Seismic-specific code essential to make our network run

# revm-inspectors

Common [`revm`] inspector implementations.

Originally part of [`reth`] as the [`reth-revm-inspectors`] crate.

## Users

* [`seismic-reth`]
* [`seismic-foundry`]

[`seismic-revm`]: https://github.com/SeismicSystems/seismic-revm/
[`seismic-reth`]: https://github.com/SeismicSystems/seismic-reth/
[`seismic-reth`]: https://github.com/SeismicSystems/seismic-reth/
[`seismic-foundry`]: https://github.com/SeismicSystems/seismic-foundry/

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in these crates by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
</sub>
