//! The Saves page's file work, run on a worker thread by the window (each
//! job returns the page's data read again) or at once in the tests.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use super::sources::{self, Source};
use super::store::{self, Backup, How, Paths, Undo};
use super::summary::{self, CarNames, Summary};
use super::{Plan, export};
use crate::launcher::ui::Mark;

/// The covers' pictures shown per profile (the newest first).
pub const THUMBS_SHOWN: usize = 4;

/// Where the jobs work and what they keep between runs (the car names of
/// the game folder, the covers' small pictures).
#[derive(Clone, Debug)]
pub struct Ctx {
    pub paths: Paths,
    pub sources: sources::Ctx,
    pub cars: Arc<Mutex<Option<(PathBuf, Option<CarNames>)>>>,
    pub thumbs: Arc<Mutex<HashMap<(PathBuf, Option<SystemTime>), Arc<Vec<u8>>>>>,
}

impl Ctx {
    pub fn new(paths: Paths, sources: sources::Ctx) -> Ctx {
        Ctx { paths, sources, cars: Default::default(), thumbs: Default::default() }
    }

    /// The game folder's car names, read once per folder.
    fn cars(&self) -> Option<CarNames> {
        let game = self.sources.game.clone()?;
        let mut g = self.cars.lock().ok()?;
        if g.as_ref().map(|c| &c.0) != Some(&game) {
            *g = Some((game.clone(), CarNames::load(&game)));
        }
        g.as_ref().and_then(|c| c.1.clone())
    }

    fn thumb(&self, file: &std::path::Path) -> Option<Arc<Vec<u8>>> {
        let key = (file.to_path_buf(), store::mtime(file));
        if let Some(t) = self.thumbs.lock().ok()?.get(&key) {
            return Some(t.clone());
        }
        let t = Arc::new(summary::thumb(&std::fs::read(file).ok()?)?);
        self.thumbs.lock().ok()?.insert(key, t.clone());
        Some(t)
    }
}

/// One profile as the page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub s: Summary,
    /// The car's line (`CarNames::label`).
    pub car: String,
    /// Its newest saved covers, each with its picture made small.
    pub thumbs: Vec<CoverPic>,
}

/// A saved cover as the page shows it: its caption ("Magazine 1", "DVD 2")
/// and its picture (`summary::THUMB` square RGBA), None when it can't be
/// read (then `missing` says so on the placeholder).
#[derive(Clone, Debug, PartialEq)]
pub struct CoverPic {
    pub caption: String,
    pub rgba: Option<Arc<Vec<u8>>>,
    pub missing: String,
}

impl CoverPic {
    pub fn caption_of(c: &summary::Cover) -> String {
        format!("{} {}", if c.dvd { "DVD" } else { "Magazine" }, c.n)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Job {
    /// Read the profiles again; with `find`, look for the original's saves
    /// too.
    Load {
        find: bool,
    },
    /// The plan for importing a folder or a zip.
    Plan(PathBuf),
    Import(Plan),
    Backup(String),
    BackupAll,
    Restore(Backup, How),
    Export(String, PathBuf),
    Rename(String, String),
    Duplicate(String),
    Delete(String),
    Undo(Undo),
    DeleteBackup(Backup),
}

/// A found source with its plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub source: Source,
    pub plan: Plan,
}

/// What a job did, and the page's data read again.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Done {
    pub rows: Vec<Row>,
    pub backups: Vec<Backup>,
    /// The sources found (a `Load { find: true }`).
    pub found: Option<Vec<Found>>,
    /// An import's plan to show.
    pub plan: Option<Plan>,
    pub lines: Vec<(Mark, String)>,
    /// What the last step can undo.
    pub undo: Option<Undo>,
    /// The profile to focus afterwards.
    pub select: Option<String>,
    /// The import that was done (its plan's source is read again).
    pub imported: bool,
}

/// The profiles and backups read again.
pub fn rows(ctx: &Ctx) -> (Vec<Row>, Vec<Backup>) {
    let cars = ctx.cars();
    let rows = summary::list(&ctx.paths.profiles)
        .into_iter()
        .map(|s| {
            let mut newest: Vec<&summary::Cover> = s.covers.iter().collect();
            newest.sort_by_key(|c| {
                std::cmp::Reverse(store::mtime(&crate::frontend::profiles::find_in(&ctx.paths.profiles, &c.name)))
            });
            let thumbs = newest
                .iter()
                .take(THUMBS_SHOWN)
                .map(|c| CoverPic {
                    caption: CoverPic::caption_of(c),
                    rgba: ctx.thumb(&crate::frontend::profiles::find_in(&ctx.paths.profiles, &c.name)),
                    missing: "Can't be read".into(),
                })
                .collect();
            Row { car: CarNames::label(cars.as_ref(), &s.car), s, thumbs }
        })
        .collect();
    (rows, store::all_backups(&ctx.paths))
}

