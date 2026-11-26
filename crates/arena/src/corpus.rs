//! Built-in text corpus. Generic game text only: nothing copied from the background games,
//! so a read of background text can never match a truth item by accident.

use crate::rng::Rng;

pub const SPEAKERS: &[&str] = &[
    "Mara", "Old Tobin", "Captain Reyes", "Elder Wynn", "Kael", "Sister Ilse", "Garrick", "The Stranger",
    "Juno", "Merchant Fenn", "Ash", "Commander Vale", "Lyra", "Brother Oskar", "Nyx", "Dr. Halvorsen",
];

pub const DIALOGUE: &[&str] = &[
    "You finally made it. I was starting to think the storm got you.",
    "The bridge to the north is out. We need another way across.",
    "Keep your voice down. The guards change shifts at midnight.",
    "I can sell you the map, but it will cost you more than gold.",
    "Did you hear that? Something is moving in the old mine.",
    "Take this lantern. You will need it where you are going.",
    "My brother never came back from the eastern ridge.",
    "We have enough supplies for three more days, maybe four.",
    "The council will never agree to this plan.",
    "If you find the second key, bring it straight to me.",
    "Stay close to the wall and do not look down.",
    "That sword belonged to my father. Treat it well.",
    "The reactor is unstable. We have ten minutes at most.",
    "Nobody has opened that door in two hundred years.",
    "I knew you would come back. You always do.",
    "Follow the river until you reach the broken tower.",
    "Hold on, the signal is breaking up. Say that again?",
    "You are not from around here, are you?",
    "Our scouts spotted smoke near the southern farms.",
    "Rest now. Tomorrow we march at first light.",
    "The price of iron has doubled since the war began.",
    "I only ask one thing: bring her home safely.",
    "Watch out! There is another one behind you!",
    "This ship will not survive another jump like that.",
    "Every name on this wall is someone we lost.",
    "The prophecy speaks of a traveller with no past.",
    "Quiet. Let me do the talking when we get inside.",
    "Thank you. I will not forget what you did today.",
    "Go to the lighthouse and light the signal fire.",
    "So you are the one everyone keeps talking about.",
    "The old road is safer, but it takes twice as long.",
    "Three of my men are still trapped inside the fort.",
    "Coffee first. Then we can talk about saving the world.",
    "Hand me that wrench, would you? No, the other one.",
    "The north gate opens only for those who carry the seal.",
    "I have seen that symbol before, in my grandmother's book.",
];

pub const SHORT_LINES: &[&str] = &[
    "Over here!", "Behind you!", "Got it.", "Move, move!", "Not again...", "Wait for me!",
    "Reloading!", "Hold the line!", "Who goes there?", "Follow me.", "That was close.", "Run!",
];

pub const PICKUP_ITEMS: &[&str] = &[
    "Iron Ore", "Copper Ore", "Healing Herb", "Silver Coin", "Arrow", "Bandage", "Rope", "Gunpowder",
    "Ancient Relic", "Mana Crystal", "Steel Plate", "Fire Opal", "Bread", "Rusty Gear", "Battery",
    "Spider Silk", "Wolf Pelt", "Quartz Shard", "Cloth Scrap", "Oak Plank", "Glass Vial", "Feather",
];

pub const OBJECTIVES: &[&str] = &[
    "Find the missing scout", "Reach the north tower", "Collect 5 mana crystals", "Defeat the bandit leader",
    "Talk to the blacksmith", "Escape the burning village", "Repair the generator", "Escort the caravan",
    "Search the abandoned lab", "Return to camp before nightfall", "Light the three beacons",
    "Open the sealed vault", "Investigate the strange signal", "Deliver the letter to Elder Wynn",
    "Survive until dawn", "Clear the mine entrance", "Craft a better weapon", "Cross the frozen lake",
];

pub const OBJECTIVE_PREFIX: &[&str] = &["New Objective:", "Objective Updated:", "Quest:", "Main Quest:", ""];

pub const TOOLTIP_TITLES: &[&str] = &[
    "Rusty Longsword", "Leather Boots", "Potion of Swiftness", "Iron Helm", "Hunter's Bow", "Spell Tome",
    "Cracked Shield", "Ring of Embers", "Travel Cloak", "Signal Flare", "Scrap Pistol", "Field Kit",
];

pub const TOOLTIP_LINES: &[&str] = &[
    "Damage 12-18", "Armor +4", "Weight 2.5", "Restores 40 health", "Move speed +15% for 30 s", "Durability 37/50",
    "Requires level 6", "Sell value 120", "Two-handed", "Deals fire damage", "Stackable", "Quest item",
];

pub const PROMPTS: &[&str] = &[
    "Press [E] to open", "Press [F] to pick up", "Hold [E] to interact", "[E] Talk", "[F] Loot",
    "Press [R] to reload", "[Space] Jump", "Hold [Q] to use", "[E] Climb", "Press [G] to drop",
];

pub const MENU_ENTRIES: &[&str] = &[
    "Resume", "Inventory", "Map", "Journal", "Skills", "Settings", "Save Game", "Load Game", "Controls",
    "Quit to Title", "Audio", "Graphics",
];

pub const HUD_LABELS: &[&str] = &["HP", "MP", "STA", "AMMO", "GOLD", "LV", "XP", "SHIELD"];

pub fn dialogue_line(rng: &mut Rng) -> String {
    if rng.chance(0.2) {
        rng.pick(SHORT_LINES).to_string()
    } else {
        rng.pick(DIALOGUE).to_string()
    }
}

pub fn speaker(rng: &mut Rng) -> String {
    rng.pick(SPEAKERS).to_string()
}

pub fn objective(rng: &mut Rng) -> String {
    let p = *rng.pick(OBJECTIVE_PREFIX);
    let o = *rng.pick(OBJECTIVES);
    if p.is_empty() { o.to_string() } else { format!("{p} {o}") }
}

/// Every text `objective` can return.
pub fn all_objectives() -> Vec<String> {
    OBJECTIVE_PREFIX
        .iter()
        .flat_map(|p| OBJECTIVES.iter().map(move |o| if p.is_empty() { o.to_string() } else { format!("{p} {o}") }))
        .collect()
}

pub fn tooltip(rng: &mut Rng) -> String {
    let mut s = rng.pick(TOOLTIP_TITLES).to_string();
    for _ in 0..rng.int(1, 3) {
        s.push('\n');
        s.push_str(rng.pick(TOOLTIP_LINES));
    }
    s
}

pub fn pickup_text(name: &str, n: i64, total: Option<i64>) -> String {
    match total {
        Some(t) => format!("+ {n} {name} ({t})"),
        None => format!("+ {n} {name}"),
    }
}

pub fn damage_number(rng: &mut Rng) -> String {
    let n = rng.int(3, 999);
    match rng.int(0, 3) {
        0 => format!("-{n}"),
        1 => format!("CRIT {n}"),
        _ => n.to_string(),
    }
}
