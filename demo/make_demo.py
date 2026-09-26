#!/usr/bin/env python3
"""Dummy data for screenshots and videos: Tom Crusader, a level 20 Undead
Protection Paladin in WoW Forever, and three level 20 alts with Forever's new
race and class combos. Nothing here comes from a real account.

    demo/make_demo.py OUT_DIR [--game "/path/to/World of Warcraft"]

Writes OUT_DIR/archive (the archive the recorder would write), OUT_DIR/logs
(archived native combat and chat logs), OUT_DIR/game (a stand-in game folder
that borrows only the game data, for the art) and OUT_DIR/config and
OUT_DIR/data (dummy settings). Run the app against it with
OUT_DIR/run.sh [args]; screenshots: FM_SHOT=x.png FM_PAGE=map OUT_DIR/run.sh.

The journey follows the Forsaken paladin's road as described for the beta
(Wowhead's Forever guides): Deathknell, Brill, Bandarion Keep in the
Whispering Wood, the Sepulcher in Silverpine and the Ruins of Lordaeron.
Holy Strike at 6, Seal of Fury, Righteous Fury and Retribution Aura at 16,
Consecration at 20.
"""

import json, math, os, random, shutil, sys
from datetime import datetime, timedelta

rng = random.Random(20261104)
rng2 = random.Random(7)  # for later additions, so the journey above stays the same
OUT = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else "demo-out")
GAME = os.path.expanduser("~/Games/battlenet/drive_c/Program Files (x86)/World of Warcraft")
if "--game" in sys.argv:
    GAME = sys.argv[sys.argv.index("--game") + 1]
TEMPLATE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "template-snapshot.json")

ME = "Player-4613-0B7C21E4"
ME_NAME = "Tom-Bandarion"
REALM = "Bandarion"

# ---------------------------------------------------------------- the world

# Areas: map, centre (normalized map coords), spread, mobs (name, level, npc id, undead).
AREAS = {
    "deathknell": (1420, (0.322, 0.662), 0.012, []),
    "dk_graves": (1420, (0.347, 0.628), 0.02, [("Mindless Zombie", 1, 1501, True), ("Wretched Zombie", 2, 1502, True)]),
    "dk_rattle": (1420, (0.298, 0.607), 0.018, [("Rattlecage Skeleton", 3, 1890, True)]),
    "nightweb": (1420, (0.268, 0.586), 0.016, [("Young Night Web Spider", 3, 1504, False), ("Night Web Spider", 4, 1505, False)]),
    "solliden": (1420, (0.382, 0.700), 0.02, [("Scarlet Convert", 4, 1506, False), ("Scarlet Initiate", 5, 1507, False)]),
    "brill": (1420, (0.607, 0.522), 0.012, []),
    "fields": (1420, (0.556, 0.468), 0.025, [("Rot Hide Graverobber", 7, 1941, False), ("Rot Hide Mongrel", 8, 1674, False)]),
    "agamand": (1420, (0.470, 0.322), 0.025, [("Rattlecage Soldier", 7, 1520, True), ("Darkeye Bonecaster", 8, 1522, True), ("Cracked Skull Soldier", 9, 1523, True)]),
    "garren": (1420, (0.576, 0.336), 0.018, [("Rot Hide Mystic", 9, 1675, False), ("Rot Hide Brute", 10, 1939, False)]),
    "scarletwatch": (1420, (0.772, 0.458), 0.02, [("Scarlet Warrior", 9, 1535, False), ("Scarlet Missionary", 10, 1536, False)]),
    "northcoast": (1420, (0.332, 0.152), 0.03, [("Murloc Coastrunner", 9, 1544, False), ("Murloc Streamrunner", 10, 1545, False)]),
    "bandarion": (1420, (0.216, 0.236), 0.012, []),
    "whispering": (1420, (0.182, 0.312), 0.025, [("Blighted Wolf", 10, 248811, False), ("Whispering Wood Lurker", 11, 248812, True)]),
    "sepulcher": (1421, (0.445, 0.420), 0.012, []),
    "deadfield": (1421, (0.456, 0.214), 0.025, [("Mudsnout Gnoll", 12, 1782, False), ("Mudsnout Shaman", 13, 1783, False)]),
    "maldens": (1421, (0.380, 0.152), 0.022, [("Moss Stalker", 12, 1780, False), ("Rot Hide Plague Weaver", 13, 1795, False)]),
    "ivar": (1421, (0.540, 0.262), 0.015, [("Grizzled Worg", 13, 1766, False), ("Ivar's Hound", 14, 1767, False)]),
    "fenris": (1421, (0.652, 0.302), 0.022, [("Rot Hide Savage", 14, 1674, False), ("Rot Hide Plague Weaver", 15, 1795, False)]),
    "olsens": (1421, (0.470, 0.532), 0.022, [("Worg", 14, 1765, False), ("Mottled Worg", 15, 1766, False)]),
    "ambermill": (1421, (0.620, 0.640), 0.02, [("Dalaran Apprentice", 15, 1867, False), ("Dalaran Protector", 16, 1912, False)]),
    "pyrewood": (1421, (0.466, 0.730), 0.022, [("Moonrage Whitescalp", 16, 1769, False), ("Moonrage Darksoul", 17, 1770, False)]),
    "shadowfang": (1421, (0.445, 0.680), 0.015, [("Moonrage Glutton", 18, 1782, False), ("Moonrage Sentry", 19, 1896, False)]),
    "ruins": (None, (0, 0), 0, [("Blighted Ghoul", 17, 248901, True), ("Plagued Abomination", 18, 248902, True), ("Lordaeron Necromancer", 18, 248903, False)]),
}
ZONES = {1420: "Tirisfal Glades", 1421: "Silverpine Forest"}

# The roads, as waypoints; travel follows them like a player would.
ROADS = {
    1420: ({
        "dk": (0.322, 0.662), "dkgate": (0.365, 0.645), "r1": (0.420, 0.610), "r2": (0.480, 0.575), "r3": (0.540, 0.548),
        "brill": (0.607, 0.522), "bn": (0.605, 0.455), "garren": (0.578, 0.370), "fork": (0.525, 0.365),
        "agamand": (0.472, 0.330), "nr": (0.420, 0.270), "coast": (0.345, 0.185), "bk1": (0.290, 0.225),
        "bandarion": (0.216, 0.236), "ww": (0.190, 0.300), "be": (0.680, 0.505), "watch": (0.770, 0.460),
        "ucx": (0.615, 0.600), "uc": (0.625, 0.655), "solliden": (0.380, 0.700),
    }, [("dk", "dkgate"), ("dkgate", "r1"), ("r1", "r2"), ("r2", "r3"), ("r3", "brill"), ("brill", "bn"), ("bn", "garren"),
        ("garren", "fork"), ("fork", "agamand"), ("agamand", "nr"), ("nr", "coast"), ("coast", "bk1"), ("bk1", "bandarion"),
        ("bandarion", "ww"), ("brill", "be"), ("be", "watch"), ("brill", "ucx"), ("ucx", "uc"), ("dkgate", "solliden")]),
    1421: ({
        "n": (0.660, 0.080), "n1": (0.585, 0.175), "n2": (0.525, 0.260), "ivar": (0.545, 0.268), "sj": (0.480, 0.395),
        "sep": (0.445, 0.420), "df": (0.470, 0.225), "mo": (0.400, 0.170), "s1": (0.495, 0.500), "ols": (0.470, 0.532),
        "s2": (0.480, 0.600), "s3": (0.470, 0.690), "pyre": (0.466, 0.730), "sfk": (0.445, 0.680), "am1": (0.555, 0.610),
        "amber": (0.620, 0.640), "fen1": (0.600, 0.320), "fen": (0.650, 0.300), "dem": (0.560, 0.440),
    }, [("n", "n1"), ("n1", "n2"), ("n2", "ivar"), ("n2", "sj"), ("sj", "sep"), ("n2", "df"), ("df", "mo"), ("sj", "s1"),
        ("s1", "ols"), ("s1", "s2"), ("s2", "s3"), ("s3", "pyre"), ("s3", "sfk"), ("s2", "am1"), ("am1", "amber"),
        ("n2", "fen1"), ("fen1", "fen"), ("s1", "dem")]),
}
SUBZONES = {
    "deathknell": "Deathknell", "dk_graves": "Deathknell", "dk_rattle": "Deathknell", "nightweb": "Night Web's Hollow",
    "solliden": "Solliden Farmstead", "brill": "Brill", "fields": "Brightwater Lake", "agamand": "Agamand Mills",
    "garren": "Garren's Haunt", "scarletwatch": "Scarlet Watch Post", "northcoast": "North Coast", "bandarion": "Bandarion Keep",
    "whispering": "Whispering Wood", "sepulcher": "The Sepulcher", "deadfield": "The Dead Field", "maldens": "Malden's Orchard",
    "ivar": "Ivar's Farm", "fenris": "Fenris Isle", "olsens": "Olsen's Farthing", "ambermill": "Ambermill",
    "pyrewood": "Pyrewood Village", "shadowfang": "Shadowfang Keep", "ruins": "Ruins of Lordaeron",
}

