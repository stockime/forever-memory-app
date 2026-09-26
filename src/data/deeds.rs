//! Deeds: a fixed list of feats, some for everyone and some for each class,
//! each worked out from what was recorded (the event log and the combat log).

use super::Model;
use super::combat::{Combat, Hit, UnitKind};
use super::memory::{Character, parse_link};
use crate::tr;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    /// Earned, and when, if the data says.
    Earned(Option<f64>),
    Progress {
        have: f64,
        need: f64,
    },
}

/// How a deed's progress reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Count,
    Copper,
    Seconds,
}

pub struct Deed {
    pub id: &'static str,
    pub name: &'static str,
    pub text: &'static str,
    /// A game icon (file data ID).
    pub icon: i64,
    /// The class file ("PALADIN") it belongs to; None for everyone.
    pub class: Option<&'static str>,
    pub unit: Unit,
    eval: fn(&Mine) -> Status,
}

impl Deed {
    pub fn eval(&self, m: &Mine) -> Status {
        (self.eval)(m)
    }
}

/// Places the game calls dungeons (Forever's Ruins of Lordaeron among them).
const DUNGEONS: &[&str] = &[
    "Ragefire Chasm",
    "Wailing Caverns",
    "The Deadmines",
    "Shadowfang Keep",
    "Blackfathom Deeps",
    "The Stockade",
    "Gnomeregan",
    "Razorfen Kraul",
    "Scarlet Monastery",
    "Razorfen Downs",
    "Uldaman",
    "Zul'Farrak",
    "Maraudon",
    "The Temple of Atal'Hakkar",
    "Blackrock Depths",
    "Blackrock Spire",
    "Dire Maul",
    "Scholomance",
    "Stratholme",
    "Ruins of Lordaeron",
];

/// Creature names that give the undead away; the logs carry no creature type.
const UNDEAD: &[&str] = &[
    "zombie",
    "skeleton",
    "ghoul",
    "ghost",
    "spirit",
    "wraith",
    "banshee",
    "lich",
    "abomination",
    "bonecaster",
    "skull",
    "rattlecage",
    "scourge",
    "wight",
    "mummy",
    "risen",
    "corpse",
    "shade",
    "specter",
    "spectre",
    "haunt",
    "ghast",
    "geist",
    "bone",
    "undying",
    "unliving",
    "restless",
    "rotting",
    "plagued",
];

