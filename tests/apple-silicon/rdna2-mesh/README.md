# Synthetic RDNA2 mesh shader conformance

Tested on Apple M5 through Wine/Rosetta and D3DMetal.

- Native PC reference: 1024 correct pixels.
- Translated RDNA2 triangle: 1024 correct pixels.
- Translated RDNA2 culled primitive: 1024 correct pixels.
- Zero reported pixel mismatches.

The original assembly and HLSL sources are included.
Generated binaries are intentionally excluded.

Use LLVM llvm-mc with amdgcn-amd-amdhsa and gfx1030,
then llvm-objcopy to extract the text section.
Compile the reference HLSL with public Microsoft DXC.

See graphics_max_rdna2_mesh in
crates/victory-shader/src/emit/host.rs
for the test harness and its environment variables.

This validates synthetic shader translation and execution,
not complete Xbox shader-container loading or game compatibility.
