# Apple Silicon research checkpoint

Experimental collaboration branch. Not an official release.

## Independent research branches

- Graphics: https://github.com/pedrofort/VIctory/tree/apple-silicon-research-20261124-031110-graphics
- Systems: https://github.com/pedrofort/VIctory/tree/apple-silicon-research-20261124-031110-systems

## Previously verified results

### Graphics

- Apple M5, Wine/Rosetta and D3DMetal.
- Mesh Shader Tier 1 and Shader Model 6.6 reported.
- Native PC mesh reference: 1024 matching pixels.
- Translated RDNA2 triangle: 1024 matching pixels.
- Translated RDNA2 culled primitive: 1024 matching pixels.
- Total: 3072 correct pixels, zero reported mismatches.
- Original assembly, HLSL and evidence included.

### Systems

- Snapshot pool retention reduced from 320 MiB to 128 MiB.
- 60% reduction in retained allocation capacity.
- Early pipeline metadata persistence independent of
  host pipeline-library support.
- Three of three pipeline keys persisted on normal exit.
- Blocking CPU fence conformance candidate validated.
- Graphics and compute regressions previously passed.

### Shared baseline

- Host submission completion checks.
- Present HRESULT instrumentation.
- Experimental texture and indirect-rendering compatibility.

## Integration status

Independent candidates were tested before integration.
The merged source has not yet undergone a full build
or integrated runtime regression.

This is a source checkpoint for further collaboration.

## Build

    cargo xwin build --locked --manifest-path apps/victory/Cargo.toml --features diagnostics --target x86_64-pc-windows-msvc

## Remaining work

- Full integrated build and regression.
- Complete Xbox mesh-container conformance.
- Advanced textures and indirect rendering.
- Cross-queue ordering.
- Physical memory measurements and streaming.
- Long-duration stability.

The pipeline library limitation remains known.
Snapshot capacity is not equivalent to physical RAM usage.
No game compatibility or performance claim is made.

No proprietary game assets, SDK binaries, third-party
executables or credentials are included.