pub fn find(ctx: &Ctx) -> Vec<Found> {
    sources::discover(&ctx.sources)
        .into_iter()
        .filter_map(|source| {
            let plan = super::plan_folder(&source.path, &ctx.paths.profiles).ok()?;
            Some(Found { source, plan })
        })
        .collect()
}

fn err(lines: &mut Vec<(Mark, String)>, what: &str, e: String) {
    lines.push((Mark::Error, format!("{what}: {e}")));
}

pub fn run(ctx: &Ctx, job: Job) -> Done {
    let p = &ctx.paths;
    let mut d = Done::default();
    let lines = &mut d.lines;
    match job {
        Job::Load { find: f } => {
            store::clean_parts(p);
            if f {
                d.found = Some(find(ctx));
            }
        }
        Job::Plan(from) => match super::plan(&from, &p.profiles) {
            Ok(plan) => d.plan = Some(plan),
            Err(e) => err(lines, &from.display().to_string(), e),
        },
        Job::Import(plan) => {
            let (done, errors) = super::import(&plan, &p.profiles);
            let profiles = done.iter().filter(|n| summary::cover_parts(n).is_none()).count();
            lines.push(match profiles {
                0 if errors.is_empty() => (Mark::Info, "Nothing new to import.".into()),
                0 => (Mark::Error, "Nothing was imported.".into()),
                n => (Mark::Ok, format!("Imported {}.", crate::launcher::model::count(n, "profile", "profiles"))),
            });
            lines.extend(errors.into_iter().map(|e| (Mark::Error, e)));
            d.select = done.into_iter().next();
            d.imported = true;
            d.found = Some(find(ctx));
        }
        Job::Backup(name) => match store::snapshot(p, &name, store::Why::Manual) {
            Ok(_) => lines.push((Mark::Ok, format!("{name}: backed up."))),
            Err(e) => err(lines, &name, e),
        },
        Job::BackupAll => {
            let names = crate::frontend::profiles::list_in(&p.profiles);
            let failed: Vec<String> = names
                .iter()
                .filter_map(|n| store::snapshot(p, n, store::Why::Manual).err().map(|e| format!("{n}: {e}")))
                .collect();
            let ok = names.len() - failed.len();
            lines.push((Mark::Ok, format!("Backed up {}.", crate::launcher::model::count(ok, "profile", "profiles"))));
            lines.extend(failed.into_iter().map(|e| (Mark::Error, e)));
        }
        Job::Restore(b, how) => match store::restore(p, &b, how) {
            Ok(n) => {
                lines.push((
                    Mark::Ok,
                    if n == b.name { format!("{n}: restored.") } else { format!("Restored as {n}.") },
                ));
                d.select = Some(n);
            }
            Err(e) => err(lines, &b.name, e),
        },
        Job::Export(name, to) => match export::export(&p.profiles, &name, &to) {
            Ok(f) => lines.push((Mark::Ok, format!("{name}: exported to {}", f.display()))),
            Err(e) => err(lines, &name, e),
        },
        Job::Rename(from, to) => match store::rename(p, &from, &to) {
            Ok(u) => {
                d.select = Some(to);
                d.undo = Some(u);
            }
            Err(e) => err(lines, &format!("{from} was not renamed"), e),
        },
        Job::Duplicate(name) => match store::duplicate(p, &name) {
            Ok((to, u)) => {
                d.select = Some(to);
                d.undo = Some(u);
            }
            Err(e) => err(lines, &format!("{name} was not copied"), e),
        },
        Job::Delete(name) => match store::delete(p, &name, store::DELETE_COVERS) {
            Ok(u) => d.undo = Some(u),
            Err(e) => err(lines, &format!("{name} was not deleted"), e),
        },
        Job::Undo(u) => match store::undo(p, &u) {
            Ok(n) => {
                lines.push((Mark::Ok, format!("Undone: {}.", u.said)));
                d.select = Some(n).filter(|n| !n.is_empty());
            }
            Err(e) => err(lines, "Not undone", e),
        },
        Job::DeleteBackup(b) => match store::delete_backup(p, &b) {
            Ok(()) => lines.push((Mark::Ok, format!("The backup of {} from {} was deleted.", b.name, b.when()))),
            Err(e) => err(lines, "The backup was not deleted", e),
        },
    }
    (d.rows, d.backups) = rows(ctx);
    d
}
