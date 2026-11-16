use crate::*;

impl Game {
    /// Game state 0xD (0x606740 → 0x4976B0): with exactly one message
    /// unread (0x514D30(0)) UI_EngageMessageDialog on it (0x514CF0), else
    /// UI_SMS_Mailbox on the Inbox (0x7F4434 = 1).
    pub(crate) fn sms_open(&mut self) -> Option<Mode> {
        use nfsu2::frontend::sms::{Mailbox, Viewer};
        let now = self.now();
        let p = self.profile.as_mut()?;
        let messages = &self.game_data.career.messages;
        Some(if p.inbox.unread() == 1 {
            let e = p.inbox.first_unread()?;
            // The viewer reads it as it opens (0x497850 → 0x514DC0).
            p.inbox.read(e);
            self.career_world.dday.on_read();
            Mode::Menu(Menu::SmsViewer(Viewer::new(&self.game_data.assets, &p.inbox, messages, e, true, 1, now)))
        } else {
            Mode::Menu(Menu::SmsMailbox(Mailbox::new(&p.inbox, messages, 1, None, now)))
        })
    }

    /// Rachel's line: a speech switch tuple played on the speech stream
    /// (`Streams::say`).
    pub(crate) fn say(&mut self, line: Option<Vec<u32>>) {
        if let (Some(line), Some(a)) = (line, self.audio.as_mut()) {
            a.streams.say(&a.mixer, &line);
        }
    }

    /// An outrun line (0x511000 / 0x5110C0): the speech stops (0x45B490)
    /// and the line is set (0x510EF0), when there is a speech object.
    pub(crate) fn rachel_outrun(&mut self, line: Vec<u32>) {
        if let Some(a) = self.audio.as_mut() {
            a.streams.stop_speech(&a.mixer);
            a.streams.say(&a.mixer, &line);
        }
    }

    /// 0x525A00: whether Rachel may speak (no audio: always).
    pub(crate) fn speech_free(&self) -> bool {
        self.audio.as_ref().is_none_or(|a| a.streams.speech_free(&a.mixer))
    }

    /// 0x529710 (the SMS screens opened by the game, not the key): it
    /// plays sound 0x838119 when set, the table 0x837FE0's cell 27·11 + 16
    /// (FOCUS on an SMS_POPUP control): id 13.
    pub(crate) fn sms_popup_sound(&mut self) {
        self.fe_sound(fe::event::FOCUS, fe::control::SMS_POPUP);
    }

    /// 0x4977C0: leaving the SMS screens files the read entries (0x514D80)
    /// and goes back to the game (0x5F4250(1)). The clone writes the save
    /// here so the read state keeps (the game keeps it in the career block
    /// its save points write, 0x5014F0).
    pub(crate) fn sms_close(&mut self) {
        // Flow event 2 in state 0xD (0x6069C4 → 0x497730): the script
        // notes the screens closed.
        self.career_world.dday.on_sms_closed();
        if let Some(p) = self.profile.as_mut() {
            p.inbox.file();
            let _ = p.commit();
        }
        self.mode = Mode::Explore;
    }

    /// The SMS screens' keys (0x4CCC10, 0x497B00).
    pub(crate) fn sms_key(&mut self, key: Key) {
        use nfsu2::frontend::sms::{Mailbox, SmsKey, Viewer, find};
        let now = self.now();
        let Some(p) = self.profile.as_mut() else { return };
        let messages = &self.game_data.career.messages;
        let (outcome, mailbox) = match &mut self.mode {
            Mode::Menu(Menu::SmsMailbox(m)) => (m.key(key, &p.inbox, messages, now), Some(m.clone())),
            Mode::Menu(Menu::SmsViewer(v)) => (v.key(key), None),
            _ => return,
        };
        match outcome {
            SmsKey::None => {}
            SmsKey::Open(e) => {
                let category = mailbox.as_ref().map_or(1, Mailbox::category);
                if p.inbox.entries.get(e).is_some_and(|x| !x.read) {
                    self.career_world.dday.on_read();
                }
                p.inbox.read(e);
                self.mode =
                    Mode::Menu(Menu::SmsViewer(Viewer::new(&self.game_data.assets, &p.inbox, messages, e, false, category, now)));
            }
            SmsKey::Delete(entry) => {
                let Some(mailbox) = mailbox else { return };
                let no = Mode::Menu(Menu::SmsMailbox(mailbox.clone()));
                self.mode = self.ask(0xF0A6_E6B5, Mode::Menu(Menu::LotDeal(LotDeal::SmsDelete { entry, mailbox })), no);
            }
            SmsKey::Leave => self.sms_close(),
            // The viewer's Back: opened on its own, the read entries are
            // filed and the mailbox opens on the message's own (its +0x03);
            // from the mailbox, back to that mailbox on its row.
            SmsKey::Mailbox => {
                let Mode::Menu(Menu::SmsViewer(v)) = &self.mode else { return };
                let (entry, mut category) = (v.entry, v.category);
                if v.alone {
                    p.inbox.file();
                    category =
                        p.inbox.entries.get(entry).and_then(|e| find(messages, e.message)).map_or(1, |m| m.category);
                }
                self.mode = Mode::Menu(Menu::SmsMailbox(Mailbox::new(&p.inbox, messages, category, Some(entry), now)));
            }
        }
    }
}