# Quests: id, title, level, giver area, target area, objectives [(mob or item, count)],
# giver, text, objective text, reward item (id, name, icon, quality) or None.
QUESTS = [
    (363, "Rude Awakening", 1, "deathknell", None, [], "Undertaker Mordo",
     "You rose from the grave with the others, but you did not rise the same. The Dark Lady has freed us from the Scourge, and Shadow Priest Sarvis waits in the chapel to set you on your path.",
     "Speak with Shadow Priest Sarvis in Deathknell.", None),
    (91208, "Coming to Terms", 1, "deathknell", None, [], "Shadow Priest Sarvis",
     "Among the risen there is one who clutches a hammer and prays to a Light that ought to have abandoned us. Find the frightened paladin before the others decide what to do with him.",
     "Find the Frightened Paladin in Deathknell.", None),
    (364, "The Mindless Ones", 2, "deathknell", "dk_graves", [("Mindless Zombie", 8), ("Wretched Zombie", 8)], "Executor Arren",
     "Not all who rise keep their minds. The mindless wander the graves of Deathknell, and they are a danger to everyone who still can think. Put them to rest.",
     "Kill 8 Mindless Zombies and 8 Wretched Zombies.", None),
    (3901, "Rattling the Rattlecages", 3, "deathknell", "dk_rattle", [("Rattlecage Skeleton", 12)], "Deathguard Saltain",
     "The Rattlecage skeletons in the old graveyard still answer to the Lich King. Break them. Twelve should quiet the place for a while.",
     "Destroy 12 Rattlecage Skeletons.", (2960, "Ragged Leather Vest", 135009, 1)),
    (98389, "A Light in the Darkness", 4, "deathknell", "nightweb", [("Young Night Web Spider", 6), ("Night Web Spider", 4)], "Aramis Hammerhand",
     "If you remember but one thing, know this: not even death can stop the Light. Some of the risen are still wrapped in the webs of Night Web's Hollow. Free them, and show them what you have found.",
     "Free 6 Webbed Victims in Night Web's Hollow for Aramis Hammerhand in Deathknell.", None),
    (381, "The Scarlet Crusade", 5, "deathknell", "solliden", [("Scarlet Convert", 10)], "Executor Arren",
     "The Scarlet Crusade has pitched its banners on Solliden Farmstead, close enough to see our chapel. Remind them whose land this is.",
     "Kill 10 Scarlet Converts.", (3272, "Zombie Skin Leggings", 134583, 2)),
    (91209, "Continue Your Training", 5, "deathknell", None, [], "Aramis Hammerhand",
     "Lordaeron may not be what it used to be, but our home still needs protection. Go to Brill in central Tirisfal Glades and meet with Shari Stilwell. Take extra care as a paladin traveling through Forsaken lands.",
     "Follow the road east to Brill. Report to Shari Stilwell.", None),
    (5481, "Gordo's Task", 6, "brill", "fields", [("Gloom Weed", 3)], "Gordo",
     "Gordo can't carry much any more. The weeds by the lake, the dark ones. Bring three. Master Apothecary needs them.",
     "Bring 3 Gloom Weed to Gordo.", None),
    (365, "Fields of Grief", 7, "brill", "fields", [("Rot Hide Graverobber", 8), ("Tirisfal Pumpkin", 10)], "Apothecary Johaan",
     "The fields by Brightwater Lake grow pumpkins still, and the gnolls dig in the graves between them. Bring me the pumpkins, and keep the Rot Hides from the dead.",
     "Kill 8 Rot Hide Graverobbers and bring 10 Tirisfal Pumpkins to Apothecary Johaan.", None),
    (362, "The Haunted Mills", 8, "brill", "agamand", [("Rattlecage Soldier", 10), ("Darkeye Bonecaster", 8)], "Coleman Farthing",
     "The Agamands were a proud family. Now their mills are full of Scourge bones that walk. Clear them out, and bring back what's left of the family's pride.",
     "Kill 10 Rattlecage Soldiers and 8 Darkeye Bonecasters at Agamand Mills.", (3319, "Short Sabre", 133480, 2)),
    (398, "Wanted: Maggot Eye", 10, "brill", "garren", [("Maggot Eye", 1)], "Executor Zygand",
     "The gnoll called Maggot Eye leads the Rot Hides at Garren's Haunt. Bring me his paw, and the reward on this poster is yours.",
     "Bring Maggot Eye's Paw to Executor Zygand in Brill.", (3322, "Wispy Cloak", 133763, 2)),
    (91210, "Murlocs at the Gates", 10, "bandarion", "northcoast", [("Murloc Coastrunner", 8), ("Murloc Streamrunner", 6)], "Breton Samuels",
     "Bandarion Keep has stood empty since before the plague. Now it is ours, and the murlocs of the North Coast have taken to raiding our gate at night. Drive them back into the sea.",
     "Kill 8 Murloc Coastrunners and 6 Murloc Streamrunners on the North Coast.", None),
    (91211, "Touring the Grounds", 11, "bandarion", None, [], "Breton Samuels",
     "You will train here, pray here and bleed here. Speak with Jorin Croge in the back room, Hilda the Breaker by the training dummies and Ander Solliden at the lookout tower, then report to Danitha Morr in the council room.",
     "Speak with Jorin Croge, Hilda the Breaker and Ander Solliden, then report to Danitha Morr.", None),
    (91212, "The Whispering Wood", 11, "bandarion", "whispering", [("Blighted Wolf", 10), ("Whispering Wood Lurker", 6)], "Danitha Morr",
     "The wood around the keep whispers with the voices of the unquiet dead, and the wolves have eaten from the plague pits. A paladin of Bandarion holds this ground. Hold it.",
     "Kill 10 Blighted Wolves and 6 Whispering Wood Lurkers.", (91401, "Bandarion Initiate's Mace", 133485, 2)),
    (374, "Proof of Demise", 10, "brill", "scarletwatch", [("Scarlet Warrior", 10)], "Deathguard Burgess",
     "The Crusade's Scarlet Watch Post watches us too closely. Bring back their insignia as proof that fewer of them watch tonight.",
     "Bring 10 Scarlet Insignia Rings to Deathguard Burgess.", None),
    (437, "The Dead Fields", 12, "sepulcher", "deadfield", [("Mudsnout Gnoll", 10), ("Mudsnout Shaman", 6)], "High Executor Hadrec",
     "The Mudsnout gnolls have made the Dead Field their own, and their shamans call on things best left buried. The Sepulcher needs that road clear.",
     "Kill 10 Mudsnout Gnolls and 6 Mudsnout Shamans at the Dead Field.", None),
    (421, "Prove Your Worth", 13, "sepulcher", "maldens", [("Moss Stalker", 10), ("Rot Hide Plague Weaver", 5)], "Dalar Dawnweaver",
     "Words are cheap in the Sepulcher. Go to Malden's Orchard and come back with something that proves you are worth the Dark Lady's time.",
     "Kill 10 Moss Stalkers and 5 Rot Hide Plague Weavers.", None),
    (423, "Ivar the Foul", 14, "sepulcher", "ivar", [("Ivar the Foul", 1), ("Ivar's Hound", 6)], "Rane Yorick",
     "Ivar was a farmer once. Now he is a butcher who keeps worgen hounds and remembers everyone who wronged him. Put an end to his remembering.",
     "Bring Ivar's Head to Rane Yorick.", (91402, "Deathguard Pauldrons", 135040, 2)),
    (428, "Lost Deathstalkers", 15, "sepulcher", "fenris", [("Rot Hide Savage", 8), ("Rot Hide Plague Weaver", 6)], "Deathstalker Faerleia",
     "Two of my Deathstalkers went to Fenris Isle and did not come back. The Rot Hides hold the island now. Find out what happened, and make them pay for it.",
     "Search Fenris Isle for the lost Deathstalkers.", None),
    (429, "Wand over Fist", 15, "sepulcher", "olsens", [("Worg", 10), ("Mottled Worg", 6)], "Shadow Priestess Malia",
     "The worgs of Olsen's Farthing hunt anything that walks, living or dead. Thin the pack.",
     "Kill 10 Worgs and 6 Mottled Worgs.", (91403, "Sepulcher Bracers", 132606, 2)),
    (91213, "Remember That I Love You", 16, "sepulcher", "ambermill", [("Dalaran Apprentice", 8), ("Dalaran Protector", 6)], "Deathguard Morris",
     "Before the plague I had a wife in Ambermill. The Dalaran mages hold it now. There was a locket. If it is still there, I would like to hold it once more.",
     "Recover the Tarnished Locket from Ambermill.", (91404, "Tarnished Locket", 133294, 2)),
    (91214, "Wrongly Blamed, Justly Corrected", 17, "sepulcher", "pyrewood", [("Moonrage Whitescalp", 10), ("Moonrage Darksoul", 6)], "Dalar Dawnweaver",
     "Pyrewood's worgen blame the Forsaken for every missing child. They are wrong. Correct them, and bring back the truth of what happens in that village at night.",
     "Kill 10 Moonrage Whitescalps and 6 Moonrage Darksouls in Pyrewood Village.", (91405, "Pathfinder's Clearway", 134583, 2)),
    (91215, "Unending Torment", 18, "brill", "ruins", [("Blighted Ghoul", 12), ("Lordaeron Necromancer", 6)], "Chancellor Amai",
     "Beneath the capital, in the ruins of the old Lordaeron, the plague still festers. The necromancers there raise our dead again and again. End their work.",
     "Kill 12 Blighted Ghouls and 6 Lordaeron Necromancers in the Ruins of Lordaeron.", None),
    (91216, "Abominable Creatures Slain", 19, "brill", "ruins", [("Plagued Abomination", 6), ("Baron Vardus", 1)], "Chancellor Amai",
     "Baron Vardus stitches abominations from the bodies of our people. Slay his creatures, and slay him.",
     "Kill 6 Plagued Abominations and Baron Vardus in the Ruins of Lordaeron.", (91406, "Baron's Signet", 133349, 3)),
    (1014, "Arugal Must Die", 20, "sepulcher", None, [], "Dalar Dawnweaver",
     "Archmage Arugal brought the worgen to Silverpine, and from Shadowfang Keep he still commands them. The Dark Lady wants his head. You will need friends for this.",
     "Kill Archmage Arugal in Shadowfang Keep and bring his head to Dalar Dawnweaver.", (6414, "Seal of Sylvanas", 133357, 2)),
]

BOSSES = {"Maggot Eye": (10, 1753, False), "Ivar the Foul": (14, 1814, False), "Baron Vardus": (20, 248910, True)}
QUEST_ITEMS = {"Gloom Weed": (134185, 2), "Tirisfal Pumpkin": (133952, 3)}  # icon, drops per mob pulled

# Loot the mobs drop: (item id, name, icon, quality).
LOOT = {
    False: [(3264, "Duskbat Wing", 134298, 0), (4865, "Ruined Pelt", 134366, 0), (3300, "Rabbit's Foot", 134295, 0),
            (4867, "Broken Scorpid Leg", 134294, 0), (2589, "Linen Cloth", 132889, 1), (2770, "Copper Ore", 134566, 1)],
    True: [(4864, "Minor Scorpion Venom Sac", 134340, 0), (2589, "Linen Cloth", 132889, 1), (4865, "Crumbling Bone", 133730, 0),
           (3300, "Rotting Flesh", 134339, 0)],
}
FOOD = (159, "Refreshing Spring Water", 132794, 1)
# Where rewards and drops go when put on (inventory slot IDs).
WEAR = {2960: 5, 3272: 7, 3319: 16, 3322: 15, 91401: 16, 91402: 3, 91403: 9, 91404: 2, 91405: 7, 91406: 12, 91413: 16}

# XP to reach the next level, levels 1..20.
XP_TO = [400, 900, 1400, 2100, 2800, 3600, 4500, 5400, 6500, 7600, 8800, 10100, 11400, 12900, 14400, 16000, 17700, 19400, 21300, 22900]

# Class abilities learned (level, spell id, name).
SPELLS = [(4, 20271, "Judgement"), (6, 91601, "Holy Strike"), (8, 1152, "Purify"), (8, 853, "Hammer of Justice"),
          (10, 633, "Lay on Hands"), (12, 91602, "Seal of Fury"), (14, 19740, "Blessing of Might"), (16, 25780, "Righteous Fury"),
          (16, 7294, "Retribution Aura"), (18, 5573, "Divine Protection"), (20, 26573, "Consecration"), (20, 879, "Exorcism")]