pub fn all() -> Vec<Deed> {
    use Unit::*;
    let d = |id, name, text, icon, class, unit, eval| Deed {
        id,
        name,
        text,
        icon,
        class,
        unit,
        eval,
    };
    let (war, pal, hun, rog, pri, sha, mag, wlk, dru) = (
        Some("WARRIOR"),
        Some("PALADIN"),
        Some("HUNTER"),
        Some("ROGUE"),
        Some("PRIEST"),
        Some("SHAMAN"),
        Some("MAGE"),
        Some("WARLOCK"),
        Some("DRUID"),
    );
    vec![
        // ---- everyone
        d(
            "first-death",
            tr!("Not the Last Time"),
            tr!("Die for the first time. The spirit healer will remember your face."),
            136147,
            None,
            Count,
            |m| nth(m.event_times("death"), 1),
        ),
        d(
            "payback",
            tr!("Payback"),
            tr!("Go back and kill whatever killed you, within half an hour."),
            132347,
            None,
            Count,
            payback,
        ),
        d(
            "dungeon",
            tr!("Stronger Together"),
            tr!("Enter a dungeon with a group at your back."),
            134149,
            None,
            Count,
            |m| match m.runs().first() {
                Some(r) => Status::Earned(Some(r.start)),
                None => Status::Progress {
                    have: 0.0,
                    need: 1.0,
                },
            },
        ),
        d(
            "level-10",
            tr!("Coming of Age"),
            tr!("Reach level 10."),
            134414,
            None,
            Count,
            |m| level(m, 10),
        ),
        d(
            "level-20",
            tr!("Twenty Winters"),
            tr!("Reach level 20."),
            134153,
            None,
            Count,
            |m| level(m, 20),
        ),
        d(
            "undead-100",
            tr!("Rest in Pieces"),
            tr!("Put 100 of the restless dead back in the ground."),
            135974,
            None,
            Count,
            |m| {
                nth(
                    m.kills
                        .iter()
                        .filter(|(_, u)| is_undead(m.cb.unit_name(*u)))
                        .map(|k| k.0)
                        .collect(),
                    100,
                )
            },
        ),
        d(
            "kills-500",
            tr!("Slayer"),
            tr!("Kill 500 enemies."),
            135358,
            None,
            Count,
            |m| nth(m.kills.iter().map(|k| k.0).collect(), 500),
        ),
        d(
            "quests-50",
            tr!("Taskmaster"),
            tr!("Turn in 50 quests."),
            134327,
            None,
            Count,
            |m| {
                nth(
                    m.c.quests
                        .iter()
                        .filter_map(|q| q.done.map(|t| t as f64))
                        .collect(),
                    50,
                )
            },
        ),
        d(
            "explore-15",
            tr!("Wanderer"),
            tr!("Discover 15 places on the map."),
            134269,
            None,
            Count,
            explored,
        ),
        d(
            "swift",
            tr!("Swift Errand"),
            tr!("Turn in a quest within five minutes of taking it."),
            132307,
            None,
            Count,
            |m| {
                let t =
                    m.c.quests
                        .iter()
                        .filter_map(|q| Some((q.accepted?, q.done?)))
                        .filter(|(a, d)| d - a <= 300)
                        .map(|(_, d)| d as f64)
                        .reduce(f64::min);
                once(t)
            },
        ),
        d(
            "rare",
            tr!("Something Blue"),
            tr!("Hold a rare item of your own."),
            134132,
            None,
            Count,
            rare,
        ),
        d(
            "gold-10",
            tr!("A Heavy Purse"),
            tr!("Carry 10 gold at once."),
            133785,
            None,
            Copper,
            gold,
        ),
        d(
            "long-day",
            tr!("Lost in the World"),
            tr!("Spend three hours in the world in a single day."),
            136106,
            None,
            Seconds,
            long_day,
        ),
        d(
            "friends",
            tr!("Friends Along the Way"),
            tr!("Group with 10 different people."),
            135943,
            None,
            Count,
            friends,
        ),
        // ---- Paladin (Forever: Holy Strike, Seal of Fury, Righteous Fury, Retribution Aura, Consecration at 20)
        d(
            "pal-loh",
            tr!("Hands of Mercy"),
            tr!("Use Lay on Hands, when nothing else would do."),
            135928,
            pal,
            Count,
            |m| nth(m.uses("Lay on Hands"), 1),
        ),
        d(
            "pal-light",
            tr!("The Light Answers"),
            tr!("Heal yourself 100 times with Holy Light."),
            135920,
            pal,
            Count,
            |m| {
                nth(
                    m.spell_hits(&m.healed, "Holy Light")
                        .filter(|h| h.src == h.dst)
                        .map(|h| h.t)
                        .collect(),
                    100,
                )
            },
        ),
        d(
            "pal-consecration",
            tr!("Consecrated Ground"),
            tr!("Burn three enemies with a single pulse of Consecration."),
            135926,
            pal,
            Count,
            |m| aoe(m, "Consecration", 3),
        ),
        d(
            "pal-tank",
            tr!("Seal of Fury"),
            tr!(
                "Hold the front through a dungeon run: 20 foes felled, and their blows on your shield."
            ),
            135962,
            pal,
            Count,
            tank,
        ),
        d(
            "pal-strike",
            tr!("Strike True"),
            tr!("Land 250 Holy Strikes."),
            135924,
            pal,
            Count,
            |m| {
                nth(
                    m.spell_hits(&m.dealt, "Holy Strike").map(|h| h.t).collect(),
                    250,
                )
            },
        ),
        d(
            "pal-retribution",
            tr!("Thorns of the Light"),
            tr!("Let Retribution Aura punish attackers for 1,000 damage."),
            135873,
            pal,
            Count,
            |m| damage(m, "Retribution Aura", 1000.0),
        ),
        // ---- Warrior
        d(
            "war-charge",
            tr!("Headlong"),
            tr!("Charge into battle 100 times."),
            132337,
            war,
            Count,
            |m| nth(m.uses("Charge"), 100),
        ),
        d(
            "war-execute",
            tr!("Executioner"),
            tr!("Finish 25 enemies with Execute."),
            135358,
            war,
            Count,
            |m| nth(m.killing_blows("Execute"), 25),
        ),
        d(
            "war-thunder",
            tr!("Thunderstruck"),
            tr!("Catch four enemies in one Thunder Clap."),
            132326,
            war,
            Count,
            |m| aoe(m, "Thunder Clap", 4),
        ),
        d(
            "war-shout",
            tr!("Rallying Cry"),
            tr!("Sound Battle Shout 100 times."),
            132333,
            war,
            Count,
            |m| nth(m.uses("Battle Shout"), 100),
        ),
        d(
            "war-rend",
            tr!("Bleed Them Dry"),
            tr!("Deal 5,000 damage with Rend."),
            132155,
            war,
            Count,
            |m| damage(m, "Rend", 5000.0),
        ),
        d(
            "war-overpower",
            tr!("No Second Chances"),
            tr!("Answer a dodge with Overpower 50 times."),
            132223,
            war,
            Count,
            |m| nth(m.uses("Overpower"), 50),
        ),
        // ---- Hunter
        d(
            "hun-tame",
            tr!("Beast Friend"),
            tr!("Tame a beast of the wild."),
            132164,
            hun,
            Count,
            |m| nth(m.uses("Tame Beast"), 1),
        ),
        d(
            "hun-auto",
            tr!("Steady Hand"),
            tr!("Land 1,000 Auto Shots."),
            132212,
            hun,
            Count,
            |m| {
                nth(
                    m.spell_hits(&m.dealt, "Auto Shot").map(|h| h.t).collect(),
                    1000,
                )
            },
        ),
        d(
            "hun-serpent",
            tr!("Serpent's Kiss"),
            tr!("Deal 5,000 damage with Serpent Sting."),
            132204,
            hun,
            Count,
            |m| damage(m, "Serpent Sting", 5000.0),
        ),
        d(
            "hun-mark",
            tr!("Marked"),
            tr!("Place Hunter's Mark 100 times."),
            236188,
            hun,
            Count,
            |m| nth(m.uses("Hunter's Mark"), 100),
        ),
        d(
            "hun-feign",
            tr!("Playing Dead"),
            tr!("Feign Death 10 times."),
            132293,
            hun,
            Count,
            |m| nth(m.uses("Feign Death"), 10),
        ),
        d(
            "hun-crit",
            tr!("Eagle Eye"),
            tr!("Land 100 critical shots."),
            132169,
            hun,
            Count,
            |m| {
                let shots: HashSet<u32> = [
                    "Auto Shot",
                    "Arcane Shot",
                    "Aimed Shot",
                    "Multi-Shot",
                    "Concussive Shot",
                ]
                .iter()
                .filter_map(|s| m.spell(s))
                .collect();
                nth(
                    m.dealt
                        .iter()
                        .filter(|h| h.crit && shots.contains(&h.spell))
                        .map(|h| h.t)
                        .collect(),
                    100,
                )
            },
        ),
        // ---- Rogue
        d(
            "rog-stealth",
            tr!("One with the Shadows"),
            tr!("Slip into Stealth 100 times."),
            132320,
            rog,
            Count,
            |m| nth(m.uses("Stealth"), 100),
        ),
        d(
            "rog-backstab",
            tr!("Backstabber"),
            tr!("Land 250 Backstabs."),
            132090,
            rog,
            Count,
            |m| {
                nth(
                    m.spell_hits(&m.dealt, "Backstab").map(|h| h.t).collect(),
                    250,
                )
            },
        ),
        d(
            "rog-sinister",
            tr!("A Thousand Cuts"),
            tr!("Land 1,000 Sinister Strikes."),
            136189,
            rog,
            Count,
            |m| {
                nth(
                    m.spell_hits(&m.dealt, "Sinister Strike")
                        .map(|h| h.t)
                        .collect(),
                    1000,
                )
            },
        ),
        d(
            "rog-eviscerate",
            tr!("Opened Up"),
            tr!("Finish 50 enemies with Eviscerate."),
            132292,
            rog,
            Count,
            |m| nth(m.killing_blows("Eviscerate"), 50),
        ),
        d(
            "rog-pickpocket",
            tr!("Light Fingers"),
            tr!("Pick 50 pockets."),
            133644,
            rog,
            Count,
            |m| nth(m.uses("Pick Pocket"), 50),
        ),
        d(
            "rog-vanish",
            tr!("Now You See Me"),
            tr!("Vanish."),
            132331,
            rog,
            Count,
            |m| nth(m.uses("Vanish"), 1),
        ),
        // ---- Priest
        d(
            "pri-shield",
            tr!("Bulwark of Faith"),
            tr!("Cast Power Word: Shield 100 times."),
            135940,
            pri,
            Count,
            |m| nth(m.uses("Power Word: Shield"), 100),
        ),
        d(
            "pri-mender",
            tr!("Mender of Others"),
            tr!("Heal others for 25,000."),
            135915,
            pri,
            Count,
            |m| {
                sum_to(
                    m.healed
                        .iter()
                        .filter(|h| h.src != h.dst)
                        .map(|h| (h.t, (h.amount - h.over) as f64))
                        .collect(),
                    25000.0,
                )
            },
        ),
        d(
            "pri-renew",
            tr!("Renewal"),
            tr!("Heal 5,000 with Renew."),
            135953,
            pri,
            Count,
            |m| healing(m, "Renew", 5000.0),
        ),
        d(
            "pri-smite",
            tr!("Smite the Wicked"),
            tr!("Land 500 Smites."),
            135924,
            pri,
            Count,
            |m| nth(m.spell_hits(&m.dealt, "Smite").map(|h| h.t).collect(), 500),
        ),
        d(
            "pri-pain",
            tr!("Word of Pain"),
            tr!("Deal 5,000 damage with Shadow Word: Pain."),
            136207,
            pri,
            Count,
            |m| damage(m, "Shadow Word: Pain", 5000.0),
        ),
        d(
            "pri-res",
            tr!("Back from Beyond"),
            tr!("Bring a fallen friend back with Resurrection."),
            135955,
            pri,
            Count,
            |m| nth(m.uses("Resurrection"), 1),
        ),
        // ---- Shaman
        d(
            "sha-bolt",
            tr!("Stormcaller"),
            tr!("Land 500 Lightning Bolts."),
            136048,
            sha,
            Count,
            |m| {
                nth(
                    m.spell_hits(&m.dealt, "Lightning Bolt")
                        .map(|h| h.t)
                        .collect(),
                    500,
                )
            },
        ),
        d(
            "sha-shock",
            tr!("Earthshaker"),
            tr!("Land 250 Earth Shocks."),
            136026,
            sha,
            Count,
            |m| {
                nth(
                    m.spell_hits(&m.dealt, "Earth Shock").map(|h| h.t).collect(),
                    250,
                )
            },
        ),
        d(
            "sha-wave",
            tr!("Healing Waters"),
            tr!("Heal 10,000 with Healing Wave."),
            136052,
            sha,
            Count,
            |m| healing(m, "Healing Wave", 10000.0),
        ),
        d(
            "sha-totems",
            tr!("Keeper of the Totems"),
            tr!("Set down 200 totems."),
            136098,
            sha,
            Count,
            |m| {
                nth(
                    m.casts
                        .iter()
                        .filter(|h| m.cb.spell_name(h.spell).contains("Totem"))
                        .map(|h| h.t)
                        .collect(),
                    200,
                )
            },
        ),
        d(
            "sha-shield",
            tr!("Crackling Ward"),
            tr!("Deal 2,000 damage with Lightning Shield."),
            136051,
            sha,
            Count,
            |m| damage(m, "Lightning Shield", 2000.0),
        ),
        d(
            "sha-spirit",
            tr!("Voice of the Ancestors"),
            tr!("Call a fallen friend back with Ancestral Spirit."),
            136077,
            sha,
            Count,
            |m| nth(m.uses("Ancestral Spirit"), 1),
        ),
        // ---- Mage
        d(
            "mag-fireball",
            tr!("Fireball!"),
            tr!("Land 500 Fireballs."),
            135812,
            mag,
            Count,
            |m| {
                nth(
                    m.spell_hits(&m.dealt, "Fireball").map(|h| h.t).collect(),
                    500,
                )
            },
        ),
        d(
            "mag-nova",
            tr!("Cold Snap"),
            tr!("Freeze three enemies with one Frost Nova."),
            135848,
            mag,
            Count,
            |m| aoe(m, "Frost Nova", 3),
        ),
        d(
            "mag-sheep",
            tr!("Sheep Happens"),
            tr!("Polymorph 25 enemies."),
            136071,
            mag,
            Count,
            |m| nth(m.uses("Polymorph"), 25),
        ),
        d(
            "mag-missiles",
            tr!("Arcane Barrage"),
            tr!("Land 1,000 Arcane Missiles."),
            136096,
            mag,
            Count,
            |m| {
                nth(
                    m.spell_hits(&m.dealt, "Arcane Missiles")
                        .map(|h| h.t)
                        .collect(),
                    1000,
                )
            },
        ),
        d(
            "mag-conjure",
            tr!("Refreshments Provided"),
            tr!("Conjure food or water 50 times."),
            132805,
            mag,
            Count,
            |m| {
                nth(
                    m.casts
                        .iter()
                        .filter(|h| m.cb.spell_name(h.spell).starts_with("Conjure"))
                        .map(|h| h.t)
                        .collect(),
                    50,
                )
            },
        ),
        d(
            "mag-blink",
            tr!("Now Here, Now There"),
            tr!("Blink 100 times."),
            135736,
            mag,
            Count,
            |m| nth(m.uses("Blink"), 100),
        ),
        // ---- Warlock
        d(
            "wlk-bolt",
            tr!("Shadow Weaver"),
            tr!("Land 500 Shadow Bolts."),
            136197,
            wlk,
            Count,
            |m| {
                nth(
                    m.spell_hits(&m.dealt, "Shadow Bolt").map(|h| h.t).collect(),
                    500,
                )
            },
        ),
        d(
            "wlk-corruption",
            tr!("Rot from Within"),
            tr!("Deal 10,000 damage with Corruption."),
            136118,
            wlk,
            Count,
            |m| damage(m, "Corruption", 10000.0),
        ),
        d(
            "wlk-soul",
            tr!("Soul Harvest"),
            tr!("Finish 25 enemies with Drain Soul."),
            136163,
            wlk,
            Count,
            |m| nth(m.killing_blows("Drain Soul"), 25),
        ),
        d(
            "wlk-tap",
            tr!("Blood for Power"),
            tr!("Life Tap 100 times."),
            136126,
            wlk,
            Count,
            |m| nth(m.uses("Life Tap"), 100),
        ),
        d(
            "wlk-fear",
            tr!("Fear Itself"),
            tr!("Send 50 enemies running with Fear."),
            136183,
            wlk,
            Count,
            |m| nth(m.uses("Fear"), 50),
        ),
        d(
            "wlk-demons",
            tr!("Demonologist"),
            tr!("Summon three different demons."),
            136218,
            wlk,
            Count,
            demons,
        ),
        // ---- Druid
        d(
            "dru-bear",
            tr!("Heart of the Bear"),
            tr!("Take on Bear Form."),
            132276,
            dru,
            Count,
            |m| nth(m.uses("Bear Form"), 1),
        ),
        d(
            "dru-cat",
            tr!("Prowler"),
            tr!("Take on Cat Form."),
            132115,
            dru,
            Count,
            |m| nth(m.uses("Cat Form"), 1),
        ),
        d(
            "dru-wrath",
            tr!("Nature's Wrath"),
            tr!("Land 500 Wraths."),
            136006,
            dru,
            Count,
            |m| nth(m.spell_hits(&m.dealt, "Wrath").map(|h| h.t).collect(), 500),
        ),
        d(
            "dru-moonfire",
            tr!("Moonkissed"),
            tr!("Land 250 Moonfires."),
            136096,
            dru,
            Count,
            |m| {
                nth(
                    m.spell_hits(&m.dealt, "Moonfire").map(|h| h.t).collect(),
                    250,
                )
            },
        ),
        d(
            "dru-roots",
            tr!("Rooted"),
            tr!("Hold 50 enemies in Entangling Roots."),
            136100,
            dru,
            Count,
            |m| nth(m.uses("Entangling Roots"), 50),
        ),
        d(
            "dru-rejuv",
            tr!("Evergreen"),
            tr!("Heal 5,000 with Rejuvenation."),
            136081,
            dru,
            Count,
            |m| healing(m, "Rejuvenation", 5000.0),
        ),
    ]
}

