# Live NIS precision, fresh career intro

An owned private Wine session on display :176 ran the original executable
SHA256 8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297.
A temporary entry hook at 5AB3D0 observed FNSTCW without modifying the
control word, preserved flags, and replayed the six displaced instructions.
The 404 observed NISMover entries all used 0x007F (PC24, nearest rounding).
The non-PC24 counter remained zero. Hook, code padding and data padding
were restored and read back byte-identically. No on-disk EXE was patched.

Navigation used screenshots and private-session keys: title Return, profile
creation Left/Return, name Return, confirmation Left/Return, save Return,
then main-menu Career Left/Return and Start New Career Return. The earlier
automatic recognition failed, so this capture does not use fastnav.
The first private session had already passed the intro before instrumentation;
it was stopped, and the successful instrumented run used a fresh save.

This establishes precision at the native scene-mover entry for this career
intro, not every scene or every callee. It supports implementing the PC24
target oracle; the existing FINIT-default helper is not ready for gameplay.
The retained script has a fixed owned-session path and requires a running
private session before navigation. Its finally block restores instrumentation.