# The party in the Ruins of Lordaeron, and others met on the road:
# guid, name, surname, class file, race, level, guild.
PARTY = [
    ("Player-4613-0C11A2F0", "Mira", "Ashvale", "PRIEST", "Undead", 19, "Bandarion Vigil"),
    ("Player-4613-0C0E77B1", "Rakka", "Emberfang", "WARLOCK", "Orc", 20, "Bandarion Vigil"),
    ("Player-4613-0B98D3C4", "Sil", "Duskreed", "HUNTER", "Troll", 18, ""),
    ("Player-4613-0C2F10D9", "Juno", "Bramblecrest", "DRUID", "Tauren", 19, "Thunder Bluff Wayfarers"),
]
STRANGERS = [
    ("Player-4613-0A71C3E2", "Hollis", "Grave", "ROGUE", "Undead", 12, ""),
    ("Player-4613-0A8A2B40", "Ondra", "Coldmourn", "MAGE", "Undead", 9, ""),
    ("Player-4613-0B01FF12", "Grask", "Ironhide", "WARRIOR", "Orc", 14, "Bloodfang"),
    ("Player-4613-0B55E0A7", "Tarrow", "Mistwalker", "SHAMAN", "Tauren", 16, ""),
    ("Player-4613-0B7710CC", "Veya", "Nightbloom", "PRIEST", "Troll", 11, ""),
    ("Player-4613-0B80AB31", "Corvin", "Blackmere", "WARLOCK", "Undead", 17, "Bandarion Vigil"),
    ("Player-4613-0B9912DE", "Oska", "Stonetusk", "HUNTER", "Orc", 15, ""),
]
CLASS_NAME = {"PRIEST": "Priest", "WARLOCK": "Warlock", "HUNTER": "Hunter", "DRUID": "Druid", "ROGUE": "Rogue",
              "MAGE": "Mage", "WARRIOR": "Warrior", "SHAMAN": "Shaman", "PALADIN": "Paladin"}
CAST = {"PRIEST": ["Smite", "Lesser Heal", "Renew", "Power Word: Shield", "Power Word: Fortitude"],
        "WARLOCK": ["Shadow Bolt", "Immolate", "Corruption", "Curse of Agony", "Life Tap"],
        "HUNTER": ["Auto Shot", "Serpent Sting", "Arcane Shot", "Hunter's Mark"],
        "DRUID": ["Wrath", "Moonfire", "Rejuvenation", "Healing Touch", "Mark of the Wild"],
        "ROGUE": ["Sinister Strike", "Eviscerate"], "MAGE": ["Frostbolt", "Fireball", "Arcane Intellect"],
        "WARRIOR": ["Heroic Strike", "Battle Shout", "Rend"], "SHAMAN": ["Lightning Bolt", "Earth Shock", "Healing Wave"]}

GENERAL = [
    "anyone know where the frightened paladin went? cant find him in the chapel",
    "LF1M tank Ruins of Lordaeron, have heals",
    "the new keep in the NW is gorgeous",
    "undead paladin is such a vibe honestly",
    "where do i learn holy strike",
    "wts copper ore, 5s a stack",
    "Brill inn has the best music in the game, fight me",
    "is Maggot Eye up? been camping for 10 min",
    "careful at the Scarlet Watch Post, pats are nasty",
    "anyone doing Ivar the Foul?",
    "consecration at 20 is so good",
    "what does the Legacy tree even do",
    "Talented is huge if you have alts with professions",
    "the Whispering Wood is creepy at night",
    "gratz on 20!",
    "any blacksmiths around? need a Crusader's helm",
    "lf group Wrongly Blamed, Pyrewood",
    "those murlocs at the north coast respawn so fast",
]

# ----------------------------------------------------------------- helpers

def ts_combat(t):
    d = datetime.fromtimestamp(t)
    return f"{d.month}/{d.day}/{d.year} {d.hour:02d}:{d.minute:02d}:{d.second:02d}.{int((t % 1) * 10000):04d}"

def ts_chat(t):
    d = datetime.fromtimestamp(t)
    return f"{d.month}/{d.day} {d.hour:02d}:{d.minute:02d}:{d.second:02d}.{int((t % 1) * 1000):03d}"

def link(item_id, name, quality, level=20):
    return f"|cnIQ{quality}:|Hitem:{item_id}::::::::{level}:1486::11:::::::|h[{name}]|h|r"

def world(pos):
    """Map coordinates to rough world coordinates, for the combat log."""
    m, x, y = pos
    return (round(2600 - y * 4000, 2), round(1400 - x * 4200, 2))