/// The deeds that apply to a character: its class's first, then everyone's.
pub fn for_class(class_file: &str) -> Vec<Deed> {
    let mut out: Vec<Deed> = all()
        .into_iter()
        .filter(|d| d.class.is_none_or(|c| c == class_file))
        .collect();
    out.sort_by_key(|d| d.class.is_none());
    out
}

/// Every deed's icon, for fetching the art ahead.
pub fn icons() -> Vec<i64> {
    all().iter().map(|d| d.icon).collect()
}

/// One character's share of the combat log, in time order.
pub struct Mine<'a> {
    pub c: &'a Character,
    pub cb: &'a Combat,
    pub dealt: Vec<&'a Hit>,
    pub taken: Vec<&'a Hit>,
    pub healed: Vec<&'a Hit>,
    pub casts: Vec<&'a Hit>,
    pub kills: Vec<(f64, u32)>,
    pub deaths: Vec<f64>,
}

impl<'a> Mine<'a> {
    pub fn new(m: &'a Model, c: &'a Character) -> Self {
        let cb = &m.combat;
        let me = cb
            .units
            .iter()
            .position(|u| u.guid == c.guid)
            .map(|i| i as u32);
        // Kills and deaths carry no owner: they are this character's when
        // they fall inside its sessions.
        let during = |t: f64| {
            let i = c.sessions.partition_point(|s| s.start as f64 - 60.0 <= t);
            c.sessions.is_empty() || (i > 0 && t <= c.sessions[i - 1].end as f64 + 60.0)
        };
        let pick = |v: &'a [Hit], by_src: bool| -> Vec<&'a Hit> {
            let mut out: Vec<&Hit> = v
                .iter()
                .filter(|h| Some(if by_src { h.src } else { h.dst }) == me)
                .collect();
            out.sort_by(|a, b| a.t.total_cmp(&b.t));
            out
        };
        let mut kills: Vec<(f64, u32)> = if me.is_some() {
            cb.kills
                .iter()
                .copied()
                .filter(|(t, _)| during(*t))
                .collect()
        } else {
            vec![]
        };
        kills.sort_by(|a, b| a.0.total_cmp(&b.0));
        Mine {
            c,
            cb,
            dealt: pick(&cb.dealt, true),
            taken: pick(&cb.taken, false),
            healed: pick(&cb.healed, true),
            casts: pick(&cb.casts, true),
            kills,
            deaths: if me.is_some() {
                cb.deaths.iter().copied().filter(|t| during(*t)).collect()
            } else {
                vec![]
            },
        }
    }

