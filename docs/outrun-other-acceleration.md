# Native other-car acceleration projection

The road-entry scalar `60BE40 -> 5EC340` uses retained world acceleration,
separately from own-car `5EC3B0`'s forward-axis projection. Binding is explicit:
`5FBA36..5FBA4E` and `6044D3..6044F3` pass car `+60`, `+70`, `+80`, `+90` to
`60BD00`. That setter assigns road-entry `+10` to velocity `+70` and `+14` to
acceleration `+80`. Retained disassembly includes both callers and the setter.

`60BE40` computes sqrt((vx²+vy²)+vz²), stores the speed as f32 and passes it with
both vector pointers to `5EC340`. Its comparison uses constant `784358`, bits
`3DCCCCCD` (0.1f32). **Strictly below** that threshold it returns the full 3D
acceleration magnitude. At or above it, it calls `43CF10` to normalize the full
velocity vector, then sums Y-product + Z-product + X-product. Negative direct
speed also takes the magnitude branch; NaN direct speed takes normalization.
These direct-helper cases are evidence of the helper, not ordinary finite car
entry states. There is no division by controller dt and no heading projection.

`43CF10` recomputes the 3D length, multiplies each component by its reciprocal,
and stores f32 components. A zero vector normalizes to +X; that fallback is
unreachable through the finite ordinary entry's projection branch because zero
speed takes the magnitude branch first. PC53/64 can retain a length with more
precision than the f32 speed argument. The PC24 implementation can reuse its
already rounded length; a generic normalize-or-zero or XY normalization would
not preserve these semantics.

## Evidence and validation

internal evidence executes complete unchanged
original `60BE40`, `5EC340`, and `43CF10` instruction blocks in an owned headless
Wine helper with synthetic vectors. It uses a private prefix and shuts down its
wineserver on success/failure. It does not attach to the live game. Executable
SHA256 and block/result hashes are in `other-acceleration-followup/metadata.json`.

`check-other-acceleration.py` checks all 351 original results, including the
separately returned normalized vector: 105 entry and 12 direct-helper cases for
each PC24/53/64 mode. Cases include zero/signed-zero velocity, the exact threshold
and adjacent f32 values, vertical motion, braking, mixed vectors and deterministic
random finite vectors. Coverage counts are asserted. Rust checks all 105 PC24
entry results bit-for-bit using the derived fixture. The game regression checks
that missing/stale controller velocity history and different controller dt no
longer change the road-entry value; own-forward output remains separate.

Runtime `CarState::velocity_acceleration` uses explicit PC24 arithmetic based on
the retained normal-world FXSAVE evidence described in
`outrun-acceleration-provenance.md`. This is not a universal live-mode CW claim.
The Python f64 model matches these PC53/64 fixtures but is not a general x87
extended-precision emulator. Rust f32 extended-exponent underflow/overflow,
nonfinite vector exception behavior and directed rounding remain outside the
validated finite domain.

The bounded game change replaces `road_car`'s XY finite difference from legacy
controller history with the retained vector helper. History storage/call
arguments remain for now to avoid unrelated lifecycle changes. No change is
made to own-action forward projection, controller scheduling or action choice.