class Sim:
    def __init__(self):
        self.n = 0
        self.rows = []
        self.combat = {}  # session index -> lines
        self.chat = {}
        self.session = -1
        self.t = 0.0
        self.level = 1
        self.xp = 0
        self.money = 0
        self.pos = (1420, 0.322, 0.662)
        self.facing = 0.0
        self.zone = None
        self.sub = None
        self.bags = {}
        self.seen = {}
        self.items = {}
        self.spells = set()
        self.questlog = {}
        self.done = set()
        self.quest_texts = {}
        self.gossip = {}
        self.deaths = 0
        self.kills = 0
        self.hp = 60
        self.players = {}
        self.group = []
        self.session_end = 0
        self.worn = {}  # slot -> item id
        self.lay_on_hands = -1e9

    def log(self, e, **row):
        self.n += 1
        row.update(e=e, n=self.n, t=int(self.t))
        self.rows.append(row)

    def cl(self, text):
        self.combat.setdefault(self.session, []).append(f"{ts_combat(self.t)}  {text}")

    def chat_line(self, text):
        self.chat.setdefault(self.session, []).append(f"{ts_chat(self.t)}  {text}")

    def wait(self, s):
        self.t += s

    # --- sessions
    def login(self, start):
        self.session += 1
        self.t = start
        self.log("login", level=self.level, money=self.money, xp=self.xp, zone=ZONES[self.pos[0]])
        self.log("zone", map=self.pos[0], sub=self.sub or "", zone=ZONES[self.pos[0]])
        self.wait(20)
        self.log("played", level=int(self.level_played), total=int(self.played))

    def logout(self):
        self.log("logout", money=self.money, zone=ZONES[self.pos[0]])

    @property
    def played(self):
        return sum(e - s for s, e in SESSION_TIMES[: self.session]) + (self.t - SESSION_TIMES[self.session][0])

    @property
    def level_played(self):
        return self.played * 0.18

    # --- movement
    def route(self, m, tx, ty):
        """The way along the roads from here to (tx, ty)."""
        nodes, edges = ROADS.get(m, ({}, []))
        if not nodes:
            return [(tx, ty)]
        near = lambda x, y: min(nodes, key=lambda k: math.hypot(nodes[k][0] - x, nodes[k][1] - y))
        a, b = near(self.pos[1], self.pos[2]), near(tx, ty)
        if a == b or math.hypot(tx - self.pos[1], ty - self.pos[2]) < 0.06:
            return [(tx, ty)]
        prev, todo = {a: None}, [a]
        while todo:
            k = todo.pop(0)
            for u, v in edges:
                for x, y in ((u, v), (v, u)):
                    if x == k and y not in prev:
                        prev[y] = k
                        todo.append(y)
        path, k = [], b
        while k is not None:
            path.append(nodes[k])
            k = prev.get(k)
        return list(reversed(path)) + [(tx, ty)]

    def go(self, area):
        m, (cx, cy), spread, _ = AREAS[area]
        if m is None:
            return
        if m != self.pos[0]:
            # Through the Undercity to the Sepulcher road: a jump in the path.
            self.wait(90)
            self.pos = (m, 0.66, 0.08) if m == 1421 else (m, 0.63, 0.66)
            self.set_zone(area)
        tx, ty = cx + rng.uniform(-spread, spread), cy + rng.uniform(-spread, spread)
        for wx, wy in self.route(m, tx, ty):
            _, x, y = self.pos
            dist = math.hypot(wx - x, wy - y)
            steps = max(1, int(dist / 0.0062))
            for i in range(1, steps + 1):
                f = i / steps
                nx = x + (wx - x) * f + rng.gauss(0, 0.0008)
                ny = y + (wy - y) * f + rng.gauss(0, 0.0008)
                self.facing = round(math.atan2(-(nx - self.pos[1]), -(ny - self.pos[2])) % (2 * math.pi), 2)
                self.pos = (m, nx, ny)
                self.wait(4 + rng.random())
                self.log("pos", map=m, x=round(nx, 4), y=round(ny, 4), f=self.facing)
                self.check_session()
        self.set_zone(area)

    def wander(self, area, seconds):
        m, (cx, cy), spread, _ = AREAS[area]
        if m is None:
            self.wait(seconds)
            return
        end = self.t + seconds
        while self.t < end:
            _, x, y = self.pos
            nx = min(max(x + rng.gauss(0, 0.004) + (cx - x) * 0.25, cx - spread * 1.4), cx + spread * 1.4)
            ny = min(max(y + rng.gauss(0, 0.004) + (cy - y) * 0.25, cy - spread * 1.4), cy + spread * 1.4)
            self.facing = round(math.atan2(-(nx - x), -(ny - y)) % (2 * math.pi), 2)
            self.pos = (m, nx, ny)
            self.wait(4 + rng.random())
            self.log("pos", map=m, x=round(nx, 4), y=round(ny, 4), f=self.facing)

    def set_zone(self, area):
        m = AREAS[area][0]
        zone = ZONES.get(m, "Tirisfal Glades") if m else "Ruins of Lordaeron"
        sub = SUBZONES[area]
        if zone != self.zone:
            self.zone = zone
            self.log("zone", map=m or 2870, sub=sub, zone=zone)
        if sub != self.sub:
            self.sub = sub
            self.log("subzone", sub=sub, zone=zone)
            if sub not in self.explored:
                self.explored.add(sub)
                self.log("explore", map=m or 2870, sub=sub, zone=zone)

    def check_session(self):
        if rng.random() < 0.03:
            self.say_chat()
        if self.t > self.session_end and self.session + 1 < len(SESSION_TIMES):
            self.logout()
            self.login(SESSION_TIMES[self.session + 1][0])
            self.session_end = SESSION_TIMES[self.session][1]

    # --- progress
    def gain_xp(self, d):
        if self.level >= 20:
            return
        self.xp += d
        while self.level < 20 and self.xp >= XP_TO[self.level - 1]:
            self.xp -= XP_TO[self.level - 1]
            self.level += 1
            self.log("level", level=self.level, zone=self.zone)
            if self.level >= 20:
                self.xp = 0
        if self.level < 20:
            self.log("xp", cur=self.xp, d=d, max=XP_TO[self.level - 1])

    def gain_money(self, d, ctx):
        self.money += d
        self.log("money", ctx=ctx, d=d, total=self.money)

    def gain_item(self, item, n=1, ctx="loot"):
        iid, name, icon, q = item
        key = str(iid)
        self.items[iid] = {"name": name, "icon": icon, "q": q}
        self.bags[key] = self.bags.get(key, 0) + n
        self.seen.setdefault(key, int(self.t))
        self.log("item", ctx=ctx, d=n, key=key, link=link(iid, name, q, self.level), total=self.bags[key])
        if ctx == "loot":
            self.log("msg", kind="loot", text=f"You receive loot: {link(iid, name, q, self.level)}" + (f"x{n}." if n > 1 else "."))

    def equip(self, item, slot):
        iid, name, icon, q = item[:4]
        if self.worn.get(slot) == iid:
            return
        self.worn[slot] = iid
        self.log("equip", link=link(iid, name, q, self.level), slot=slot)

    def trainer(self, area_hub):
        new = [s for s in SPELLS if s[0] <= self.level and s[1] not in self.spells]
        if not new:
            return
        self.log("open", what="trainer")
        self.log("gossip", name="Hilda the Breaker" if area_hub == "bandarion" else "Shari Stilwell", npc=248850 if area_hub == "bandarion" else 5680)
        for lvl, sid, name in new:
            self.spells.add(sid)
            self.gain_money(-(lvl * lvl * 8 + 10), "trainer")
            self.log("spell", id=sid)
            self.wait(2)

    def vendor(self):
        self.log("open", what="merchant")
        total = 0
        for key, n in list(self.bags.items()):
            iid = int(key)
            info = self.items.get(iid)
            if info and info["q"] == 0:
                self.log("item", ctx="merchant", d=-n, key=key, link=link(iid, info["name"], 0, self.level), total=0)
                total += n * (4 + self.level * 2)
                del self.bags[key]
        if total:
            self.gain_money(total, "merchant")
        self.gain_money(-(25 * self.level), "merchant")
        self.gain_item(FOOD, 5, "merchant")

    # --- combat
    def my_abilities(self, undead):
        out = ["Seal of Fury" if 91602 in self.spells else "Seal of Righteousness"]
        if 20271 in self.spells:
            out.append("Judgement")
        if 91601 in self.spells:
            out.append("Holy Strike")
        if 26573 in self.spells:
            out.append("Consecration")
        if undead and 879 in self.spells:
            out.append("Exorcism")
        return out

    def adv(self, guid, hp, maxhp, pos=None):
        wx, wy = world(pos or self.pos)
        m = (pos or self.pos)[0] or 2870
        return f"{guid},0000000000000000,{hp},{maxhp},{40 + self.level * 8},{self.level * 2},{self.level * 40},0,0,{self.level * 30},{self.level * 30},0,{wx},{wy},{m},{self.facing:.4f},0,{self.level}"

    def fight(self, name, level, npc, undead, boss=False, party=False, lose=False):
        guid = f"Creature-0-4615-0-{rng.randint(1000, 9999)}-{npc}-{rng.randint(0, 0xFFFFFFFF):010X}"
        mob = f'{guid},"{name}",0xa48,0x0'
        me = f'{ME},"{ME_NAME}-",0x511,0x0'
        hp = int((30 + level * 26) * (3.0 if boss else 1))
        maxhp = self.level * 38 + 60
        self.hp = min(self.hp, maxhp)
        swing_base = 8 + self.level * 2.6
        abilities = self.my_abilities(undead)
        next_swing, next_mob, next_judge, next_strike, next_consec = 0.0, 0.8, 1.2, 2.0, 0.5
        start = self.t
        first = True
        members = self.group if party else []
        if not party and rng.random() < 0.08:
            g, gname, _, cls, *_ = rng.choice(STRANGERS)
            spell = CAST[cls][-1]
            src = f'{g},"{gname}-{REALM}",0x548,0x0'
            self.cl(f'SPELL_CAST_SUCCESS,{src},{me},0,"{spell}",0x2,{self.adv(g, 400, 400, self.pos)}')
            self.cl(f'SPELL_CAST_SUCCESS,{src},{mob},0,"{CAST[cls][0]}",0x20,{self.adv(g, 400, 400, self.pos)}')
        while hp > 0:
            # One step of the fight: the nearest thing to happen.
            step = min(next_swing, next_mob, next_judge if "Judgement" in abilities else 99, next_strike if "Holy Strike" in abilities else 99)
            step = max(step, 0.2)
            self.wait(step)
            next_swing -= step; next_mob -= step; next_judge -= step; next_strike -= step; next_consec -= step
            if first and "Consecration" in abilities:
                self.cl(f'SPELL_CAST_SUCCESS,{me},0000000000000000,nil,0x80000000,0x80000000,26573,"Consecration",0x2,{self.adv(ME, self.hp, maxhp)}')
                first = False
            if next_swing <= 0:
                crit = rng.random() < 0.06
                d = int(swing_base * rng.uniform(0.8, 1.2) * (2 if crit else 1))
                if rng.random() < 0.07:
                    self.cl(f"SWING_MISSED,{me},{mob},MISS,nil")
                else:
                    hp -= d
                    self.cl(f"SWING_DAMAGE,{me},{mob},{self.adv(ME, self.hp, maxhp)},{d},{d},-1,1,0,0,0,{1 if crit else 'nil'},nil,nil")
                    seal = abilities[0]
                    sid = 91602 if seal == "Seal of Fury" else 25742
                    sd = int(self.level * 1.2 + rng.randint(2, 5))
                    hp -= sd
                    self.cl(f'SPELL_DAMAGE,{me},{mob},{sid},"{seal}",0x2,{self.adv(guid, max(hp, 0), 999)},{sd},{sd},-1,2,0,0,0,nil,nil,nil')
                    if rng.random() < 0.05:
                        tg = int(self.level * 0.6 + 3)
                        hp -= tg
                        self.cl(f'SPELL_DAMAGE,{me},{mob},5227,"Touch of the Grave",0x20,{self.adv(guid, max(hp, 0), 999)},{tg},{tg},-1,32,0,0,0,nil,nil,nil')
                next_swing = 2.4 if self.level >= 10 else 3.0
            if "Judgement" in abilities and next_judge <= 0:
                d = int(self.level * 2.4 + rng.randint(2, 9))
                crit = rng.random() < 0.08
                d = d * 2 if crit else d
                hp -= d
                self.cl(f'SPELL_CAST_SUCCESS,{me},{mob},20271,"Judgement",0x2,{self.adv(ME, self.hp, maxhp)}')
                self.cl(f'SPELL_DAMAGE,{me},{mob},20271,"Judgement",0x2,{self.adv(guid, max(hp, 0), 999)},{d},{d},-1,2,0,0,0,{1 if crit else "nil"},nil,nil')
                next_judge = 8.0
            if "Holy Strike" in abilities and next_strike <= 0:
                d = int(self.level * 1.3 + swing_base * 0.42 + rng.randint(1, 6))
                hp -= d
                self.cl(f'SPELL_DAMAGE,{me},{mob},91601,"Holy Strike",0x2,{self.adv(guid, max(hp, 0), 999)},{d},{d},-1,2,0,0,0,nil,nil,nil')
                next_strike = 6.0
            if "Consecration" in abilities and next_consec <= 0:
                d = rng.randint(6, 9)
                hp -= d
                self.cl(f'SPELL_PERIODIC_DAMAGE,{me},{mob},26573,"Consecration",0x2,{self.adv(guid, max(hp, 0), 999)},{d},{d},-1,2,0,0,0,nil,nil,nil')
                next_consec = 1.0
            if "Exorcism" in abilities and rng.random() < 0.05:
                d = rng.randint(70, 85)
                hp -= d
                self.cl(f'SPELL_DAMAGE,{me},{mob},879,"Exorcism",0x2,{self.adv(guid, max(hp, 0), 999)},{d},{d},-1,2,0,0,0,nil,nil,nil')
            for g, gname, _, cls, *_ in members:
                if rng.random() < 0.35:
                    spell = rng.choice(CAST[cls])
                    src = f'{g},"{gname}-{REALM}",0x512,0x0'
                    self.cl(f'SPELL_CAST_SUCCESS,{src},{mob},0,"{spell}",0x20,{self.adv(g, 600, 600, self.pos)}')
                    if spell in ("Lesser Heal", "Renew", "Rejuvenation", "Healing Touch") and self.hp < maxhp:
                        h = rng.randint(60, 140)
                        over = max(0, self.hp + h - maxhp)
                        self.hp = min(maxhp, self.hp + h)
                        self.cl(f'SPELL_HEAL,{src},{me},0,"{spell}",0x2,{self.adv(ME, self.hp, maxhp)},{h},{h},{over},0,nil')
                    else:
                        d = rng.randint(30, 70)
                        hp -= d
                        self.cl(f'SPELL_DAMAGE,{src},{mob},0,"{spell}",0x20,{self.adv(guid, max(hp, 0), 999)},{d},{d},-1,32,0,0,0,nil,nil,nil')
            if next_mob <= 0:
                if rng.random() < 0.22:
                    self.cl(f"SWING_MISSED,{mob},{me},{rng.choice(['DODGE', 'PARRY', 'BLOCK'])},nil")
                else:
                    d = int((2 + level * 1.1) * rng.uniform(0.7, 1.3) * (1.6 if boss else 1))
                    self.hp -= d
                    self.cl(f"SWING_DAMAGE,{mob},{me},{self.adv(guid, max(hp, 0), 999)},{d},{d},-1,1,0,0,0,nil,nil,nil")
                    if 7294 in self.spells:
                        r = rng.randint(4, 7)
                        hp -= r
                        self.cl(f'SPELL_DAMAGE,{me},{mob},7294,"Retribution Aura",0x2,{self.adv(guid, max(hp, 0), 999)},{r},{r},-1,2,0,0,0,nil,nil,nil')
                next_mob = 2.0
            if lose and self.t - start > 6:
                self.hp = min(self.hp, int(maxhp * 0.25))
            if not lose and self.hp < maxhp * 0.3 and 633 in self.spells and self.t - self.lay_on_hands > 3600 and rng2.random() < 0.25:
                self.lay_on_hands = self.t
                h = maxhp - self.hp
                self.hp = maxhp
                self.cl(f'SPELL_CAST_SUCCESS,{me},{me},633,"Lay on Hands",0x2,{self.adv(ME, self.hp, maxhp)}')
                self.cl(f'SPELL_HEAL,{me},{me},633,"Lay on Hands",0x2,{self.adv(ME, self.hp, maxhp)},{h},{h},0,0,nil')
            if self.hp < maxhp * 0.3:
                # Some fights are lost (the road has two); otherwise the Light answers.
                if lose:
                    self.hp = 0
                    self.die()
                    return False
                h = int(self.level * 12 + rng.randint(15, 30))
                over = max(0, self.hp + h - maxhp)
                self.wait(2.5)
                self.hp = min(maxhp, self.hp + h)
                self.cl(f'SPELL_HEAL,{me},{me},635,"Holy Light",0x2,{self.adv(ME, self.hp, maxhp)},{h},{h},{over},0,nil')
            if self.t - start > 90:
                break
        self.wait(0.3)
        self.cl(f"UNIT_DIED,0000000000000000,nil,0x80000000,0x80000000,{mob},0")
        self.kills += 1
        diff = level - self.level
        xp = max(10, int((self.level * 5 + 45) * (1 + 0.05 * diff))) * (3 if boss else 1)
        if party:
            xp = int(xp * 0.55)
        self.gain_xp(xp)
        self.log("open", what="loot")
        if rng.random() < 0.7:
            self.gain_money(rng.randint(level * 3, level * 9), "loot")
        if rng.random() < 0.55:
            self.gain_item(rng.choice(LOOT[undead]))
        # Patch up between pulls.
        if self.hp < maxhp * 0.45:
            h = int(self.level * 11 + rng.randint(10, 30))
            over = max(0, self.hp + h - maxhp)
            self.wait(2.5)
            self.cl(f'SPELL_HEAL,{me},{me},635,"Holy Light",0x2,{self.adv(ME, min(maxhp, self.hp + h), maxhp)},{h},{h},{over},0,nil')
            self.hp = min(maxhp, self.hp + h)
        self.hp = min(maxhp, self.hp + int(maxhp * 0.25))
        return True

    def die(self):
        self.deaths += 1
        self.cl(f'UNIT_DIED,0000000000000000,nil,0x80000000,0x80000000,{ME},"{ME_NAME}-",0x511,0x0,0')
        self.log("death", sub=self.sub, zone=self.zone)
        self.wait(40)
        self.log("alive")
        self.wait(70)
        self.log("unghost")
        self.hp = int((self.level * 38 + 60) * 0.5)

    def say_chat(self):
        if rng.random() < 0.5:
            who = rng.choice(STRANGERS + PARTY)
            self.chat_line(f"[1. General] {who[1]}-{REALM}: {rng.choice(GENERAL)}")
        if rng.random() < 0.08:
            who = rng.choice(STRANGERS)
            self.chat_line(f"[4. LookingForGroup] {who[1]}-{REALM}: LFM Ruins of Lordaeron, need tank")

    # --- a quest from start to finish
    def quest(self, q):
        qid, title, level, hub, target, objectives, giver, text, objective, reward = q
        self.go(hub)
        self.log("gossip", name=giver, npc=40000 + qid % 9000)
        self.quest_texts[qid] = {"title": title, "text": text, "objective": objective, "level": level, "seen": int(self.t)}
        self.log("quest", act="accept", id=qid, title=title)
        self.questlog[qid] = {"id": qid, "title": title, "level": level, "objectives": [
            {"text": f"{o}: 0/{n}", "have": 0, "need": n, "done": False} for o, n in objectives]}
        if target:
            party = target == "ruins"
            if party:
                self.run_dungeon()
            self.go(target) if AREAS[target][0] else None
            _, _, _, mobs = AREAS[target]
            for i, (what, need) in enumerate(objectives):
                have = 0
                while have < need:
                    if what in QUEST_ITEMS:
                        icon, _ = QUEST_ITEMS[what]
                        self.wander(target, 25)
                        have += 1
                        self.gain_item((90000 + qid, what, icon, 1), 1, "loot")
                    else:
                        boss = what in BOSSES
                        if boss:
                            lvl, npc, undead = BOSSES[what]
                        else:
                            m = [x for x in mobs if x[0] == what][0]
                            _, lvl, npc, undead = m
                            lvl += rng.choice([0, 0, 1])
                        self.wander(target, rng.uniform(8, 30)) if AREAS[target][0] else self.wait(rng.uniform(15, 35))
                        lose = (what == "Ivar the Foul" and self.deaths == 0) or (what == "Moonrage Darksoul" and have == 2 and self.deaths == 1)
                        if not self.fight(what, lvl, npc, undead, boss=boss, party=party, lose=lose):
                            self.go(hub) if AREAS[hub][0] else None
                            self.go(target) if AREAS[target][0] else None
                            continue
                        have += 1
                        # Mobs that aren't on the list come along too.
                        if not party and rng.random() < 0.35 and mobs:
                            other = rng.choice(mobs)
                            self.fight(other[0], other[1], other[2], other[3])
                    ob = self.questlog[qid]["objectives"][i]
                    ob.update(have=have, text=f"{what}: {have}/{need}", done=have >= need)
                    self.log("objective", have=have, i=i + 1, id=qid, need=need, text=ob["text"])
                    self.say_chat() if rng.random() < 0.15 else None
                    self.check_session()
            if party:
                self.leave_group()
        self.go(hub)
        self.log("gossip", name=giver, npc=40000 + qid % 9000)
        xp = int(80 * level * (1.3 if level > 10 else 1) + 60)
        money = level * 45 + rng.randint(0, 60)
        choice = None
        if reward:
            choice = link(reward[0], reward[1], reward[3], self.level)
            self.quest_texts[qid]["choices"] = [choice]
        self.quest_texts[qid]["reward"] = "You have done well. The Dark Lady will hear of it."
        self.quest_texts[qid]["progress"] = "Is it done?"
        self.log("quest", act="turnin", choice=choice, id=qid, money=money, title=title, xp=xp)
        self.log("quest", act="remove", id=qid)
        self.questlog.pop(qid, None)
        self.done.add(qid)
        self.gain_xp(xp)
        self.gain_money(money, "quest")
        if reward:
            self.gain_item(reward, 1, "quest")
            if reward[0] in WEAR:
                self.wait(4)
                self.equip(reward, WEAR[reward[0]])
        self.check_session()

    def run_dungeon(self):
        self.group = PARTY
        for g in PARTY:
            self.players[g[0]] = g
        self.log("group", members=[f"{g[1]}-{REALM}" for g in PARTY] + ["Tom"])
        self.chat_line(f"[Party] Rakka-{REALM}: tom you tank?")
        self.chat_line(f"To Rakka-{REALM}: shield's up. consecration's up. let's go")
        self.chat_line(f"[Party] Mira-{REALM}: I'll keep you standing, paladin")
        self.set_zone("ruins")

    def leave_group(self):
        self.chat_line(f"[Party] Juno-{REALM}: gg, that baron hits hard")
        self.chat_line(f"[Party] Mira-{REALM}: well tanked!")
        self.group = []
        self.log("group", members=[])
        self.pos = (1420, 0.63, 0.66)
        self.zone = None
        self.set_zone("brill")