    fn spell(&self, name: &str) -> Option<u32> {
        self.cb
            .spells
            .iter()
            .position(|s| s == name)
            .map(|i| i as u32)
    }

    fn spell_hits<'s>(
        &self,
        v: &'s [&'a Hit],
        name: &str,
    ) -> impl Iterator<Item = &'s &'a Hit> + 's {
        let id = self.spell(name);
        v.iter().filter(move |h| Some(h.spell) == id)
    }

    /// When a spell was used: its casts, or failing those its hits and heals.
    fn uses(&self, name: &str) -> Vec<f64> {
        let casts: Vec<f64> = self.spell_hits(&self.casts, name).map(|h| h.t).collect();
        if !casts.is_empty() {
            return casts;
        }
        self.spell_hits(&self.dealt, name)
            .chain(self.spell_hits(&self.healed, name))
            .filter(|h| h.amount > 0)
            .map(|h| h.t)
            .collect()
    }

    /// Kills whose last blow from me was this spell.
    fn killing_blows(&self, name: &str) -> Vec<f64> {
        let Some(id) = self.spell(name) else {
            return vec![];
        };
        let mut last: HashMap<u32, u32> = HashMap::new();
        let mut hits = self.dealt.iter().peekable();
        let mut out = vec![];
        for &(t, u) in &self.kills {
            while let Some(h) = hits.next_if(|h| h.t <= t) {
                last.insert(h.dst, h.spell);
            }
            if last.get(&u) == Some(&id) {
                out.push(t);
            }
        }
        out
    }

    fn event_times(&self, kind: &str) -> Vec<f64> {
        self.c
            .events
            .iter()
            .filter(|e| e.e == kind)
            .map(|e| e.t as f64)
            .collect()
    }

    /// Dungeon runs with a group: entering a dungeon while grouped, to
    /// leaving it or the group.
    pub fn runs(&self) -> Vec<Run> {
        let mut out: Vec<Run> = vec![];
        let (mut grouped, mut open): (bool, Option<f64>) = (false, None);
        let mut close = |open: &mut Option<f64>, end: f64| {
            if let Some(start) = open.take() {
                out.push(Run { start, end });
            }
        };
        for e in &self.c.events {
            match e.e.as_str() {
                "group" => {
                    let n =
                        e.v.get("members")
                            .map(super::memory::entries)
                            .map_or(0, |m| m.len());
                    grouped = n >= 3;
                    if !grouped {
                        close(&mut open, e.t as f64);
                    }
                }
                "zone" | "login" => {
                    let inside = e.s("zone").is_some_and(|z| DUNGEONS.contains(&z));
                    if !inside {
                        close(&mut open, e.t as f64);
                    } else if grouped && open.is_none() {
                        open = Some(e.t as f64);
                    }
                }
                "logout" => close(&mut open, e.t as f64),
                _ => {}
            }
        }
        if let Some(e) = self.c.events.last() {
            close(&mut open, e.t as f64);
        }
        out
    }

    fn count_in<T>(v: &[T], at: impl Fn(&T) -> f64, from: f64, to: f64) -> usize {
        v.partition_point(|x| at(x) <= to) - v.partition_point(|x| at(x) < from)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Run {
    pub start: f64,
    pub end: f64,
}

// ---- evaluators

/// Earned at the need-th occurrence.
fn nth(mut times: Vec<f64>, need: usize) -> Status {
    times.sort_by(f64::total_cmp);
    match times.get(need - 1) {
        Some(t) => Status::Earned(Some(*t)),
        None => Status::Progress {
            have: times.len() as f64,
            need: need as f64,
        },
    }
}

fn once(t: Option<f64>) -> Status {
    match t {
        Some(t) => Status::Earned(Some(t)),
        None => Status::Progress {
            have: 0.0,
            need: 1.0,
        },
    }
}

/// Earned when the running total reaches `need`.
fn sum_to(mut v: Vec<(f64, f64)>, need: f64) -> Status {
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut total = 0.0;
    for (t, x) in v {
        total += x;
        if total >= need {
            return Status::Earned(Some(t));
        }
    }
    Status::Progress { have: total, need }
}

fn damage(m: &Mine, spell: &str, need: f64) -> Status {
    sum_to(
        m.spell_hits(&m.dealt, spell)
            .map(|h| (h.t, h.amount as f64))
            .collect(),
        need,
    )
}

fn healing(m: &Mine, spell: &str, need: f64) -> Status {
    sum_to(
        m.spell_hits(&m.healed, spell)
            .map(|h| (h.t, (h.amount - h.over) as f64))
            .collect(),
        need,
    )
}

/// The most enemies one application of a spell hit (hits within a tenth of
/// a second count as one pulse).
fn aoe(m: &Mine, spell: &str, need: usize) -> Status {
    let hits: Vec<&&Hit> = m.spell_hits(&m.dealt, spell).collect();
    let mut best = 0;
    let mut i = 0;
    while i < hits.len() {
        let mut j = i;
        let mut targets = HashSet::new();
        while j < hits.len() && hits[j].t - hits[i].t <= 0.1 {
            targets.insert(hits[j].dst);
            j += 1;
        }
        if targets.len() >= need {
            return Status::Earned(Some(hits[i].t));
        }
        best = best.max(targets.len());
        i = j;
    }
    Status::Progress {
        have: best as f64,
        need: need as f64,
    }
}

fn level(m: &Mine, need: i64) -> Status {
    let t =
        m.c.events
            .iter()
            .find(|e| e.e == "level" && e.i("level").unwrap_or(0) >= need)
            .map(|e| e.t as f64);
    match t {
        Some(t) => Status::Earned(Some(t)),
        None if m.c.level >= need => Status::Earned(None),
        None => Status::Progress {
            have: m.c.level as f64,
            need: need as f64,
        },
    }
}

fn is_undead(name: &str) -> bool {
    let n = name.to_lowercase();
    UNDEAD.iter().any(|w| n.contains(w))
}

fn payback(m: &Mine) -> Status {
    for &d in &m.deaths {
        let killer = m
            .taken
            .iter()
            .rev()
            .skip_while(|h| h.t > d)
            .take_while(|h| d - h.t <= 30.0)
            .find(|h| {
                m.cb.units
                    .get(h.src as usize)
                    .is_some_and(|u| u.kind == UnitKind::Creature)
            });
        let Some(k) = killer else { continue };
        let name = m.cb.unit_name(k.src);
        if let Some((t, _)) = m
            .kills
            .iter()
            .find(|(t, u)| *t > d && *t - d <= 1800.0 && m.cb.unit_name(*u) == name)
        {
            return Status::Earned(Some(*t));
        }
    }
    Status::Progress {
        have: 0.0,
        need: 1.0,
    }
}

fn explored(m: &Mine) -> Status {
    let mut seen = HashSet::new();
    let mut times = vec![];
    for e in m.c.events.iter().filter(|e| e.e == "explore") {
        let place = e.s("sub").filter(|s| !s.is_empty()).or(e.s("zone"));
        if let Some(p) = place
            && seen.insert(p)
        {
            times.push(e.t as f64);
        }
    }
    nth(times, 15)
}

fn rare(m: &Mine) -> Status {
    let from_bags =
        m.c.events
            .iter()
            .filter(|e| e.e == "item" && e.i("d").unwrap_or(0) > 0)
            .filter(|e| {
                e.s("link")
                    .and_then(parse_link)
                    .and_then(|l| l.quality)
                    .is_some_and(|q| q >= 3)
            })
            .map(|e| e.t as f64);
    let worn =
        m.c.snapshot
            .get("equipment")
            .map(super::memory::entries)
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, it)| it.get("quality").and_then(Value::as_i64).unwrap_or(0) >= 3)
            .map(|(_, it)| it.get("firstSeen").and_then(Value::as_i64).unwrap_or(0));
    let worn: Vec<i64> = worn.collect();
    match from_bags.reduce(f64::min) {
        Some(t) => Status::Earned(Some(t)),
        None if !worn.is_empty() => {
            Status::Earned(worn.into_iter().filter(|t| *t > 0).min().map(|t| t as f64))
        }
        None => Status::Progress {
            have: 0.0,
            need: 1.0,
        },
    }
}

