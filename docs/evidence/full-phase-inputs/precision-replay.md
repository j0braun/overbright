# PC24 ground-query arithmetic and projection

All **2,580 recorded component boundaries** in `full-landing.bin.zlib` have x87 control word **0x007F**: 24-bit significand precision, round to nearest. The FXSAVE image begins at record +80, and its first little-endian u16 is FCW. Records are 65,536 bytes. `precision_fixture.py` verifies the original raw SHA256, asserts this control word for every record and writes counts by event to `precision.json`. This observation covers the retained Peugeot 206 Quick Race session, not every process mode.

The previous ground-query projection summed matrix terms in Python double precision and rounded only the result. `441000` instead executes each operation at PC24, with X accumulating (Z+Y)+X and Y/Z accumulating (Z+X)+Y, then matrix translation and body COM. The corrected exporter changes 2 X, 2 Y and 37 Z query coordinates among 2,236 wheel cases. Expected native compression values remain unchanged.

Two production arithmetic differences are supported by disassembly and retained native state:

* Cached plane offset: `5D806B..5D8087` accumulates `(vertex.z*normal.z + vertex.y*normal.y) + vertex.x*normal.x`. The old `Vec3::dot` order differed by four f32 ULPs on native face 101 and two on face 105. The new regression compares all normal/offset bits for the six distinct retained planes. It failed before the correction and passes afterward.
* Height: both the recent-cache path (`5E7396..5E73B5`) and grid path (`5E7624..5E7644`) multiply their numerator by the separately rounded reciprocal `-1 / normal.z`. The old final division rounded differently. The production height calculation now follows the traced operation sequence.

With those changes and corrected native query points, actual `CarState::wheel_compression` → `World::ground_for` → `TrackGround::query` matches all **2,236 captured clamped compressions bit-for-bit**, while preserving exact surface identity and the four-wheel traversal's final recent-floor order. No fallback history, timestep cap or tolerance increase was added. The test still rejects section-boundary and nonzero extra-height cases outside this capture's projection.

## Full-force limits

This closes the height-query arithmetic for the recorded inputs, not full runtime frame equivalence. The red full-force diagnostic reconstructs a quaternion/model origin from the native cached matrix/COM. That reconstruction changes some query-point bits and creates small nonzero damper rates where the native input has zero. Native wheel axes also retain native trigonometric approximation scale. An explicit body-frame/force-input boundary is the next step so replay can consume those captured inputs without silently claiming that current runtime quaternion representation matches them.

The native compression denominator adds Y/Z products before X; current production dot ordering is not changed here because the captured road normals have Y=0 and the actual query replay already matches exactly. Nonzero-Y coverage remains required for a further rounding correction.