# --------------------------------------------------------------- sessions

def local(y, mo, d, h, mi):
    return datetime(y, mo, d, h, mi).timestamp()

SESSION_TIMES = [
    (local(2026, 9, 18, 19, 12), local(2026, 9, 18, 21, 40)),
    (local(2026, 9, 19, 19, 48), local(2026, 9, 19, 22, 5)),
    (local(2026, 9, 20, 18, 30), local(2026, 9, 20, 21, 20)),
    (local(2026, 9, 21, 20, 5), local(2026, 9, 21, 22, 15)),
    (local(2026, 9, 22, 19, 0), local(2026, 9, 22, 21, 45)),
    (local(2026, 9, 23, 19, 30), local(2026, 9, 23, 22, 30)),
    (local(2026, 9, 24, 18, 45), local(2026, 9, 24, 21, 50)),
    (local(2026, 9, 25, 17, 30), local(2026, 9, 25, 20, 30)),
]


def next_session(s):
    s.logout()
    s.login(SESSION_TIMES[s.session + 1][0])
    s.session_end = SESSION_TIMES[s.session][1]


def endgame(s):
    """At 20: the Ruins of Lordaeron again for gear, and Bandarion Keep."""
    while s.session < len(SESSION_TIMES) - 2:
        next_session(s)
        s.go("brill")
        s.vendor()
        for run in range(3):
            s.run_dungeon()
            for _ in range(26):
                name, lvl, npc, undead = rng.choice(AREAS["ruins"][3])
                s.wait(rng.uniform(10, 30))
                s.fight(name, lvl + 1, npc, undead, party=True)
            lvl, npc, undead = BOSSES["Baron Vardus"]
            s.fight("Baron Vardus", lvl, npc, undead, boss=True, party=True)
            if run == 0:
                s.gain_item((91413, "Scepter of the Abandoned", 133485, 3))
                s.wait(6)
                s.equip((91413, "Scepter of the Abandoned", 133485, 3), 16)
            s.leave_group()
        s.go("bandarion")
        s.log("gossip", name="Danitha Morr", npc=248851)
        s.wander("bandarion", 300)
    next_session(s)


