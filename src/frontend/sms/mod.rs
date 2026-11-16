//! The career's SMS messages (docs/formats.md, "SMS").
//!
//! The message table is GlobalB's career chunk 0x34A17 (0x14-byte records,
//! 0x838454 / count 0x838450, loaded by 0x5012F0). The inbox is the career
//! object's block at +0x5D20 (0x860A60): 256 entries of 0x14 bytes, the
//! entry count at +0x1400, the unread count at +0x1404 and a "new" flag at
//! +0x140C; the career save writes it (0x5014F0, from 0x5146A4) and reads
//! it back (0x514E80, from 0x5293A3).

#[cfg(test)]
use crate::formats::hash::bin_hash;

mod inbox;
mod table;
mod triggers;

pub use super::screens::sms_mailbox::*;
pub use inbox::*;
pub use table::*;
pub use triggers::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Vec<Message> {
        let m = |hash, category, urgent, icon| Message { hash, category, urgent, icon, ..Default::default() };
        vec![m(0, 4, false, 1), m(1, 5, true, 5), m(2, 2, false, 9)]
    }

    #[test]
    fn the_inbox_counts_reads_deletes_and_files() {
        let t = table();
        let mut inbox = Inbox::default();
        assert!(inbox.add(0) && inbox.add(1) && inbox.add(2));
        assert!(!inbox.add(1), "a message arrives once");
        assert_eq!(inbox.unread(), 3);
        // 0x4ABAF0 never looks at entry 0.
        assert_eq!(inbox.notify_kind(&t), 9);
        inbox.read(2);
        assert_eq!(inbox.notify_kind(&t), 5);
        inbox.read(1);
        assert_eq!(inbox.notify_kind(&t), 8, "entry 0 is skipped: the envelope");
        assert_eq!(inbox.unread(), 1);
        // Not filed yet: everything in the Inbox, newest first.
        assert_eq!(inbox.mailbox(&t, 1), vec![2, 1, 0]);
        assert!(inbox.mailbox(&t, 5).is_empty());
        inbox.file();
        assert_eq!(inbox.mailbox(&t, 1), vec![0]);
        assert_eq!(inbox.mailbox(&t, 5), vec![1]);
        assert_eq!(inbox.mailbox(&t, 2), vec![2]);
        inbox.delete(0);
        assert_eq!(inbox.unread(), 0);
        assert!(inbox.mailbox(&t, 1).is_empty());
        // Urgent entries ask for the mailbox until shown.
        assert!(inbox.urgent_pending(&t));
        inbox.mark_shown(&t);
        assert!(!inbox.urgent_pending(&t));
    }

    #[test]
    fn the_inbox_saves_and_loads() {
        let mut inbox = Inbox::default();
        inbox.add(2);
        inbox.add(0);
        inbox.read(1);
        inbox.file();
        let mut back = Inbox::default();
        for line in inbox.save_lines().lines() {
            back.load_line(line.strip_prefix("sms ").unwrap());
        }
        assert_eq!(back, inbox);
    }

    /// The game's table: SMS_INSTRUCTION at 10 s of driving, the window
    /// tint unlock (SMS_UNLOCK_01, stage 1's second normal win) after the
    /// second stage-1 win, the outrun pair on an outrun's end.
    #[test]
    #[cfg_attr(not(ob_game), ignore = "needs NFSU2_ROOT (the player's own game files)")]
    fn messages_arrive_by_their_conditions() {
        let root = crate::testkit::game_root();
        let data = crate::frontend::career::CareerData::load(&root).unwrap();
        assert_eq!(data.messages.len(), 167);
        let by = |name: &str| data.messages.iter().find(|m| m.name == name).unwrap().hash;
        let mut p = crate::frontend::career::Profile::new("T");
        p.skip_intro(&data, &root, "PEUGOT", &mut crate::opponents::GameRng::default());
        assert!(!p.inbox.contains(by("SMS_INSTRUCTION")));
        p.drive_time = 9.0;
        p.sms_drive(&data, 0.5);
        assert!(!p.inbox.contains(by("SMS_INSTRUCTION")));
        p.sms_drive(&data, 0.6);
        assert!(p.inbox.contains(by("SMS_INSTRUCTION")));
        let normal: Vec<_> = data
            .events
            .iter()
            .filter(|e| e.stage == 1 && e.category == crate::frontend::career::Category::Normal && !e.suv)
            .cloned()
            .collect();
        p.win(&data, &root, &normal[0], 60.0, (0, 0), &mut crate::opponents::GameRng::default());
        assert!(!p.inbox.contains(by("SMS_UNLOCK_01")));
        p.win(&data, &root, &normal[1], 60.0, (0, 0), &mut crate::opponents::GameRng::default());
        assert!(p.inbox.contains(by("SMS_UNLOCK_01")));
        p.outrun_result(&data, false, &mut crate::opponents::GameRng::default());
        assert!(p.inbox.contains(by("SMS_OUTRUN_DEFEAT")) && !p.inbox.contains(by("SMS_OUTRUN_VICTORY")));
        p.sms_shop_entered(&data, bin_hash("CRIB_1"));
        assert!(p.inbox.contains(by("SMS_LOC_EVENTS_OUT_THERE")));
        // The magazine triggers (0x34A1B): stage 2's fourth normal win sends
        // SMS_MAG_S2_SHOW_1 with the generic magazine texts and the cover's
        // title; its fourth outrun win the unique visual parts' (Hoods).
        assert_eq!(data.mag_triggers.len(), 28);
        p.stage = 2;
        p.fire_triggers(&data, 0, 3);
        assert!(!p.inbox.contains(by("SMS_MAG_S2_SHOW_1")));
        p.fire_triggers(&data, 0, 4);
        let e = *p.inbox.entries.iter().find(|e| e.message == by("SMS_MAG_S2_SHOW_1")).unwrap();
        assert_eq!((e.body, e.subject), (bin_hash("SMS_MAG_GENERIC_BODY_1"), bin_hash("SMS_MAG_GENERIC_SUBJ_1")));
        assert_eq!(p.trigger_counters.magazine, 1);
        p.fire_triggers(&data, 2, 4);
        let e = *p.inbox.entries.iter().find(|e| e.message == by("SMS_UNIQUE_S2_VISUAL_1")).unwrap();
        assert_eq!((e.body, e.extra), (bin_hash("SMS_VISUAL_GENERIC_1_BODY"), 0x028F_7092));
        assert_eq!(p.trigger_counters.unique, 1);
        // The same records are the world challenges: STAGE_2_SHOW_1 starts
        // at TRIGGER_START_STAGE_2_SHOW_1, has 120 s to reach its magazine
        // shoot, and is on offer once the stage's normal wins reach 4.
        let (i, t) = data.mag_triggers.iter().enumerate().find(|(_, t)| t.name == bin_hash("STAGE_2_SHOW_1")).unwrap();
        assert_eq!((t.marker, t.time, t.kind), (bin_hash("TRIGGER_START_STAGE_2_SHOW_1"), 120, 4));
        assert!(data.milestones.iter().any(|m| m.hash == t.milestone && m.name == "STAGE_2_SHOW_1_PICTURE"));
        assert!(!p.challenges_on_offer(&data, false).contains(&i));
        let wins: Vec<_> = data
            .events
            .iter()
            .filter(|e| e.stage == 2 && e.category == crate::frontend::career::Category::Normal && !e.suv)
            .take(4)
            .map(|e| e.index)
            .collect();
        p.completed.extend(wins);
        assert!(p.challenges_on_offer(&data, false).contains(&i) && p.challenges_on_offer(&data, true).is_empty());
        p.challenges_done.insert(t.name);
        assert!(!p.challenges_on_offer(&data, false).contains(&i));
        // A DVD cover offered (condition 14 by its rating).
        p.offer_covers(&data, 10.0);
        assert!(p.inbox.contains(by("SMS_DVD_COVER_1")));
        // An engage tip pays its +0x0C once.
        let tip = data.messages.iter().find(|m| m.name == "ENGAGE_TIPS_SMS").unwrap();
        let cash = p.cash;
        assert!(p.sms_tip(&data, tip.value[0]) && !p.sms_tip(&data, tip.value[0]));
        assert_eq!(p.cash, cash + tip.value[1]);
        let assets = crate::frontend::Assets::load(&root).unwrap();
        let m = find(&data.messages, by("SMS_UNIQUE_S2_VISUAL_1")).unwrap();
        assert_eq!(subject(&assets, m, Some(&e)), "Unique Hoods");
    }

    #[test]
    fn labels_follow_the_name() {
        assert_eq!(labels("SMS_INSTRUCTION").0, "SMS_INSTRUCTION_SUBJECT");
        assert_eq!(labels("SMS_CAR_UNLOCK_7"), ("SMS_CAR_UNLOCK_1_SUBJECT".into(), "SMS_CAR_UNLOCK_1_BODY".into()));
        assert_eq!(labels("ENGAGE_TIPS_AUTO").1, "ENGAGE_TIPS_AUTO_BODY");
    }
}
