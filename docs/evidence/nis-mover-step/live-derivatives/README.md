# Live NIS integration boundary

Owned fresh-career session display176, verified original executable as in
../live-precision. Temporary hooks at5AA4AA (after derivative collection) and
5AA58F (after integration, before component post callbacks) filter composite
vtable7A258C. Recorder preserves flags/registers and copies integer words;
it does not alter FPU state. It retains the LAST completed derivative36,
pre-state36 and post-state36 arrays, not all1212 observed integrations.

All36 final state values match f32(pre + f32(dt*derivative)) bit-for-bit,
with dt0.011111111380159855 and CW007F. Chassis slots32/33 have zero
derivatives, but34/35 contain BF7FD078 andBD1C418A. The no-op callback
therefore does not imply zero scratch: those two values change the wheel
state before component post. Their origin/lifetime and subsequent effects
remain open. No zero-fill assumption or production implementation is added.

Hooks and padding were restored and read back byte-identically; ownedWine
then stopped. Navigation matches prior fresh-career capture. Raw snapshot
and reproducible decoder retained. NumPy is used only to check arithmetic.
Assemble recorder with llvm-mc i386, link text782B00 and symbols
pre_return=5AA4B5/post_return=5AA596, extract.text viaobjcopy.
The hook count is not a claim of1212 independently verified snapshots.