def simulate():
    s = Sim()
    s.explored = set()
    s.session_end = SESSION_TIMES[0][1]
    s.pos = (1420, 0.322, 0.662)
    s.login(SESSION_TIMES[0][0])
    s.set_zone("deathknell")
    starter = [(25, "Worn Mace", 133480, 1, 16), (26, "Initiate's Chain Vest", 132627, 1, 5)]
    for iid, name, icon, q, slot in starter:
        s.items[iid] = {"name": name, "icon": icon, "q": q}
        s.seen[str(iid)] = int(s.t)
        s.worn[slot] = iid
    for q in QUESTS[:-1]:
        hub = q[3]
        if hub in ("brill", "bandarion", "sepulcher") and rng.random() < 0.6:
            s.go(hub)
            s.trainer(hub)
            s.vendor()
        s.quest(q)
        # Grind a little where the level lags the quests.
        while s.level < min(20, q[2] + 1) and q[4] and AREAS[q[4]][0]:
            area = q[4]
            mob = rng.choice(AREAS[area][3])
            s.wander(area, rng.uniform(10, 30))
            s.fight(mob[0], mob[1] + 1, mob[2], mob[3])
            s.check_session()
    while s.level < 20:
        mob = rng.choice(AREAS["shadowfang"][3])
        s.go("shadowfang")
        s.wander("shadowfang", 15)
        s.fight(mob[0], mob[1], mob[2], mob[3])
        s.check_session()
    endgame(s)
    s.go("sepulcher")
    s.trainer("sepulcher")
    # The last evening: at 20, in the gear from the road.
    for slot, item in GEAR.items():
        iid, name, icon, q = item[:4]
        s.items[iid] = {"name": name, "icon": icon, "q": q}
        s.seen.setdefault(str(iid), int(s.t) - rng.randint(0, 3 * 86400))
        s.equip((iid, name, icon, q), int(slot))
    arugal = QUESTS[-1]
    s.quest_texts[1014] = {"title": arugal[1], "text": arugal[7], "objective": arugal[8], "level": 20, "seen": int(s.t),
                           "choices": [link(6414, "Seal of Sylvanas", 2, 20)]}
    s.questlog[1014] = {"id": 1014, "title": "Arugal Must Die", "level": 20, "objectives": [
        {"text": "Head of Arugal: 0/1", "have": 0, "need": 1, "done": False}]}
    s.log("quest", act="accept", id=1014, title="Arugal Must Die")
    # Two quests still open.
    s.questlog[91217] = {"id": 91217, "title": "The Wolfsbane Oath", "level": 20, "objectives": [
        {"text": "Speak with Danitha Morr at Bandarion Keep", "have": 0, "need": 1, "done": False}]}
    s.quest_texts[91217] = {"title": "The Wolfsbane Oath", "level": 20, "seen": int(s.t),
        "text": "Every paladin of Bandarion Keep carries a weapon earned, not given. Wolfsbane waits for one who has proven the Light still answers the Forsaken. Go back to the keep; Danitha Morr will tell you the rest.",
        "objective": "Speak with Danitha Morr at Bandarion Keep."}
    s.log("quest", act="accept", id=91217, title="The Wolfsbane Oath")
    s.wander("sepulcher", 900)
    s.go("bandarion")
    s.log("gossip", name="Danitha Morr", npc=248851)
    s.wander("bandarion", 900)
    s.logout()
    return s

# ------------------------------------------------------------------- gear

def tip(name, q, lines):
    color = {0: "9d9d9d", 1: "ffffff", 2: "1eff00", 3: "0070dd"}[q]
    return [{"l": name, "lc": color}, {"l": "Soulbound", "lc": "ffffff"}] + lines

GEAR = {
    "1": (91407, "Crusader's Silvered Chain Helm", 133071, 2, [{"l": "Head", "lc": "ffffff", "r": "Mail", "rc": "ffffff"}, {"l": "168 Armor", "lc": "ffffff"}, {"l": "+6 Strength", "lc": "ffffff"}, {"l": "+7 Stamina", "lc": "ffffff"}, {"l": "Durability 60 / 60", "lc": "ffffff"}, {"l": "Requires Level 20", "lc": "ffffff"}, {"l": "Blacksmithing", "lc": "ffd200"}]),
    "2": (91404, "Tarnished Locket", 133294, 2, [{"l": "Neck", "lc": "ffffff"}, {"l": "+5 Stamina", "lc": "ffffff"}, {"l": "+3 Spirit", "lc": "ffffff"}, {"l": "Requires Level 16", "lc": "ffffff"}, {"l": "\"Remember that I love you.\"", "lc": "ffd200"}]),
    "3": (91402, "Deathguard Pauldrons", 135040, 2, [{"l": "Shoulder", "lc": "ffffff", "r": "Mail", "rc": "ffffff"}, {"l": "131 Armor", "lc": "ffffff"}, {"l": "+4 Strength", "lc": "ffffff"}, {"l": "+5 Stamina", "lc": "ffffff"}, {"l": "Durability 55 / 55", "lc": "ffffff"}, {"l": "Requires Level 14", "lc": "ffffff"}]),
    "4": (91408, "Tirisfal Linen Shirt", 135009, 1, [{"l": "Shirt", "lc": "ffffff"}]),
    "5": (91409, "Hauberk of the Forsaken Vigil", 132627, 2, [{"l": "Chest", "lc": "ffffff", "r": "Mail", "rc": "ffffff"}, {"l": "204 Armor", "lc": "ffffff"}, {"l": "+6 Strength", "lc": "ffffff"}, {"l": "+8 Stamina", "lc": "ffffff"}, {"l": "Durability 85 / 85", "lc": "ffffff"}, {"l": "Requires Level 18", "lc": "ffffff"}]),
    "6": (91410, "Crusader's Chain Belt", 132492, 2, [{"l": "Waist", "lc": "ffffff", "r": "Mail", "rc": "ffffff"}, {"l": "92 Armor", "lc": "ffffff"}, {"l": "+4 Strength", "lc": "ffffff"}, {"l": "+4 Stamina", "lc": "ffffff"}, {"l": "Durability 35 / 35", "lc": "ffffff"}, {"l": "Requires Level 19", "lc": "ffffff"}, {"l": "Blacksmithing", "lc": "ffd200"}]),
    "7": (91405, "Pathfinder's Clearway", 134583, 2, [{"l": "Legs", "lc": "ffffff", "r": "Mail", "rc": "ffffff"}, {"l": "162 Armor", "lc": "ffffff"}, {"l": "+5 Strength", "lc": "ffffff"}, {"l": "+6 Stamina", "lc": "ffffff"}, {"l": "Durability 75 / 75", "lc": "ffffff"}, {"l": "Requires Level 17", "lc": "ffffff"}]),
    "8": (91411, "Crusader's Boots", 132539, 2, [{"l": "Feet", "lc": "ffffff", "r": "Mail", "rc": "ffffff"}, {"l": "118 Armor", "lc": "ffffff"}, {"l": "+3 Strength", "lc": "ffffff"}, {"l": "+5 Stamina", "lc": "ffffff"}, {"l": "Durability 50 / 50", "lc": "ffffff"}, {"l": "Requires Level 19", "lc": "ffffff"}, {"l": "Blacksmithing", "lc": "ffd200"}]),
    "9": (91403, "Sepulcher Bracers", 132606, 2, [{"l": "Wrist", "lc": "ffffff", "r": "Mail", "rc": "ffffff"}, {"l": "71 Armor", "lc": "ffffff"}, {"l": "+3 Stamina", "lc": "ffffff"}, {"l": "+2 Strength", "lc": "ffffff"}, {"l": "Durability 35 / 35", "lc": "ffffff"}, {"l": "Requires Level 15", "lc": "ffffff"}]),
    "10": (91412, "Crusader's Gloves", 132938, 2, [{"l": "Hands", "lc": "ffffff", "r": "Mail", "rc": "ffffff"}, {"l": "105 Armor", "lc": "ffffff"}, {"l": "+4 Strength", "lc": "ffffff"}, {"l": "+4 Stamina", "lc": "ffffff"}, {"l": "Durability 35 / 35", "lc": "ffffff"}, {"l": "Requires Level 18", "lc": "ffffff"}, {"l": "Blacksmithing", "lc": "ffd200"}]),
    "11": (6414, "Seal of Sylvanas", 133357, 2, [{"l": "Finger", "lc": "ffffff"}, {"l": "+5 Stamina", "lc": "ffffff"}, {"l": "+4 Strength", "lc": "ffffff"}, {"l": "Requires Level 20", "lc": "ffffff"}]),
    "12": (91406, "Baron's Signet", 133349, 3, [{"l": "Finger", "lc": "ffffff"}, {"l": "+6 Stamina", "lc": "ffffff"}, {"l": "Equip: Increases your chance to block attacks with a shield by 1%.", "lc": "1eff00"}, {"l": "Requires Level 19", "lc": "ffffff"}]),
    "15": (6629, "Sporid Cape", 133768, 3, [{"l": "Back", "lc": "ffffff"}, {"l": "24 Armor", "lc": "ffffff"}, {"l": "+5 Stamina", "lc": "ffffff"}, {"l": "+3 Agility", "lc": "ffffff"}, {"l": "Requires Level 17", "lc": "ffffff"}]),
    "16": (91413, "Scepter of the Abandoned", 133485, 3, [{"l": "One-Hand", "lc": "ffffff", "r": "Mace", "rc": "ffffff"}, {"l": "21 - 40 Damage", "lc": "ffffff", "r": "Speed 2.40", "rc": "ffffff"}, {"l": "(12.7 damage per second)", "lc": "ffffff"}, {"l": "+4 Stamina", "lc": "ffffff"}, {"l": "Equip: Increases damage and healing done by magical spells and effects by up to 16.", "lc": "1eff00"}, {"l": "Durability 75 / 75", "lc": "ffffff"}, {"l": "Requires Level 18", "lc": "ffffff"}]),
    "17": (6630, "Seedcloud Buckler", 134955, 3, [{"l": "Off Hand", "lc": "ffffff", "r": "Shield", "rc": "ffffff"}, {"l": "412 Armor", "lc": "ffffff"}, {"l": "8 Block", "lc": "ffffff"}, {"l": "+5 Stamina", "lc": "ffffff"}, {"l": "+3 Spirit", "lc": "ffffff"}, {"l": "Durability 85 / 85", "lc": "ffffff"}, {"l": "Requires Level 17", "lc": "ffffff"}]),
}

def equipment(seen):
    out = {}
    for slot, (iid, name, icon, q, lines) in GEAR.items():
        dur = [int(x) for l in lines if l["l"].startswith("Durability") for x in l["l"].split()[1::2]]
        out[slot] = {"id": iid, "icon": icon, "quality": q, "link": link(iid, name, q, 20),
                     "tooltip": tip(name, q, lines), "firstSeen": seen.get(str(iid), 0)}
        if dur:
            out[slot]["durability"] = dur[:2]
    return out

STATS = [
    {"name": "General", "stats": [
        {"key": "HEALTH", "label": "Health", "number": 842, "value": "842"},
        {"key": "POWER", "label": "Mana", "number": 616, "value": "616"},
        {"key": "MOVESPEED", "label": "Movement Speed", "number": 100, "value": "100%"}]},
    {"name": "Primary Attributes", "stats": [
        {"key": "STRENGTH", "label": "Strength", "number": 61, "value": "61"},
        {"key": "AGILITY", "label": "Agility", "number": 38, "value": "38"},
        {"key": "STAMINA", "label": "Stamina", "number": 72, "value": "72"},
        {"key": "INTELLECT", "label": "Intellect", "number": 44, "value": "44"},
        {"key": "SPIRIT", "label": "Spirit", "number": 44, "value": "44"}]},
    {"name": "Weapons", "stats": [
        {"key": "MAINHAND_DAMAGE", "label": "Main Hand", "number": 40, "value": "28 - 40"},
        {"key": "ATTACK_AP", "label": "Attack Power", "number": 162, "value": "162"},
        {"key": "SPELLPOWER", "label": "Spell Power", "number": 16, "value": "16"}]},
    {"name": "Modifiers", "stats": [
        {"key": "CRITCHANCE", "label": "Critical Strike", "number": "5.6%", "value": "5.6%"},
        {"key": "HITCHANCE", "label": "Hit", "number": "3.0%", "value": "3.0%"},
        {"key": "HASTE", "label": "Haste", "number": "0.0%", "value": "0.0%"}]},
    {"name": "Defense", "stats": [
        {"key": "DEFENSE", "label": "Defense", "number": 100, "value": "100 / 100"},
        {"key": "DODGE", "label": "Dodge", "number": 5.3, "value": "5.3%"},
        {"key": "PARRY", "label": "Parry", "number": 5.0, "value": "5.0%"},
        {"key": "BLOCK", "label": "Block", "number": 15.8, "value": "15.8%"},
        {"key": "ARMOR", "label": "Armor", "number": 1862, "value": "1862"}]},
]