fn gold(m: &Mine) -> Status {
    const NEED: i64 = 10 * 10000;
    let mut best = m.c.money;
    for e in &m.c.events {
        let total = match e.e.as_str() {
            "money" => e.i("total"),
            "login" | "logout" => e.i("money"),
            _ => None,
        };
        if let Some(total) = total {
            if total >= NEED {
                return Status::Earned(Some(e.t as f64));
            }
            best = best.max(total);
        }
    }
    if best >= NEED {
        return Status::Earned(None);
    }
    Status::Progress {
        have: best as f64,
        need: NEED as f64,
    }
}

fn long_day(m: &Mine) -> Status {
    const NEED: i64 = 3 * 3600;
    let mut days: HashMap<chrono::NaiveDate, i64> = HashMap::new();
    let mut best = 0;
    for s in &m.c.sessions {
        let day = crate::theme::local(s.start as f64).date_naive();
        let before = days.get(&day).copied().unwrap_or(0);
        let after = before + s.seconds();
        if after >= NEED {
            return Status::Earned(Some((s.start + NEED - before) as f64));
        }
        days.insert(day, after);
        best = best.max(after);
    }
    Status::Progress {
        have: best as f64,
        need: NEED as f64,
    }
}

fn friends(m: &Mine) -> Status {
    let me = m.c.name.split(' ').next().unwrap_or("").to_lowercase();
    let mut seen = HashSet::new();
    let mut times = vec![];
    for e in m.c.events.iter().filter(|e| e.e == "group") {
        let members =
            e.v.get("members")
                .map(super::memory::entries)
                .unwrap_or_default();
        for (_, v) in members {
            let Some(name) = v.as_str() else { continue };
            let first = name.split('-').next().unwrap_or(name).to_lowercase();
            if first != me && !first.is_empty() && seen.insert(first) {
                times.push(e.t as f64);
            }
        }
    }
    nth(times, 10)
}

