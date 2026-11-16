//! Saving and loading the profile (native save files).

use super::*;

// The trigger messages' counters as this session left them
// ([`Profile::trigger_counters`]). In the original 0x864210 / 0x86420C
// are plain globals outside the saved career block whose only writers
// are 0x5338D0's increments (0x533964, 0x533A2D): zero at start-up, not
// in the profile file and never reset by a load, a new career or Resume.
// The native file has no field for them, so the clone keeps them here
// and `Profile::load` (Resume, Load) carries them over.
// Per thread: the game's career runs on one, and each test thread (a
// `profiles::sandbox` too) is its own session.
thread_local! {
    pub(super) static SESSION_TRIGGER_COUNTERS: std::cell::Cell<crate::frontend::sms::TriggerCounters> = const { std::cell::Cell::new(crate::frontend::sms::TriggerCounters { magazine: 0, unique: 0 }) };
}

impl Profile {
    pub fn path() -> PathBuf {
        // Overbright's settings folder (`platform::dirs::config`).
        crate::platform::dirs::config().join("career.txt")
    }

    /// Save into the original's profile file (see `profiles`): the
    /// payload the profile was loaded from (else the original's fresh one)
    /// with the clone's fields written in. The profile becomes current.
    pub fn save(&self) -> Result<()> {
        self.store(true)
    }

    /// Bring the current profile's payload in memory up to date without
    /// writing the file: the original keeps the career in memory and
    /// writes the file only at its save points (the memory-card flow
    /// 0x4A3100 → MC_Main states 6 / 7, see "Save points" in
    /// docs/formats.md); Resume and the Profile Manager's Save read it.
    pub fn commit(&self) -> Result<()> {
        self.store(false)
    }

    fn store(&self, write: bool) -> Result<()> {
        if self.scratch {
            return Ok(());
        }
        let ctx = NativeContext::get().context("the native profile context is not set")?;
        ctx.check_writable()?;
        // The folder the profile was loaded from or made in when the name
        // is the current one's in any case (the original's file system
        // makes no difference; see `profiles::resolve`), else the folder
        // the name finds.
        let typed = crate::frontend::profiles::sanitize(&self.name);
        let name = crate::frontend::profiles::current_name()
            .filter(|c| c.eq_ignore_ascii_case(&typed))
            .unwrap_or_else(|| crate::frontend::profiles::resolve(&typed));
        let base = (crate::frontend::profiles::current_name().as_deref() == Some(name.as_str()))
            .then(crate::frontend::profiles::current_save)
            .flatten()
            .or_else(|| crate::frontend::profiles::read(&name).ok())
            .unwrap_or_else(|| crate::formats::native_save::NativeSave::fresh(&name));
        let s = self.to_native(&base, ctx);
        if write {
            crate::frontend::profiles::write(&name, &s)?;
        }
        crate::frontend::profiles::set_current(&name, s);
        Ok(())
    }

    /// The current profile as kept in memory (its file as loaded or last
    /// saved, with the commits since), else as saved on disk.
    pub fn load() -> Option<Profile> {
        let ctx = NativeContext::get()?;
        let name = crate::frontend::profiles::current_name()?;
        let s = crate::frontend::profiles::current_save().or_else(|| crate::frontend::profiles::read(&name).ok())?;
        let mut p = Profile::from_native(&s, ctx);
        p.trigger_counters = SESSION_TRIGGER_COUNTERS.get();
        Some(p)
    }

    /// The payload this profile would be saved as (the save-point
    /// snapshot 0x52A470 and its comparison 0x52A4D0).
    pub fn native_payload(&self) -> Option<Vec<u8>> {
        let ctx = NativeContext::get()?;
        let base = crate::frontend::profiles::current_save().unwrap_or_else(|| {
            crate::formats::native_save::NativeSave::fresh(&crate::frontend::profiles::sanitize(&self.name))
        });
        let mut s = self.to_native(&base, ctx);
        // The options live in the profile too (`Settings::write_native`
        // on the current payload); they are in `base` already.
        s.set_name(&self.name);
        Some(s.payload)
    }
}