# Protection, 16 points: 11 from levels 10 to 20, 5 more from Talented (Legacy).
PROT = {"Redoubt": 5, "Precision": 3, "Anticipation": 2, "Improved Seal of Fury": 1, "Shield Specialization": 3,
        "Improved Righteous Fury": 1, "One-Handed Weapon Specialization": 1}
# 8 Legacy points: two main professions at 150 on each of the four characters.
LEGACY = {"High Alert": 2, "Talented": 5, "Well Rested": 1}
PROFESSIONS = {"Tom": ["Blacksmithing", "Mining"], "Usain": ["Tailoring", "Enchanting"],
               "Elijah": ["Alchemy", "Herbalism"], "Oprah": ["Engineering", "Skinning"], "Jon": []}

def talents(template):
    t = json.loads(json.dumps(template["talents"]))
    spec = t["specs"][0]
    spent = 0
    for n in spec["nodes"]:
        name = n["entries"][0]["name"]
        r = PROT.get(name, 0) if 11541 in n["groups"] else 0
        n["ranks"] = r
        n["activeRank"] = r
        spent += r
    for g in spec["groups"]:
        g["spent"] = sum(n["ranks"] for n in spec["nodes"] if g["id"] in n["groups"])
    spec["currency"] = [{"maxQuantity": spent, "quantity": 0, "spent": spent, "spentInTree": spent, "traitCurrencyID": 3820}]
    return t

def legacy(template):
    L = json.loads(json.dumps(template["legacy"]))
    for tree in L["trees"]:
        spent = 0
        for n in tree["nodes"]:
            name = n["entries"][0]["name"]
            r = LEGACY.get(name, 0)
            n["ranks"] = r
            n["activeRank"] = min(r, 1) if n.get("type") == 1 else r
            if name == "Talented" and r:
                n["activeEntry"] = n["entries"][min(r, len(n["entries"])) - 1]["id"]
            spent += r
        # One account-wide currency: every tree reports the same total.
        tree["currency"] = [{"maxQuantity": 8, "quantity": 8 - sum(LEGACY.values()), "spent": sum(LEGACY.values()), "spentInTree": spent, "traitCurrencyID": 4225}]
    return L

def skills(template, profs):
    out = [s for s in template["skills"] if not s.get("isHeader") or s["name"] != "Professions"]
    out.append({"isHeader": True, "name": "Professions", "rank": 0, "maxRank": 0, "skillID": 11})
    for p in profs:
        out.append({"isHeader": False, "name": p, "rank": 150, "maxRank": 150, "skillID": 0})
    for s in out:
        if s.get("name") == "Defense":
            s.update(rank=100, maxRank=100)
        elif not s.get("isHeader") and s.get("maxRank", 0) > 1 and s["name"] not in profs:
            s.update(rank=min(100, s.get("maxRank", 100)), maxRank=100)
    return out

def snapshot(template, s):
    snap = {k: v for k, v in template.items()}
    snap.update(
        name="Tom", surname="Crusader", displayName="Tom Crusader", guid=ME, realm=REALM,
        level=20, money=s.money, zone="Silverpine Forest", xp={"cur": 0, "max": 0},
        updated=int(s.t) - 60, loggedOut=int(s.t), sex=2,
        equipment=equipment(s.seen), stats=STATS, talents=talents(template), legacy=legacy(template),
        skills=skills(template, PROFESSIONS["Tom"]),
    )
    return snap

# Alts: level 20, the new race and class combos, a quick evening each.
ALTS = [
    ("Usain", "Frostbolt", "Orc", "Orc", 2, "Mage", "MAGE", 8, "Player-4613-0C3A0071",
     [("16", 91501, "Emberwood Staff", 135146, 2), ("5", 91502, "Robe of the Pyre", 132646, 2)], "Durotar"),
    ("Elijah", "Felwood", "Troll", "Troll", 8, "Warlock", "WARLOCK", 9, "Player-4613-0C3A0072",
     [("16", 91503, "Voodoo Hexstaff", 135144, 2), ("5", 91504, "Darkspear Shroud", 132648, 2)], "The Barrens"),
    ("Oprah", "Windfury", "Dwarf", "Dwarf", 3, "Shaman", "SHAMAN", 7, "Player-4613-0C3A0073",
     [("16", 91505, "Stonecaller's Maul", 133046, 2), ("5", 91506, "Hauberk of the Deeprun", 132625, 2)], "Loch Modan"),
    ("Jon", "Hamstring", "Human", "Human", 1, "Warrior", "WARRIOR", 1, "Player-4613-0C3A0074",
     [("16", 25, "Worn Shortsword", 135274, 1), ("4", 38, "Recruit's Shirt", 135009, 1)], "Elwynn Forest"),
]

# A few more evenings for the alts, so they have days to write about; the
# "collect" steps are the mounts and companions they brought home (account.json).
def Q(act, qid, title, **kw):
    return ("quest", dict(act=act, id=qid, title=title, **kw))

def Z(zone, sub=""):
    return ("zone", {"zone": zone, "sub": sub})

def SUB(zone, sub):
    return ("subzone", {"zone": zone, "sub": sub})

ALT_LIFE = {
    "Usain": [
        (local(2026, 9, 21, 14, 10), [
            Z("Orgrimmar", "Valley of Spirits"), ("open", {"what": "trainer"}),
            ("gossip", {"name": "Uthel'nay", "npc": 7311}), ("spell", {"id": 1461}), ("spell", {"id": 865}),
            SUB("Orgrimmar", "Valley of Honor"), ("gossip", {"name": "Ogunaro Wolfbinder", "npc": 3362}),
            ("open", {"what": "merchant"}), ("money", {"ctx": "merchant", "d": -35000}),
            ("collect", {"kind": "mounts", "key": 14, "name": "Timber Wolf", "icon": 132224}),
            Z("The Barrens", "The Crossroads"), ("gossip", {"name": "Mankrik", "npc": 3432}),
            Q("accept", 899, "Consumed by Hatred", text="The Kolkar did this. They raided our camp and took her from me. I cannot rest while they still walk the Barrens. Bring me their beads, as many as the dead have fingers.", objective="Bring 12 Kolkar Heads to Mankrik at the Crossroads."),
            Q("accept", 4021, "Lost in Battle", text="My wife went out with the caravan to Lushwater Oasis. The Kolkar came. She did not come back. Find her. Whatever is left, find her.", objective="Find Mankrik's wife."),
            SUB("The Barrens", "Lushwater Oasis"), ("equip", {"link": link(91501, "Emberwood Staff", 2, 20), "slot": 16}),
        ], 8400),
        (local(2026, 9, 24, 13, 30), [
            Z("The Barrens", "Lushwater Oasis"), ("death", {}), ("alive", {}),
            Q("turnin", 4021, "Lost in Battle"), SUB("The Barrens", "The Crossroads"),
            ("gossip", {"name": "Mankrik", "npc": 3432}), Q("turnin", 899, "Consumed by Hatred", money=1800),
            ("money", {"ctx": "quest", "d": 1800}),
            ("msg", {"kind": "skill", "text": "Your skill in Tailoring has increased to 150."}),
        ], 7200),
    ],
    "Elijah": [
        (local(2026, 9, 20, 13, 0), [
            Z("The Barrens", "The Forgotten Pools"), Q("turnin", 870, "The Forgotten Pools", money=1200),
            Q("accept", 867, "Harpy Raiders", text="The Witchwing harpies nest in the rocks to the west and pick off our riders. Their talons would make a fine warning to the rest.", objective="Bring 8 Witchwing Talons to Darsok Swiftdagger."),
            ("equip", {"link": link(91503, "Voodoo Hexstaff", 2, 20), "slot": 16}),
            SUB("The Barrens", "The Crossroads"), ("gossip", {"name": "Darsok Swiftdagger", "npc": 3449}),
            Q("turnin", 867, "Harpy Raiders", money=1500),
        ], 6600),
        (local(2026, 9, 23, 14, 0), [
            Z("Tirisfal Glades", "Brill"), Z("Silverpine Forest", "The Sepulcher"),
            ("gossip", {"name": "Dalar Dawnweaver", "npc": 1938}),
            Q("accept", 99, "Arugal's Folly", text="The wizard Arugal thought his wolves would save Lordaeron. Now Pyrewood howls at night and the Dalaran mages at Ambermill guard secrets nobody should keep. Bring me their spellbooks.", objective="Bring the Dalaran spellbooks from Ambermill to Dalar Dawnweaver."),
            SUB("Silverpine Forest", "Ambermill"), ("item", {"item": (8491, "Cat Carrier (Black Tabby)", 132599, 1)}),
            ("collect", {"kind": "pets", "key": 42, "name": "Black Tabby Cat", "icon": 132599}),
            ("death", {}), ("alive", {}),
        ], 7800),
    ],
    "Oprah": [
        (local(2026, 9, 19, 14, 30), [
            Z("Loch Modan", "Thelsamar"), ("gossip", {"name": "Mountaineer Kadrell", "npc": 1340}),
            Q("turnin", 436, "Report to Ironforge", money=900),
            ("msg", {"kind": "skill", "text": "Your skill in Engineering has increased to 146."}),
            ("msg", {"kind": "skill", "text": "Your skill in Engineering has increased to 150."}),
            ("item", {"item": (4401, "Mechanical Squirrel Box", 132599, 1), "ctx": "craft"}),
            ("collect", {"kind": "pets", "key": 39, "name": "Mechanical Squirrel", "icon": 134063}),
            SUB("Loch Modan", "Ironband's Excavation Site"),
            Q("accept", 468, "Excavation Progress Report", text="Prospector Ironband wants word carried to Ironforge that the dig goes well. It does not go well. Carry the word anyway.", objective="Take the report to Ironforge."),
        ], 7000),
        (local(2026, 9, 22, 13, 45), [
            Z("Wetlands", "Menethil Harbor"), ("gossip", {"name": "Karl Boran", "npc": 1242}),
            Q("accept", 484, "Young Crocolisk Skins", text="The young crocolisks of the marsh make the finest leather this side of the sea. Bring me their skins and I will make it worth your while.", objective="Bring 4 Young Crocolisk Skins to Karl Boran in Menethil Harbor."),
            SUB("Wetlands", "Bluegill Marsh"), ("death", {}), ("alive", {}),
            SUB("Wetlands", "Menethil Harbor"), Q("turnin", 484, "Young Crocolisk Skins", money=1400),
        ], 6300),
    ],
}