fn tank(m: &Mine) -> Status {
    const KILLS: usize = 20;
    let mut best = 0;
    for r in m.runs() {
        let kills = Mine::count_in(&m.kills, |k| k.0, r.start, r.end);
        let blows = Mine::count_in(&m.taken, |h| h.t, r.start, r.end);
        if kills >= KILLS && blows >= kills {
            return Status::Earned(Some(r.end));
        }
        best = best.max(kills);
    }
    Status::Progress {
        have: best as f64,
        need: KILLS as f64,
    }
}

fn demons(m: &Mine) -> Status {
    let mut times = vec![];
    for d in [
        "Summon Imp",
        "Summon Voidwalker",
        "Summon Succubus",
        "Summon Felhunter",
    ] {
        if let Some(t) = m.uses(d).into_iter().reduce(f64::min) {
            times.push(t);
        }
    }
    nth(times, 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds() {
        assert_eq!(nth(vec![3.0, 1.0, 2.0], 2), Status::Earned(Some(2.0)));
        assert_eq!(
            nth(vec![1.0], 3),
            Status::Progress {
                have: 1.0,
                need: 3.0
            }
        );
        assert_eq!(
            sum_to(vec![(2.0, 50.0), (1.0, 60.0)], 100.0),
            Status::Earned(Some(2.0))
        );
        assert!(is_undead("Rattlecage Skeleton") && !is_undead("Mudsnout Gnoll"));
    }

    #[test]
    fn every_class_has_six() {
        let deeds = all();
        let mut ids = HashSet::new();
        for d in &deeds {
            assert!(ids.insert(d.id), "duplicate deed {}", d.id);
        }
        for class in [
            "WARRIOR", "PALADIN", "HUNTER", "ROGUE", "PRIEST", "SHAMAN", "MAGE", "WARLOCK", "DRUID",
        ] {
            assert_eq!(
                deeds.iter().filter(|d| d.class == Some(class)).count(),
                6,
                "{class}"
            );
        }
    }
}