def alt_snapshot(template, a, when):
    first, sur, race, race_file, race_id, cls, cls_file, cls_id, guid, gear, zone = a
    snap = {k: v for k, v in template.items()}
    eq = {}
    for slot, iid, name, icon, q in gear:
        eq[slot] = {"id": iid, "icon": icon, "quality": q, "link": link(iid, name, q, 20),
                    "tooltip": tip(name, q, [{"l": "Requires Level 18", "lc": "ffffff"}]), "firstSeen": int(when)}
    level = 1 if first == "Jon" else 20
    snap.update(
        name=first, surname=sur, displayName=f"{first} {sur}", guid=guid, realm=REALM, level=level,
        race=race, raceFile=race_file, raceID=race_id, **{"class": cls}, classFile=cls_file, classID=cls_id,
        faction="Alliance" if race in ("Dwarf", "Human") else "Horde", money=rng.randint(20000, 90000), zone=zone,
        xp={"cur": 0, "max": 0}, updated=int(when), loggedOut=int(when), equipment=eq,
        stats=[g for g in STATS if g["name"] in ("General", "Primary Attributes")],
        talents={"active": 1, "specs": []}, legacy=legacy(template),
        skills=skills(template, PROFESSIONS[first]),
    )
    return snap

# ------------------------------------------------------------------ output

def dump(path, v):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        json.dump(v, f, indent=2, ensure_ascii=False, sort_keys=True)
        f.write("\n")

def main():
    template = json.load(open(TEMPLATE))
    s = simulate()
    arch = os.path.join(OUT, "archive")
    shutil.rmtree(OUT, ignore_errors=True)
    tom = os.path.join(arch, "characters", "tom-crusader")
    dump(os.path.join(tom, "snapshot.json"), snapshot(template, s))
    dump(os.path.join(tom, "seen.json"), s.seen)
    dump(os.path.join(tom, "questlog.json"), list(s.questlog.values()))
    by_day = {}
    for r in s.rows:
        by_day.setdefault(datetime.fromtimestamp(r["t"]).strftime("%Y-%m-%d"), []).append(r)
    for day, rows in by_day.items():
        os.makedirs(os.path.join(tom, "log"), exist_ok=True)
        with open(os.path.join(tom, "log", f"{day}.jsonl"), "w") as f:
            for r in rows:
                f.write(json.dumps(r, ensure_ascii=False, sort_keys=True) + "\n")
    with open(os.path.join(tom, "personality.md"), "w") as f:
        f.write("One of the first Forsaken to take up the shield at Bandarion Keep. Stubborn, dry, dutiful. "
                "Keeps count of the living he has kept alive rather than the dead he has made. "
                "Distrusts sermons, trusts a good shield and the people behind it. "
                "Still not sure the Light should answer him, and a little afraid of the day it stops.\n")
    # Diary entries written earlier for the demo (by the app's own writer).
    saved = os.path.join(os.path.dirname(os.path.abspath(__file__)), "diary")
    if os.path.isdir(saved):
        shutil.copytree(saved, os.path.join(tom, "diary"))
    n = s.n
    alt_quests = {}
    collected = []
    for i, a in enumerate(ALTS):
        when = SESSION_TIMES[i][0] - 3600 * (i + 2)
        slug = f"{a[0]}-{a[1]}".lower()
        d = os.path.join(arch, "characters", slug)
        lvl = 1 if a[0] == "Jon" else 20
        money = 0 if lvl == 1 else 50000
        by_day = {}
        evenings = [(when, [("zone", {"zone": a[10], "sub": ""})], 300 if lvl == 1 else 2400)] + ALT_LIFE.get(a[0], [])
        for start, steps, length in sorted(evenings, key=lambda e: e[0]):
            t = int(start)
            day = datetime.fromtimestamp(t).strftime("%Y-%m-%d")
            rows = by_day.setdefault(day, [])
            n += 1
            rows.append({"e": "login", "level": lvl, "money": money, "n": n, "t": t, "xp": 0, "zone": steps[0][1].get("zone", a[10])})
            for e, row in steps:
                t += rng.randint(240, 900)
                if e == "collect":
                    collected.append((row["kind"], row["key"], row["name"], row["icon"], t, a[8]))
                    continue
                n += 1
                if e == "quest" and "text" in row:
                    alt_quests[row["id"]] = {"title": row["title"], "text": row.pop("text"), "objective": row.pop("objective", ""),
                                             "level": 20, "seen": t}
                if e == "item":
                    iid, name, icon, q = row.pop("item")
                    s.items[iid] = {"name": name, "icon": icon, "q": q}
                    row.update(ctx=row.get("ctx", "loot"), d=1, key=str(iid), link=link(iid, name, q, lvl), total=1)
                if e == "money":
                    money += row["d"]
                    row["total"] = money
                rows.append({**row, "e": e, "n": n, "t": t})
            n += 1
            rows.append({"e": "logout", "money": money, "n": n, "t": int(start + length), "zone": steps[-1][1].get("zone", a[10])})
        snap = alt_snapshot(template, a, max(r["t"] for rs in by_day.values() for r in rs))
        dump(os.path.join(d, "snapshot.json"), snap)
        os.makedirs(os.path.join(d, "log"), exist_ok=True)
        for day, rows in by_day.items():
            with open(os.path.join(d, "log", day + ".jsonl"), "w") as f:
                for r in rows:
                    f.write(json.dumps(r, ensure_ascii=False, sort_keys=True) + "\n")
    s.quest_texts.update(alt_quests)
    # Account-wide collections (account.json, addon 0.3.0): who brought each home, and when.
    brill = next(r["t"] for r in s.rows if r["e"] == "subzone" and r.get("sub") == "Brill" and r["t"] > SESSION_TIMES[5][0])
    collected.insert(0, ("mounts", 5, "Skeletal Horse", 132264, brill, ME))
    account = {}
    for kind, key, name, icon, t, guid in collected:
        account.setdefault(kind, {})[str(key)] = {"name": name, "icon": icon, "first": int(t), "by": guid}
    dump(os.path.join(arch, "account.json"), account)
    # Letters between them, written earlier for the demo (by the app's own writer).
    saved = os.path.join(os.path.dirname(os.path.abspath(__file__)), "letters")
    if os.path.isdir(saved):
        shutil.copytree(saved, os.path.join(arch, "letters"))
    for qid, q in s.quest_texts.items():
        dump(os.path.join(arch, "quests", f"{qid}.json"), q)
    dump(os.path.join(arch, "items.json"), {str(k): v for k, v in s.items.items()})
    players = {}
    for g, first, sur, cls, race, level, guild in PARTY + STRANGERS:
        players[g] = {"name": first, "surname": sur, "class": cls, "race": race, "level": level, "guild": guild or None, "seen": int(s.t)}
    dump(os.path.join(arch, "players.json"), {k: {kk: vv for kk, vv in v.items() if vv is not None} for k, v in players.items()})
    dump(os.path.join(arch, "gossip.json"), {
        "5680:Light keep you, Forsaken. Few would have believed it.": {"name": "Shari Stilwell", "npc": 5680, "seen": int(s.t),
            "text": "Light keep you, Forsaken. Few would have believed it.", "options": ["I would like to train."]},
        "248850:Again. Your shield arm drops every time you swing.": {"name": "Hilda the Breaker", "npc": 248850, "seen": int(s.t),
            "text": "Again. Your shield arm drops every time you swing.", "options": ["I would like to train.", "Tell me about Bandarion Keep."]},
    })
    dump(os.path.join(arch, "state.json"), {"lastSeq": s.n})
    # Native logs, archived per session, as the recorder leaves them.
    for kind, per in (("combat", s.combat), ("chat", s.chat)):
        os.makedirs(os.path.join(OUT, "logs", kind), exist_ok=True)
        for i, lines in per.items():
            start = datetime.fromtimestamp(SESSION_TIMES[max(i, 0)][0])
            name = "WoWCombatLog.txt" if kind == "combat" else "WoWChatLog.txt"
            with open(os.path.join(OUT, "logs", kind, start.strftime("%Y-%m-%dT%H%M%S") + "-" + name), "w") as f:
                f.write("\n".join(lines) + "\n")
    # A stand-in game folder: the real game data (for art), nothing else.
    game = os.path.join(OUT, "game")
    os.makedirs(os.path.join(game, "_classic_beta_", "Logs"), exist_ok=True)
    for name in (".build.info", "Data"):
        os.symlink(os.path.join(GAME, name), os.path.join(game, name))
    shutil.copy(os.path.join(GAME, "_classic_beta_", ".flavor.info"), os.path.join(game, "_classic_beta_", ".flavor.info"))
    dump(os.path.join(OUT, "config", "forever-memory", "settings.json"), {
        "game_dir": game, "flavor": "_classic_beta_", "archive": arch, "raw_logs": os.path.join(OUT, "logs"),
        "language": "en", "record": False, "archive_logs": False, "writer": {"kind": "cli", "command": "claude"},
        "elevenlabs_key": "demo-not-a-real-key", "narration_speed": 1.0, "s3": {"enabled": False, "endpoint": "", "region": "", "bucket": "",
        "access_key": "", "secret_key": ""}, "onboarded": True})
    with open(os.path.join(OUT, "run.sh"), "w") as f:
        f.write(f"""#!/bin/sh
# Forever Memory on the demo data; art comes from the shared cache.
export XDG_CONFIG_HOME="{OUT}/config" XDG_DATA_HOME="{OUT}/data"
exec "${{FM_BIN:-forever-memory}}" "$@"
""")
    os.chmod(os.path.join(OUT, "run.sh"), 0o755)
    played = sum(e - s0 for s0, e in SESSION_TIMES)
    print(f"{s.n} events, level {s.level}, {s.kills} kills, {s.deaths} deaths, {len(s.done)} quests done, "
          f"{s.money // 10000}g {s.money // 100 % 100}s, {sum(len(v) for v in s.combat.values())} combat lines, "
          f"{sum(len(v) for v in s.chat.values())} chat lines, {played / 3600:.1f} h in sessions")

if __name__ == "__main__":
    main()
