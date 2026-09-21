/**
 * Canonical card metadata catalog for Twilight Imperium 4.
 * Derived verbatim from ti4-content corpus.
 */

export interface StrategyCardMeta {
  id: string;
  name: string;
  initiative: number;
  primaryText: string;
  secondaryText: string;
}

export interface ObjectiveMeta {
  id: string;
  name: string;
  phase: string;
  points: number;
  description: string;
}

export interface CardMeta {
  id: string;
  name: string;
  phase?: string;
  description: string;
}

export const STRATEGY_CARDS: Record<string, StrategyCardMeta> = {
  "base2": {
    "id": "base2",
    "name": "Diplomacy",
    "initiative": 2,
    "primaryText": "Choose 1 system other than the Mecatol Rex system that contains a planet you control; each other player places a command token from their reinforcements in the chosen system.\nThen, ready each exhausted planet you control in that system.",
    "secondaryText": "Spend 1 token from your strategy pool to ready up to 2 exhausted planets"
  },
  "base4": {
    "id": "base4",
    "name": "Construction",
    "initiative": 4,
    "primaryText": "Place 1 PDS or 1 Space Dock on a planet you control.\nPlace 1 PDS on a planet you control.",
    "secondaryText": "Place 1 token from your strategy pool in any system; you may place either 1 space dock or 1 PDS on a planet you control in that system."
  },
  "pok4construction": {
    "id": "pok4construction",
    "name": "Construction",
    "initiative": 4,
    "primaryText": "Place 1 PDS or 1 Space Dock on a planet you control.\nPlace 1 PDS on a planet you control.",
    "secondaryText": "Spend 1 token from your strategy pool and place it in any system; you may place either 1 space dock or 1 PDS on a planet you control in that system"
  },
  "pok1leadership": {
    "id": "pok1leadership",
    "name": "Leadership",
    "initiative": 1,
    "primaryText": "Gain 3 command tokens\nSpend any amount of influence to gain 1 command token for every 3 influence spent",
    "secondaryText": "Spend any amount of influence to gain 1 command token for every 3 influence spent"
  },
  "pok2diplomacy": {
    "id": "pok2diplomacy",
    "name": "Diplomacy",
    "initiative": 2,
    "primaryText": "Choose 1 system other than the Mecatol Rex system that contains a planet you control; each other player places a command token from their reinforcements in the chosen system. Then, ready up to 2 exhausted planets you control.",
    "secondaryText": "Spend 1 token from your strategy pool to ready up to 2 exhausted planets you control."
  },
  "pok3politics": {
    "id": "pok3politics",
    "name": "Politics",
    "initiative": 3,
    "primaryText": "Choose a player other than the speaker.  That player gains the speaker token.\nDraw 2 action cards\nLook at the top 2 cards of the agenda deck. Place each card on the top or bottom of the deck in any order.",
    "secondaryText": "Spend 1 token from your strategy pool to draw 2 action cards."
  },
  "pok5trade": {
    "id": "pok5trade",
    "name": "Trade",
    "initiative": 5,
    "primaryText": "Gain 3 trade goods.\nReplenish commodities.\nChoose any number of other players. Those players use the secondary ability of this strategy card without spending a command token.",
    "secondaryText": "Spend 1 token from your strategy pool to replenish your commodities."
  },
  "pok6warfare": {
    "id": "pok6warfare",
    "name": "Warfare",
    "initiative": 6,
    "primaryText": "Remove 1 of your command tokens from the game board; then, gain 1 command token.\nRedistribute any number of the command tokens on your command sheet.",
    "secondaryText": "Spend 1 token from your strategy pool to use the PRODUCTION ability of 1 of your space docks in your home system (this token is not placed in your home system)."
  },
  "pok7technology": {
    "id": "pok7technology",
    "name": "Technology",
    "initiative": 7,
    "primaryText": "Research 1 technology.\nSpend 6 resources to research 1 technology.",
    "secondaryText": "Spend 1 token from your strategy pool and 4 resources to research 1 technology."
  },
  "pok8imperial": {
    "id": "pok8imperial",
    "name": "Imperial",
    "initiative": 8,
    "primaryText": "Immediately score 1 public objective if you fulfill its requirements.\nGain 1 victory point if you control Mecatol Rex; otherwise, draw 1 secret objective.",
    "secondaryText": "Spend 1 token from your strategy pool to draw 1 secret objective."
  },
  "te4construction": {
    "id": "te4construction",
    "name": "Construction",
    "initiative": 4,
    "primaryText": "Either place 1 structure on a planet you control, or use the PRODUCTION ability of 1 of your space docks.\nPlace 1 structure on a planet you control.",
    "secondaryText": "Spend 1 token from your strategy pool to place 1 structure on a planet you control."
  },
  "te6warfare": {
    "id": "te6warfare",
    "name": "Warfare",
    "initiative": 6,
    "primaryText": "Perform a tactical action in any system without placing a command token, even if the system already has your command token in it: that system still counts as being activated. You may redistribute your command tokens before and after this action.",
    "secondaryText": "Spend 1 token from your strategy pool to use the Production abilities of the units in your home system. (This token is not placed in your home system)"
  }
};

export const SECRET_OBJECTIVES: Record<string, ObjectiveMeta> = {
  "ans": {
    "id": "ans",
    "name": "Adapt New Strategies",
    "phase": "Status",
    "points": 1,
    "description": "Own 2 faction technologies. 'Valefar Assimilator' technologies do not count toward this objective."
  },
  "adapt_new_strategies": {
    "id": "ans",
    "name": "Adapt New Strategies",
    "phase": "Status",
    "points": 1,
    "description": "Own 2 faction technologies. 'Valefar Assimilator' technologies do not count toward this objective."
  },
  "btgk": {
    "id": "btgk",
    "name": "Become the Gatekeeper",
    "phase": "Status",
    "points": 1,
    "description": "Have 1 or more ships in a system that contains an alpha wormhole and 1 or more ships in a system that contains a beta wormhole."
  },
  "become_the_gatekeeper": {
    "id": "btgk",
    "name": "Become the Gatekeeper",
    "phase": "Status",
    "points": 1,
    "description": "Have 1 or more ships in a system that contains an alpha wormhole and 1 or more ships in a system that contains a beta wormhole."
  },
  "ctr": {
    "id": "ctr",
    "name": "Control the Region",
    "phase": "Status",
    "points": 1,
    "description": "Have 1 or more ships in 6 systems."
  },
  "control_the_region": {
    "id": "ctr",
    "name": "Control the Region",
    "phase": "Status",
    "points": 1,
    "description": "Have 1 or more ships in 6 systems."
  },
  "csl": {
    "id": "csl",
    "name": "Cut Supply Lines",
    "phase": "Status",
    "points": 1,
    "description": "Have 1 or more ships in the same system as another player's space dock."
  },
  "cut_supply_lines": {
    "id": "csl",
    "name": "Cut Supply Lines",
    "phase": "Status",
    "points": 1,
    "description": "Have 1 or more ships in the same system as another player's space dock."
  },
  "dtgs": {
    "id": "dtgs",
    "name": "Destroy Their Greatest Ship",
    "phase": "Action",
    "points": 1,
    "description": "Destroy another player's war sun or flagship."
  },
  "destroy_their_greatest_ship": {
    "id": "dtgs",
    "name": "Destroy Their Greatest Ship",
    "phase": "Action",
    "points": 1,
    "description": "Destroy another player's war sun or flagship."
  },
  "eap": {
    "id": "eap",
    "name": "Establish a Perimeter",
    "phase": "Status",
    "points": 1,
    "description": "Have 4 PDS units on the game board."
  },
  "establish_a_perimeter": {
    "id": "eap",
    "name": "Establish a Perimeter",
    "phase": "Status",
    "points": 1,
    "description": "Have 4 PDS units on the game board."
  },
  "faa": {
    "id": "faa",
    "name": "Forge an Alliance",
    "phase": "Status",
    "points": 1,
    "description": "Control 4 cultural planets."
  },
  "forge_an_alliance": {
    "id": "faa",
    "name": "Forge an Alliance",
    "phase": "Status",
    "points": 1,
    "description": "Control 4 cultural planets."
  },
  "fsn": {
    "id": "fsn",
    "name": "Form a Spy Network",
    "phase": "Status",
    "points": 1,
    "description": "Discard 5 action cards."
  },
  "form_a_spy_network": {
    "id": "fsn",
    "name": "Form a Spy Network",
    "phase": "Status",
    "points": 1,
    "description": "Discard 5 action cards."
  },
  "fwm": {
    "id": "fwm",
    "name": "Fuel the War Machine",
    "phase": "Status",
    "points": 1,
    "description": "Have 3 space docks on the game board."
  },
  "fuel_the_war_machine": {
    "id": "fwm",
    "name": "Fuel the War Machine",
    "phase": "Status",
    "points": 1,
    "description": "Have 3 space docks on the game board."
  },
  "gamf": {
    "id": "gamf",
    "name": "Gather a Mighty Fleet",
    "phase": "Status",
    "points": 1,
    "description": "Have 5 dreadnoughts on the game board."
  },
  "gather_a_mighty_fleet": {
    "id": "gamf",
    "name": "Gather a Mighty Fleet",
    "phase": "Status",
    "points": 1,
    "description": "Have 5 dreadnoughts on the game board."
  },
  "lsc": {
    "id": "lsc",
    "name": "Learn the Secrets of the Cosmos",
    "phase": "Status",
    "points": 1,
    "description": "Have 1 or more ships in 3 systems that are each adjacent to an anomaly."
  },
  "learn_the_secrets_of_the_cosmos": {
    "id": "lsc",
    "name": "Learn the Secrets of the Cosmos",
    "phase": "Status",
    "points": 1,
    "description": "Have 1 or more ships in 3 systems that are each adjacent to an anomaly."
  },
  "mew": {
    "id": "mew",
    "name": "Make an Example of Their World",
    "phase": "Action",
    "points": 1,
    "description": "Destroy the last of a player's ground forces on a planet during the bombardment step."
  },
  "make_an_example_of_their_world": {
    "id": "mew",
    "name": "Make an Example of Their World",
    "phase": "Action",
    "points": 1,
    "description": "Destroy the last of a player's ground forces on a planet during the bombardment step."
  },
  "mlp": {
    "id": "mlp",
    "name": "Master the Laws of Physics",
    "phase": "Status",
    "points": 1,
    "description": "Own 4 technologies of the same color."
  },
  "master_the_laws_of_physics": {
    "id": "mlp",
    "name": "Master the Laws of Physics",
    "phase": "Status",
    "points": 1,
    "description": "Own 4 technologies of the same color."
  },
  "mrm": {
    "id": "mrm",
    "name": "Mine Rare Metals",
    "phase": "Status",
    "points": 1,
    "description": "Control 4 hazardous planets."
  },
  "mine_rare_metals": {
    "id": "mrm",
    "name": "Mine Rare Metals",
    "phase": "Status",
    "points": 1,
    "description": "Control 4 hazardous planets."
  },
  "mp": {
    "id": "mp",
    "name": "Monopolize Production",
    "phase": "Status",
    "points": 1,
    "description": "Control 4 industrial planets."
  },
  "monopolize_production": {
    "id": "mp",
    "name": "Monopolize Production",
    "phase": "Status",
    "points": 1,
    "description": "Control 4 industrial planets."
  },
  "ose": {
    "id": "ose",
    "name": "Occupy the Seat of the Empire",
    "phase": "Status",
    "points": 1,
    "description": "Control Mecatol Rex and have 3 or more ships in its system."
  },
  "occupy_the_seat_of_the_empire": {
    "id": "ose",
    "name": "Occupy the Seat of the Empire",
    "phase": "Status",
    "points": 1,
    "description": "Control Mecatol Rex and have 3 or more ships in its system."
  },
  "sar": {
    "id": "sar",
    "name": "Spark a Rebellion",
    "phase": "Action",
    "points": 1,
    "description": "Win a combat against a player who has the most victory points."
  },
  "spark_a_rebellion": {
    "id": "sar",
    "name": "Spark a Rebellion",
    "phase": "Action",
    "points": 1,
    "description": "Win a combat against a player who has the most victory points."
  },
  "te": {
    "id": "te",
    "name": "Threaten Enemies",
    "phase": "Status",
    "points": 1,
    "description": "Have 1 or more ships in a system that is adjacent to another player's home system."
  },
  "threaten_enemies": {
    "id": "te",
    "name": "Threaten Enemies",
    "phase": "Status",
    "points": 1,
    "description": "Have 1 or more ships in a system that is adjacent to another player's home system."
  },
  "ttfd": {
    "id": "ttfd",
    "name": "Turn Their Fleets to Dust",
    "phase": "Action",
    "points": 1,
    "description": "Destroy the last of a player's non-fighter ships in the active system during the space cannon offense step."
  },
  "turn_their_fleets_to_dust": {
    "id": "ttfd",
    "name": "Turn Their Fleets to Dust",
    "phase": "Action",
    "points": 1,
    "description": "Destroy the last of a player's non-fighter ships in the active system during the space cannon offense step."
  },
  "uf": {
    "id": "uf",
    "name": "Unveil Flagship",
    "phase": "Action",
    "points": 1,
    "description": "Win a space combat in a system that contains your flagship. You cannot score this objective if your flagship is destroyed in the combat."
  },
  "unveil_flagship": {
    "id": "uf",
    "name": "Unveil Flagship",
    "phase": "Action",
    "points": 1,
    "description": "Win a space combat in a system that contains your flagship. You cannot score this objective if your flagship is destroyed in the combat."
  },
  "bam": {
    "id": "bam",
    "name": "Become a Martyr",
    "phase": "Action",
    "points": 1,
    "description": "Lose control of a planet in a home system."
  },
  "become_a_martyr": {
    "id": "bam",
    "name": "Become a Martyr",
    "phase": "Action",
    "points": 1,
    "description": "Lose control of a planet in a home system."
  },
  "baf": {
    "id": "baf",
    "name": "Betray a Friend",
    "phase": "Action",
    "points": 1,
    "description": "Win a combat against a player whose promissory note you had in your play area at the start of your tactical action."
  },
  "betray_a_friend": {
    "id": "baf",
    "name": "Betray a Friend",
    "phase": "Action",
    "points": 1,
    "description": "Win a combat against a player whose promissory note you had in your play area at the start of your tactical action."
  },
  "btv": {
    "id": "btv",
    "name": "Brave the Void",
    "phase": "Action",
    "points": 1,
    "description": "Win a combat in an anomaly."
  },
  "brave_the_void": {
    "id": "btv",
    "name": "Brave the Void",
    "phase": "Action",
    "points": 1,
    "description": "Win a combat in an anomaly."
  },
  "dts": {
    "id": "dts",
    "name": "Darken the Skies",
    "phase": "Action",
    "points": 1,
    "description": "Win a combat in another player's home system."
  },
  "darken_the_skies": {
    "id": "dts",
    "name": "Darken the Skies",
    "phase": "Action",
    "points": 1,
    "description": "Win a combat in another player's home system."
  },
  "dfat": {
    "id": "dfat",
    "name": "Defy Space and Time",
    "phase": "Status",
    "points": 1,
    "description": "Have units in the wormhole nexus."
  },
  "defy_space_and_time": {
    "id": "dfat",
    "name": "Defy Space and Time",
    "phase": "Status",
    "points": 1,
    "description": "Have units in the wormhole nexus."
  },
  "dyp": {
    "id": "dyp",
    "name": "Demonstrate Your Power",
    "phase": "Action",
    "points": 1,
    "description": "Have 3 or more non-fighter ships in the active system at the end of a space combat."
  },
  "demonstrate_your_power": {
    "id": "dyp",
    "name": "Demonstrate Your Power",
    "phase": "Action",
    "points": 1,
    "description": "Have 3 or more non-fighter ships in the active system at the end of a space combat."
  },
  "dhw": {
    "id": "dhw",
    "name": "Destroy Heretical Works",
    "phase": "Status",
    "points": 1,
    "description": "Purge 2 of your relic fragments of any type."
  },
  "destroy_heretical_works": {
    "id": "dhw",
    "name": "Destroy Heretical Works",
    "phase": "Status",
    "points": 1,
    "description": "Purge 2 of your relic fragments of any type."
  },
  "dp": {
    "id": "dp",
    "name": "Dictate Policy",
    "phase": "Agenda",
    "points": 1,
    "description": "There are 3 or more laws in play."
  },
  "dictate_policy": {
    "id": "dp",
    "name": "Dictate Policy",
    "phase": "Agenda",
    "points": 1,
    "description": "There are 3 or more laws in play."
  },
  "dtd": {
    "id": "dtd",
    "name": "Drive the Debate",
    "phase": "Agenda",
    "points": 1,
    "description": "You or a planet you control are elected by an agenda."
  },
  "drive_the_debate": {
    "id": "dtd",
    "name": "Drive the Debate",
    "phase": "Agenda",
    "points": 1,
    "description": "You or a planet you control are elected by an agenda."
  },
  "eh": {
    "id": "eh",
    "name": "Establish Hegemony",
    "phase": "Status",
    "points": 1,
    "description": "Control planets that have a combined influence value of at least 12."
  },
  "establish_hegemony": {
    "id": "eh",
    "name": "Establish Hegemony",
    "phase": "Status",
    "points": 1,
    "description": "Control planets that have a combined influence value of at least 12."
  },
  "fwp": {
    "id": "fwp",
    "name": "Fight with Precision",
    "phase": "Action",
    "points": 1,
    "description": "Destroy the last of a player's fighters in the active system during the anti-fighter barrage step."
  },
  "fight_with_precision": {
    "id": "fwp",
    "name": "Fight with Precision",
    "phase": "Action",
    "points": 1,
    "description": "Destroy the last of a player's fighters in the active system during the anti-fighter barrage step."
  },
  "fc": {
    "id": "fc",
    "name": "Foster Cohesion",
    "phase": "Status",
    "points": 1,
    "description": "Be neighbors with all other players."
  },
  "foster_cohesion": {
    "id": "fc",
    "name": "Foster Cohesion",
    "phase": "Status",
    "points": 1,
    "description": "Be neighbors with all other players."
  },
  "hrm": {
    "id": "hrm",
    "name": "Hoard Raw Materials",
    "phase": "Status",
    "points": 1,
    "description": "Control planets that have a combined resource value of at least 12."
  },
  "hoard_raw_materials": {
    "id": "hrm",
    "name": "Hoard Raw Materials",
    "phase": "Status",
    "points": 1,
    "description": "Control planets that have a combined resource value of at least 12."
  },
  "mtm": {
    "id": "mtm",
    "name": "Mechanize the Military",
    "phase": "Status",
    "points": 1,
    "description": "Have 1 mech on each of 4 planets."
  },
  "mechanize_the_military": {
    "id": "mtm",
    "name": "Mechanize the Military",
    "phase": "Status",
    "points": 1,
    "description": "Have 1 mech on each of 4 planets."
  },
  "otf": {
    "id": "otf",
    "name": "Occupy the Fringe",
    "phase": "Status",
    "points": 1,
    "description": "Have 9 or more ground forces on a planet that does not contain 1 of your space docks."
  },
  "occupy_the_fringe": {
    "id": "otf",
    "name": "Occupy the Fringe",
    "phase": "Status",
    "points": 1,
    "description": "Have 9 or more ground forces on a planet that does not contain 1 of your space docks."
  },
  "pem": {
    "id": "pem",
    "name": "Produce en Masse",
    "phase": "Status",
    "points": 1,
    "description": "Have units with a combined PRODUCTION value of at least 8 in a single system."
  },
  "produce_en_masse": {
    "id": "pem",
    "name": "Produce en Masse",
    "phase": "Status",
    "points": 1,
    "description": "Have units with a combined PRODUCTION value of at least 8 in a single system."
  },
  "pe": {
    "id": "pe",
    "name": "Prove Endurance",
    "phase": "Action",
    "points": 1,
    "description": "Be the last player to pass during a game round."
  },
  "prove_endurance": {
    "id": "pe",
    "name": "Prove Endurance",
    "phase": "Action",
    "points": 1,
    "description": "Be the last player to pass during a game round."
  },
  "sai": {
    "id": "sai",
    "name": "Seize an Icon",
    "phase": "Status",
    "points": 1,
    "description": "Control a legendary planet."
  },
  "seize_an_icon": {
    "id": "sai",
    "name": "Seize an Icon",
    "phase": "Status",
    "points": 1,
    "description": "Control a legendary planet."
  },
  "syc": {
    "id": "syc",
    "name": "Stake Your Claim",
    "phase": "Status",
    "points": 1,
    "description": "Control a planet in a system that contains a planet controlled by another player."
  },
  "stake_your_claim": {
    "id": "syc",
    "name": "Stake Your Claim",
    "phase": "Status",
    "points": 1,
    "description": "Control a planet in a system that contains a planet controlled by another player."
  },
  "sb": {
    "id": "sb",
    "name": "Strengthen Bonds",
    "phase": "Status",
    "points": 1,
    "description": "Have another player's promissory note in your play area."
  },
  "strengthen_bonds": {
    "id": "sb",
    "name": "Strengthen Bonds",
    "phase": "Status",
    "points": 1,
    "description": "Have another player's promissory note in your play area."
  }
};

export const PUBLIC_OBJECTIVES: Record<string, ObjectiveMeta> = {
  "corner": {
    "id": "corner",
    "name": "Corner the Market",
    "phase": "Status",
    "points": 1,
    "description": "Control 4 planets that each have the same planet trait."
  },
  "corner_the_market": {
    "id": "corner",
    "name": "Corner the Market",
    "phase": "Status",
    "points": 1,
    "description": "Control 4 planets that each have the same planet trait."
  },
  "develop": {
    "id": "develop",
    "name": "Develop Weaponry",
    "phase": "Status",
    "points": 1,
    "description": "Own 2 unit upgrade technologies."
  },
  "develop_weaponry": {
    "id": "develop",
    "name": "Develop Weaponry",
    "phase": "Status",
    "points": 1,
    "description": "Own 2 unit upgrade technologies."
  },
  "diversify": {
    "id": "diversify",
    "name": "Diversify Research",
    "phase": "Status",
    "points": 1,
    "description": "Own 2 technologies in each of 2 colors."
  },
  "diversify_research": {
    "id": "diversify",
    "name": "Diversify Research",
    "phase": "Status",
    "points": 1,
    "description": "Own 2 technologies in each of 2 colors."
  },
  "monument": {
    "id": "monument",
    "name": "Erect a Monument",
    "phase": "Status",
    "points": 1,
    "description": "Spend 8 resources."
  },
  "erect_a_monument": {
    "id": "monument",
    "name": "Erect a Monument",
    "phase": "Status",
    "points": 1,
    "description": "Spend 8 resources."
  },
  "expand_borders": {
    "id": "expand_borders",
    "name": "Expand Borders",
    "phase": "Status",
    "points": 1,
    "description": "Control 6 planets in non-home systems."
  },
  "research_outposts": {
    "id": "research_outposts",
    "name": "Found Research Outposts",
    "phase": "Status",
    "points": 1,
    "description": "Control 3 planets that have technology specialties."
  },
  "found_research_outposts": {
    "id": "research_outposts",
    "name": "Found Research Outposts",
    "phase": "Status",
    "points": 1,
    "description": "Control 3 planets that have technology specialties."
  },
  "intimidate": {
    "id": "intimidate",
    "name": "Intimidate Council",
    "phase": "Status",
    "points": 1,
    "description": "Have 1 or more ships in 2 systems that are adjacent to Mecatol Rex's system."
  },
  "intimidate_council": {
    "id": "intimidate",
    "name": "Intimidate Council",
    "phase": "Status",
    "points": 1,
    "description": "Have 1 or more ships in 2 systems that are adjacent to Mecatol Rex's system."
  },
  "lead": {
    "id": "lead",
    "name": "Lead From the Front",
    "phase": "Status",
    "points": 1,
    "description": "Spend a total of 3 tokens from your tactic and/or strategy pools."
  },
  "lead_from_the_front": {
    "id": "lead",
    "name": "Lead From the Front",
    "phase": "Status",
    "points": 1,
    "description": "Spend a total of 3 tokens from your tactic and/or strategy pools."
  },
  "trade_routes": {
    "id": "trade_routes",
    "name": "Negotiate Trade Routes",
    "phase": "Status",
    "points": 1,
    "description": "Spend 5 trade goods."
  },
  "negotiate_trade_routes": {
    "id": "trade_routes",
    "name": "Negotiate Trade Routes",
    "phase": "Status",
    "points": 1,
    "description": "Spend 5 trade goods."
  },
  "sway_council": {
    "id": "sway_council",
    "name": "Sway the Council",
    "phase": "Status",
    "points": 1,
    "description": "Spend 8 influence."
  },
  "sway_the_council": {
    "id": "sway_council",
    "name": "Sway the Council",
    "phase": "Status",
    "points": 1,
    "description": "Spend 8 influence."
  },
  "amass_wealth": {
    "id": "amass_wealth",
    "name": "Amass Wealth",
    "phase": "Status",
    "points": 1,
    "description": "Spend 3 influence, 3 resources, and 3 trade goods."
  },
  "build_defenses": {
    "id": "build_defenses",
    "name": "Build Defenses",
    "phase": "Status",
    "points": 1,
    "description": "Have 4 or more structures."
  },
  "lost_outposts": {
    "id": "lost_outposts",
    "name": "Discover Lost Outposts",
    "phase": "Status",
    "points": 1,
    "description": "Control 2 planets that have attachments."
  },
  "discover_lost_outposts": {
    "id": "lost_outposts",
    "name": "Discover Lost Outposts",
    "phase": "Status",
    "points": 1,
    "description": "Control 2 planets that have attachments."
  },
  "engineer_marvel": {
    "id": "engineer_marvel",
    "name": "Engineer a Marvel",
    "phase": "Status",
    "points": 1,
    "description": "Have your flagship or a war sun on the game board."
  },
  "engineer_a_marvel": {
    "id": "engineer_marvel",
    "name": "Engineer a Marvel",
    "phase": "Status",
    "points": 1,
    "description": "Have your flagship or a war sun on the game board."
  },
  "deep_space": {
    "id": "deep_space",
    "name": "Explore Deep Space",
    "phase": "Status",
    "points": 1,
    "description": "Have units in 3 systems that do not contain planets."
  },
  "explore_deep_space": {
    "id": "deep_space",
    "name": "Explore Deep Space",
    "phase": "Status",
    "points": 1,
    "description": "Have units in 3 systems that do not contain planets."
  },
  "infrastructure": {
    "id": "infrastructure",
    "name": "Improve Infrastructure",
    "phase": "Status",
    "points": 1,
    "description": "Have structures on 3 planets outside of your home system."
  },
  "improve_infrastructure": {
    "id": "infrastructure",
    "name": "Improve Infrastructure",
    "phase": "Status",
    "points": 1,
    "description": "Have structures on 3 planets outside of your home system."
  },
  "make_history": {
    "id": "make_history",
    "name": "Make History",
    "phase": "Status",
    "points": 1,
    "description": "Have units in 2 systems that contain legendary planets, Mecatol Rex, or anomalies."
  },
  "outer_rim": {
    "id": "outer_rim",
    "name": "Populate the Outer Rim",
    "phase": "Status",
    "points": 1,
    "description": "Have units in 3 systems on the edge of the game board other than your home system."
  },
  "populate_the_outer_rim": {
    "id": "outer_rim",
    "name": "Populate the Outer Rim",
    "phase": "Status",
    "points": 1,
    "description": "Have units in 3 systems on the edge of the game board other than your home system."
  },
  "push_boundaries": {
    "id": "push_boundaries",
    "name": "Push Boundaries",
    "phase": "Status",
    "points": 1,
    "description": "Control more planets than each of 2 of your neighbors."
  },
  "raise_fleet": {
    "id": "raise_fleet",
    "name": "Raise a Fleet",
    "phase": "Status",
    "points": 1,
    "description": "Have 5 or more non-fighter ships in 1 system."
  },
  "raise_a_fleet": {
    "id": "raise_fleet",
    "name": "Raise a Fleet",
    "phase": "Status",
    "points": 1,
    "description": "Have 5 or more non-fighter ships in 1 system."
  },
  "centralize_trade": {
    "id": "centralize_trade",
    "name": "Centralize Galactic Trade",
    "phase": "Status",
    "points": 2,
    "description": "Spend 10 trade goods."
  },
  "centralize_galactic_trade": {
    "id": "centralize_trade",
    "name": "Centralize Galactic Trade",
    "phase": "Status",
    "points": 2,
    "description": "Spend 10 trade goods."
  },
  "conquer": {
    "id": "conquer",
    "name": "Conquer the Weak",
    "phase": "Status",
    "points": 2,
    "description": "Control 1 planet that is in another player's home system."
  },
  "conquer_the_weak": {
    "id": "conquer",
    "name": "Conquer the Weak",
    "phase": "Status",
    "points": 2,
    "description": "Control 1 planet that is in another player's home system."
  },
  "brain_trust": {
    "id": "brain_trust",
    "name": "Form Galactic Brain Trust",
    "phase": "Status",
    "points": 2,
    "description": "Control 5 planets that have technology specialties."
  },
  "form_galactic_brain_trust": {
    "id": "brain_trust",
    "name": "Form Galactic Brain Trust",
    "phase": "Status",
    "points": 2,
    "description": "Control 5 planets that have technology specialties."
  },
  "golden_age": {
    "id": "golden_age",
    "name": "Found a Golden Age",
    "phase": "Status",
    "points": 2,
    "description": "Spend 16 resources."
  },
  "found_a_golden_age": {
    "id": "golden_age",
    "name": "Found a Golden Age",
    "phase": "Status",
    "points": 2,
    "description": "Spend 16 resources."
  },
  "galvanize": {
    "id": "galvanize",
    "name": "Galvanize the People",
    "phase": "Status",
    "points": 2,
    "description": "Spend a total of 6 tokens from your tactic and/or strategy pools."
  },
  "galvanize_the_people": {
    "id": "galvanize",
    "name": "Galvanize the People",
    "phase": "Status",
    "points": 2,
    "description": "Spend a total of 6 tokens from your tactic and/or strategy pools."
  },
  "manipulate_law": {
    "id": "manipulate_law",
    "name": "Manipulate Galactic Law",
    "phase": "Status",
    "points": 2,
    "description": "Spend 16 influence."
  },
  "manipulate_galactic_law": {
    "id": "manipulate_law",
    "name": "Manipulate Galactic Law",
    "phase": "Status",
    "points": 2,
    "description": "Spend 16 influence."
  },
  "master_science": {
    "id": "master_science",
    "name": "Master the Sciences",
    "phase": "Status",
    "points": 2,
    "description": "Own 2 technologies in each of 4 colors."
  },
  "master_the_sciences": {
    "id": "master_science",
    "name": "Master the Sciences",
    "phase": "Status",
    "points": 2,
    "description": "Own 2 technologies in each of 4 colors."
  },
  "revolutionize": {
    "id": "revolutionize",
    "name": "Revolutionize Warfare",
    "phase": "Status",
    "points": 2,
    "description": "Own 3 unit upgrade technologies."
  },
  "revolutionize_warfare": {
    "id": "revolutionize",
    "name": "Revolutionize Warfare",
    "phase": "Status",
    "points": 2,
    "description": "Own 3 unit upgrade technologies."
  },
  "subdue": {
    "id": "subdue",
    "name": "Subdue the Galaxy",
    "phase": "Status",
    "points": 2,
    "description": "Control 11 planets in non-home systems."
  },
  "subdue_the_galaxy": {
    "id": "subdue",
    "name": "Subdue the Galaxy",
    "phase": "Status",
    "points": 2,
    "description": "Control 11 planets in non-home systems."
  },
  "unify_colonies": {
    "id": "unify_colonies",
    "name": "Unify the Colonies",
    "phase": "Status",
    "points": 2,
    "description": "Control 6 planets that each have the same planet trait."
  },
  "unify_the_colonies": {
    "id": "unify_colonies",
    "name": "Unify the Colonies",
    "phase": "Status",
    "points": 2,
    "description": "Control 6 planets that each have the same planet trait."
  },
  "supremacy": {
    "id": "supremacy",
    "name": "Achieve Supremacy",
    "phase": "Status",
    "points": 2,
    "description": "Have your flagship or war sun in another player's home system or the Mecatol Rex system."
  },
  "achieve_supremacy": {
    "id": "supremacy",
    "name": "Achieve Supremacy",
    "phase": "Status",
    "points": 2,
    "description": "Have your flagship or war sun in another player's home system or the Mecatol Rex system."
  },
  "become_legend": {
    "id": "become_legend",
    "name": "Become a Legend",
    "phase": "Status",
    "points": 2,
    "description": "Have units in 4 systems that contain legendary planets, Mecatol Rex, or anomalies."
  },
  "become_a_legend": {
    "id": "become_legend",
    "name": "Become a Legend",
    "phase": "Status",
    "points": 2,
    "description": "Have units in 4 systems that contain legendary planets, Mecatol Rex, or anomalies."
  },
  "command_armada": {
    "id": "command_armada",
    "name": "Command an Armada",
    "phase": "Status",
    "points": 2,
    "description": "Have 8 or more non-fighter ships in 1 system."
  },
  "command_an_armada": {
    "id": "command_armada",
    "name": "Command an Armada",
    "phase": "Status",
    "points": 2,
    "description": "Have 8 or more non-fighter ships in 1 system."
  },
  "massive_cities": {
    "id": "massive_cities",
    "name": "Construct Massive Cities",
    "phase": "Status",
    "points": 2,
    "description": "Have 7 or more structures."
  },
  "construct_massive_cities": {
    "id": "massive_cities",
    "name": "Construct Massive Cities",
    "phase": "Status",
    "points": 2,
    "description": "Have 7 or more structures."
  },
  "control_borderlands": {
    "id": "control_borderlands",
    "name": "Control the Borderlands",
    "phase": "Status",
    "points": 2,
    "description": "Have units in 5 systems on the edge of the game board other than your home system."
  },
  "control_the_borderlands": {
    "id": "control_borderlands",
    "name": "Control the Borderlands",
    "phase": "Status",
    "points": 2,
    "description": "Have units in 5 systems on the edge of the game board other than your home system."
  },
  "vast_reserves": {
    "id": "vast_reserves",
    "name": "Hold Vast Reserves",
    "phase": "Status",
    "points": 2,
    "description": "Spend 6 influence, 6 resources, and 6 trade goods."
  },
  "hold_vast_reserves": {
    "id": "vast_reserves",
    "name": "Hold Vast Reserves",
    "phase": "Status",
    "points": 2,
    "description": "Spend 6 influence, 6 resources, and 6 trade goods."
  },
  "vast_territories": {
    "id": "vast_territories",
    "name": "Patrol Vast Territories",
    "phase": "Status",
    "points": 2,
    "description": "Have units in 5 systems that do not contain planets."
  },
  "patrol_vast_territories": {
    "id": "vast_territories",
    "name": "Patrol Vast Territories",
    "phase": "Status",
    "points": 2,
    "description": "Have units in 5 systems that do not contain planets."
  },
  "protect_border": {
    "id": "protect_border",
    "name": "Protect the Border",
    "phase": "Status",
    "points": 2,
    "description": "Have structures on 5 planets outside of your home system."
  },
  "protect_the_border": {
    "id": "protect_border",
    "name": "Protect the Border",
    "phase": "Status",
    "points": 2,
    "description": "Have structures on 5 planets outside of your home system."
  },
  "ancient_monuments": {
    "id": "ancient_monuments",
    "name": "Reclaim Ancient Monuments",
    "phase": "Status",
    "points": 2,
    "description": "Control 3 planets that have attachments."
  },
  "reclaim_ancient_monuments": {
    "id": "ancient_monuments",
    "name": "Reclaim Ancient Monuments",
    "phase": "Status",
    "points": 2,
    "description": "Control 3 planets that have attachments."
  },
  "distant_lands": {
    "id": "distant_lands",
    "name": "Rule Distant Lands",
    "phase": "Status",
    "points": 2,
    "description": "Control 2 planets that are each in or adjacent to a different, other player's home system."
  },
  "rule_distant_lands": {
    "id": "distant_lands",
    "name": "Rule Distant Lands",
    "phase": "Status",
    "points": 2,
    "description": "Control 2 planets that are each in or adjacent to a different, other player's home system."
  }
};

export const ACTION_CARDS: Record<string, CardMeta> = {
  "abs": {
    "id": "abs",
    "name": "Ancient Burial Sites",
    "phase": "Agenda",
    "description": "Choose 1 player. Exhaust each cultural planet owned by that player."
  },
  "ancient_burial_sites": {
    "id": "abs",
    "name": "Ancient Burial Sites",
    "phase": "Agenda",
    "description": "Choose 1 player. Exhaust each cultural planet owned by that player."
  },
  "assassin": {
    "id": "assassin",
    "name": "Assassinate Representative",
    "phase": "Agenda",
    "description": "Choose 1 player. That player cannot vote on this agenda."
  },
  "assassinate_representative": {
    "id": "assassin",
    "name": "Assassinate Representative",
    "phase": "Agenda",
    "description": "Choose 1 player. That player cannot vote on this agenda."
  },
  "bribery": {
    "id": "bribery",
    "name": "Bribery",
    "phase": "Agenda",
    "description": "Spend any number of trade goods. For each trade good spent, cast 1 additional vote for the outcome on which you voted."
  },
  "bunker": {
    "id": "bunker",
    "name": "Bunker",
    "phase": "Action",
    "description": "During this invasion, apply -4 to the result of each BOMBARDMENT roll against planets you control."
  },
  "confusing": {
    "id": "confusing",
    "name": "Confusing Legal Text",
    "phase": "Agenda",
    "description": "Choose 1 player. That player is the elected player instead."
  },
  "confusing_legal_text": {
    "id": "confusing",
    "name": "Confusing Legal Text",
    "phase": "Agenda",
    "description": "Choose 1 player. That player is the elected player instead."
  },
  "const_rider": {
    "id": "const_rider",
    "name": "Construction Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, place 1 space dock from your reinforcements on a planet you control."
  },
  "construction_rider": {
    "id": "const_rider",
    "name": "Construction Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, place 1 space dock from your reinforcements on a planet you control."
  },
  "courageous": {
    "id": "courageous",
    "name": "Courageous to the End",
    "phase": "Action",
    "description": "Roll 2 dice. For each result equal to or greater than that ship's combat value, your opponent must choose and destroy 1 of their ships."
  },
  "courageous_to_the_end": {
    "id": "courageous",
    "name": "Courageous to the End",
    "phase": "Action",
    "description": "Roll 2 dice. For each result equal to or greater than that ship's combat value, your opponent must choose and destroy 1 of their ships."
  },
  "cripple": {
    "id": "cripple",
    "name": "Cripple Defenses",
    "phase": "Action",
    "description": "Choose 1 planet. Destroy each PDS on that planet."
  },
  "cripple_defenses": {
    "id": "cripple",
    "name": "Cripple Defenses",
    "phase": "Action",
    "description": "Choose 1 planet. Destroy each PDS on that planet."
  },
  "diplo_rider": {
    "id": "diplo_rider",
    "name": "Diplomacy Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, choose 1 system that contains a planet you control. Each other player places a command token from their reinforcements in that system."
  },
  "diplomacy_rider": {
    "id": "diplo_rider",
    "name": "Diplomacy Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, choose 1 system that contains a planet you control. Each other player places a command token from their reinforcements in that system."
  },
  "dh1": {
    "id": "dh1",
    "name": "Direct Hit",
    "phase": "Action",
    "description": "Destroy that ship."
  },
  "direct_hit": {
    "id": "dh4",
    "name": "Direct Hit",
    "phase": "Action",
    "description": "Destroy that ship."
  },
  "dh2": {
    "id": "dh2",
    "name": "Direct Hit",
    "phase": "Action",
    "description": "Destroy that ship."
  },
  "dh3": {
    "id": "dh3",
    "name": "Direct Hit",
    "phase": "Action",
    "description": "Destroy that ship."
  },
  "dh4": {
    "id": "dh4",
    "name": "Direct Hit",
    "phase": "Action",
    "description": "Destroy that ship."
  },
  "disable": {
    "id": "disable",
    "name": "Disable",
    "phase": "Action",
    "description": "Your opponents' PDS units lose PLANETARY SHIELD and SPACE CANNON during this invasion."
  },
  "distinguished": {
    "id": "distinguished",
    "name": "Distinguished Councilor",
    "phase": "Agenda",
    "description": "Cast 5 additional votes for that outcome."
  },
  "distinguished_councilor": {
    "id": "distinguished",
    "name": "Distinguished Councilor",
    "phase": "Agenda",
    "description": "Cast 5 additional votes for that outcome."
  },
  "economic_initiative": {
    "id": "economic_initiative",
    "name": "Economic Initiative",
    "phase": "Action",
    "description": "Ready each cultural planet you control."
  },
  "emergency": {
    "id": "emergency",
    "name": "Emergency Repairs",
    "phase": "Action",
    "description": "Repair all of your units that have SUSTAIN DAMAGE in the active system."
  },
  "emergency_repairs": {
    "id": "emergency",
    "name": "Emergency Repairs",
    "phase": "Action",
    "description": "Repair all of your units that have SUSTAIN DAMAGE in the active system."
  },
  "experimental": {
    "id": "experimental",
    "name": "Experimental Battlestation",
    "phase": "Action",
    "description": "Choose 1 of your space docks that is either in or adjacent to that system. That space dock uses SPACE CANNON 5(x3) against the active player's ships in the active system."
  },
  "experimental_battlestation": {
    "id": "experimental",
    "name": "Experimental Battlestation",
    "phase": "Action",
    "description": "Choose 1 of your space docks that is either in or adjacent to that system. That space dock uses SPACE CANNON 5(x3) against the active player's ships in the active system."
  },
  "f_prototype": {
    "id": "f_prototype",
    "name": "Fighter Prototype",
    "phase": "Action",
    "description": "Apply +2 to the result of each of your fighters' combat rolls during this combat round."
  },
  "fighter_prototype": {
    "id": "f_prototype",
    "name": "Fighter Prototype",
    "phase": "Action",
    "description": "Apply +2 to the result of each of your fighters' combat rolls during this combat round."
  },
  "fire_team": {
    "id": "fire_team",
    "name": "Fire Team",
    "phase": "Action",
    "description": "Reroll any number of your dice."
  },
  "fs1": {
    "id": "fs1",
    "name": "Flank Speed",
    "phase": "Action",
    "description": "Apply +1 to the move value of each of your ships during this tactical action."
  },
  "flank_speed": {
    "id": "fs4",
    "name": "Flank Speed",
    "phase": "Action",
    "description": "Apply +1 to the move value of each of your ships during this tactical action."
  },
  "fs2": {
    "id": "fs2",
    "name": "Flank Speed",
    "phase": "Action",
    "description": "Apply +1 to the move value of each of your ships during this tactical action."
  },
  "fs3": {
    "id": "fs3",
    "name": "Flank Speed",
    "phase": "Action",
    "description": "Apply +1 to the move value of each of your ships during this tactical action."
  },
  "fs4": {
    "id": "fs4",
    "name": "Flank Speed",
    "phase": "Action",
    "description": "Apply +1 to the move value of each of your ships during this tactical action."
  },
  "f_researched": {
    "id": "f_researched",
    "name": "Focused Research",
    "phase": "Action",
    "description": "Spend 4 trade goods to research 1 technology"
  },
  "focused_research": {
    "id": "f_researched",
    "name": "Focused Research",
    "phase": "Action",
    "description": "Spend 4 trade goods to research 1 technology"
  },
  "f_deployment": {
    "id": "f_deployment",
    "name": "Frontline Deployment",
    "phase": "Action",
    "description": "Place 3 infantry from your reinforcements on 1 planet you control."
  },
  "frontline_deployment": {
    "id": "f_deployment",
    "name": "Frontline Deployment",
    "phase": "Action",
    "description": "Place 3 infantry from your reinforcements on 1 planet you control."
  },
  "ghost_ship": {
    "id": "ghost_ship",
    "name": "Ghost Ship",
    "phase": "Action",
    "description": "Place 1 destroyer from your reinforcements in a non-home system that contains a wormhole and does not contain other players' ships."
  },
  "imp_rider": {
    "id": "imp_rider",
    "name": "Imperial Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, gain 1 victory point."
  },
  "imperial_rider": {
    "id": "imp_rider",
    "name": "Imperial Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, gain 1 victory point."
  },
  "silence_space": {
    "id": "silence_space",
    "name": "In The Silence Of Space",
    "phase": "Action",
    "description": "Choose 1 system. During this tactical action, your ships in the chosen system can move through systems that contain other players' ships."
  },
  "in_the_silence_of_space": {
    "id": "silence_space",
    "name": "In The Silence Of Space",
    "phase": "Action",
    "description": "Choose 1 system. During this tactical action, your ships in the chosen system can move through systems that contain other players' ships."
  },
  "industrial_initiative": {
    "id": "industrial_initiative",
    "name": "Industrial Initiative",
    "phase": "Action",
    "description": "Gain 1 trade good for each industrial planet you control."
  },
  "infiltrate": {
    "id": "infiltrate",
    "name": "Infiltrate",
    "phase": "Action",
    "description": "Replace each PDS and space dock that is on that planet with a matching unit from your reinforcements."
  },
  "insub": {
    "id": "insub",
    "name": "Insubordination",
    "phase": "Action",
    "description": "Remove 1 token from another player's tactic pool and return it to their reinforcements."
  },
  "insubordination": {
    "id": "insub",
    "name": "Insubordination",
    "phase": "Action",
    "description": "Remove 1 token from another player's tactic pool and return it to their reinforcements."
  },
  "intercept": {
    "id": "intercept",
    "name": "Intercept",
    "phase": "Action",
    "description": "Your opponent cannot retreat during this round of space combat."
  },
  "lead_rider": {
    "id": "lead_rider",
    "name": "Leadership Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, gain 3 command tokens."
  },
  "leadership_rider": {
    "id": "lead_rider",
    "name": "Leadership Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, gain 3 command tokens."
  },
  "lost_star": {
    "id": "lost_star",
    "name": "Lost Star Chart",
    "phase": "Action",
    "description": "During this tactical action, systems that contain alpha and beta wormholes are adjacent to each other."
  },
  "lost_star_chart": {
    "id": "lost_star",
    "name": "Lost Star Chart",
    "phase": "Action",
    "description": "During this tactical action, systems that contain alpha and beta wormholes are adjacent to each other."
  },
  "lucky": {
    "id": "lucky",
    "name": "Lucky Shot",
    "phase": "Action",
    "description": "Destroy 1 dreadnought, cruiser, or destroyer in a system that contains a planet you control."
  },
  "lucky_shot": {
    "id": "lucky",
    "name": "Lucky Shot",
    "phase": "Action",
    "description": "Destroy 1 dreadnought, cruiser, or destroyer in a system that contains a planet you control."
  },
  "mjets1": {
    "id": "mjets1",
    "name": "Maneuvering Jets",
    "phase": "Action",
    "description": "Cancel 1 hit."
  },
  "maneuvering_jets": {
    "id": "mjets4",
    "name": "Maneuvering Jets",
    "phase": "Action",
    "description": "Cancel 1 hit."
  },
  "mjets2": {
    "id": "mjets2",
    "name": "Maneuvering Jets",
    "phase": "Action",
    "description": "Cancel 1 hit."
  },
  "mjets3": {
    "id": "mjets3",
    "name": "Maneuvering Jets",
    "phase": "Action",
    "description": "Cancel 1 hit."
  },
  "mjets4": {
    "id": "mjets4",
    "name": "Maneuvering Jets",
    "phase": "Action",
    "description": "Cancel 1 hit."
  },
  "mining_initiative": {
    "id": "mining_initiative",
    "name": "Mining Initiative",
    "phase": "Action",
    "description": "Gain trade goods equal to the resource value of 1 planet you control."
  },
  "mb1": {
    "id": "mb1",
    "name": "Morale Boost",
    "phase": "Action",
    "description": "Apply +1 to the result of each of your unit's combat rolls during this combat round."
  },
  "morale_boost": {
    "id": "mb4",
    "name": "Morale Boost",
    "phase": "Action",
    "description": "Apply +1 to the result of each of your unit's combat rolls during this combat round."
  },
  "mb2": {
    "id": "mb2",
    "name": "Morale Boost",
    "phase": "Action",
    "description": "Apply +1 to the result of each of your unit's combat rolls during this combat round."
  },
  "mb3": {
    "id": "mb3",
    "name": "Morale Boost",
    "phase": "Action",
    "description": "Apply +1 to the result of each of your unit's combat rolls during this combat round."
  },
  "mb4": {
    "id": "mb4",
    "name": "Morale Boost",
    "phase": "Action",
    "description": "Apply +1 to the result of each of your unit's combat rolls during this combat round."
  },
  "parley": {
    "id": "parley",
    "name": "Parley",
    "phase": "Action",
    "description": "Return the committed units to the space area."
  },
  "plague": {
    "id": "plague",
    "name": "Plague",
    "phase": "Action",
    "description": "Choose 1 planet that is controlled by another player. Roll 1 die for each infantry on that planet. For each result of 6 or greater, destroy 1 of those units."
  },
  "stability": {
    "id": "stability",
    "name": "Political Stability",
    "phase": "Status",
    "description": "Do not return your strategy card(s). You do not choose strategy cards during the next strategy phase."
  },
  "political_stability": {
    "id": "stability",
    "name": "Political Stability",
    "phase": "Status",
    "description": "Do not return your strategy card(s). You do not choose strategy cards during the next strategy phase."
  },
  "politic_rider": {
    "id": "politic_rider",
    "name": "Politics Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, draw 3 action cards and gain the speaker token."
  },
  "politics_rider": {
    "id": "politic_rider",
    "name": "Politics Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, draw 3 action cards and gain the speaker token."
  },
  "disgrace": {
    "id": "disgrace",
    "name": "Public Disgrace",
    "phase": "Strategy",
    "description": "That player must choose a different strategy card instead, if able."
  },
  "public_disgrace": {
    "id": "disgrace",
    "name": "Public Disgrace",
    "phase": "Strategy",
    "description": "That player must choose a different strategy card instead, if able."
  },
  "meltdown": {
    "id": "meltdown",
    "name": "Reactor Meltdown",
    "phase": "Action",
    "description": "Destroy 1 space dock in a non-home system."
  },
  "reactor_meltdown": {
    "id": "meltdown",
    "name": "Reactor Meltdown",
    "phase": "Action",
    "description": "Destroy 1 space dock in a non-home system."
  },
  "reparations": {
    "id": "reparations",
    "name": "Reparations",
    "phase": "Action",
    "description": "Exhaust 1 planet that player controls and ready 1 planet you control."
  },
  "repeal": {
    "id": "repeal",
    "name": "Repeal Law",
    "phase": "Action",
    "description": "Discard 1 law from play."
  },
  "repeal_law": {
    "id": "repeal",
    "name": "Repeal Law",
    "phase": "Action",
    "description": "Discard 1 law from play."
  },
  "messiah": {
    "id": "messiah",
    "name": "Rise of a Messiah",
    "phase": "Action",
    "description": "Place 1 infantry from your reinforcements on each planet you control."
  },
  "rise_of_a_messiah": {
    "id": "messiah",
    "name": "Rise of a Messiah",
    "phase": "Action",
    "description": "Place 1 infantry from your reinforcements on each planet you control."
  },
  "sabo1": {
    "id": "sabo1",
    "name": "Sabotage",
    "phase": "Any",
    "description": "Cancel that action card."
  },
  "sabotage": {
    "id": "sabo4",
    "name": "Sabotage",
    "phase": "Any",
    "description": "Cancel that action card."
  },
  "sabo2": {
    "id": "sabo2",
    "name": "Sabotage",
    "phase": "Any",
    "description": "Cancel that action card."
  },
  "sabo3": {
    "id": "sabo3",
    "name": "Sabotage",
    "phase": "Any",
    "description": "Cancel that action card."
  },
  "sabo4": {
    "id": "sabo4",
    "name": "Sabotage",
    "phase": "Any",
    "description": "Cancel that action card."
  },
  "salvage": {
    "id": "salvage",
    "name": "Salvage",
    "phase": "Action",
    "description": "Your opponent gives you all of their commodities."
  },
  "sh1": {
    "id": "sh1",
    "name": "Shields Holding",
    "phase": "Action",
    "description": "Cancel up to 2 hits."
  },
  "shields_holding": {
    "id": "sh4",
    "name": "Shields Holding",
    "phase": "Action",
    "description": "Cancel up to 2 hits."
  },
  "sh2": {
    "id": "sh2",
    "name": "Shields Holding",
    "phase": "Action",
    "description": "Cancel up to 2 hits."
  },
  "sh3": {
    "id": "sh3",
    "name": "Shields Holding",
    "phase": "Action",
    "description": "Cancel up to 2 hits."
  },
  "sh4": {
    "id": "sh4",
    "name": "Shields Holding",
    "phase": "Action",
    "description": "Cancel up to 2 hits."
  },
  "jamming": {
    "id": "jamming",
    "name": "Signal Jamming",
    "phase": "Action",
    "description": "Choose 1 non-home system that contains or is adjacent to 1 of your ships. Place a command token from another player's reinforcements in that system."
  },
  "signal_jamming": {
    "id": "jamming",
    "name": "Signal Jamming",
    "phase": "Action",
    "description": "Choose 1 non-home system that contains or is adjacent to 1 of your ships. Place a command token from another player's reinforcements in that system."
  },
  "s_retreat1": {
    "id": "s_retreat1",
    "name": "Skilled Retreat",
    "phase": "Action",
    "description": "Move all of your ships from the active system into an adjacent system that does not contain another player's ships. The space combat ends in a draw. Then, place a command token from your reinforcements in that system."
  },
  "skilled_retreat": {
    "id": "s_retreat4",
    "name": "Skilled Retreat",
    "phase": "Action",
    "description": "Move all of your ships from the active system into an adjacent system that does not contain another player's ships. The space combat ends in a draw. Then, place a command token from your reinforcements in that system."
  },
  "s_retreat2": {
    "id": "s_retreat2",
    "name": "Skilled Retreat",
    "phase": "Action",
    "description": "Move all of your ships from the active system into an adjacent system that does not contain another player's ships. The space combat ends in a draw. Then, place a command token from your reinforcements in that system."
  },
  "s_retreat3": {
    "id": "s_retreat3",
    "name": "Skilled Retreat",
    "phase": "Action",
    "description": "Move all of your ships from the active system into an adjacent system that does not contain another player's ships. The space combat ends in a draw. Then, place a command token from your reinforcements in that system."
  },
  "s_retreat4": {
    "id": "s_retreat4",
    "name": "Skilled Retreat",
    "phase": "Action",
    "description": "Move all of your ships from the active system into an adjacent system that does not contain another player's ships. The space combat ends in a draw. Then, place a command token from your reinforcements in that system."
  },
  "spy": {
    "id": "spy",
    "name": "Spy",
    "phase": "Action",
    "description": "Choose 1 player. That player gives you 1 random action card from their hand."
  },
  "summit": {
    "id": "summit",
    "name": "Summit",
    "phase": "Strategy",
    "description": "Gain 2 command tokens."
  },
  "tactical": {
    "id": "tactical",
    "name": "Tactical Bombardment",
    "phase": "Action",
    "description": "Choose 1 system that contains 1 or more of your units that have BOMBARDMENT. Exhaust each planet controlled by other players in that system."
  },
  "tactical_bombardment": {
    "id": "tactical",
    "name": "Tactical Bombardment",
    "phase": "Action",
    "description": "Choose 1 system that contains 1 or more of your units that have BOMBARDMENT. Exhaust each planet controlled by other players in that system."
  },
  "tech_rider": {
    "id": "tech_rider",
    "name": "Technology Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, research 1 technology."
  },
  "technology_rider": {
    "id": "tech_rider",
    "name": "Technology Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, research 1 technology."
  },
  "trade_rider": {
    "id": "trade_rider",
    "name": "Trade Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, gain 5 trade goods."
  },
  "unexpected": {
    "id": "unexpected",
    "name": "Unexpected Action",
    "phase": "Action",
    "description": "Remove 1 of your command tokens from the game board and return it to your reinforcements."
  },
  "unexpected_action": {
    "id": "unexpected",
    "name": "Unexpected Action",
    "phase": "Action",
    "description": "Remove 1 of your command tokens from the game board and return it to your reinforcements."
  },
  "unstable": {
    "id": "unstable",
    "name": "Unstable Planet",
    "phase": "Action",
    "description": "Choose 1 hazardous planet. Exhaust that planet and destroy up to 3 infantry on it."
  },
  "unstable_planet": {
    "id": "unstable",
    "name": "Unstable Planet",
    "phase": "Action",
    "description": "Choose 1 hazardous planet. Exhaust that planet and destroy up to 3 infantry on it."
  },
  "upgrade": {
    "id": "upgrade",
    "name": "Upgrade",
    "phase": "Action",
    "description": "Replace 1 of your cruisers in that system with 1 dreadnought from your reinforcements."
  },
  "uprising": {
    "id": "uprising",
    "name": "Uprising",
    "phase": "Action",
    "description": "Exhaust 1 non-home planet controlled by another player. Then gain trade goods equal to its resource value."
  },
  "veto": {
    "id": "veto4",
    "name": "Veto",
    "phase": "Agenda",
    "description": "Discard that agenda and reveal 1 agenda from the top of the deck. Players vote on this agenda instead."
  },
  "veto3": {
    "id": "veto3",
    "name": "Veto",
    "phase": "Agenda",
    "description": "Discard that agenda and reveal 1 agenda from the top of the deck. Players vote on this agenda instead."
  },
  "veto4": {
    "id": "veto4",
    "name": "Veto",
    "phase": "Agenda",
    "description": "Discard that agenda and reveal 1 agenda from the top of the deck. Players vote on this agenda instead."
  },
  "war_effort": {
    "id": "war_effort",
    "name": "War Effort",
    "phase": "Action",
    "description": "Place 1 cruiser from your reinforcements in a system that contains 1 or more of your ships."
  },
  "war_rider": {
    "id": "war_rider",
    "name": "Warfare Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, place 1 dreadnought from your reinforcements in a system that contains 1 or more of your ships."
  },
  "warfare_rider": {
    "id": "war_rider",
    "name": "Warfare Rider",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, place 1 dreadnought from your reinforcements in a system that contains 1 or more of your ships."
  },
  "blitz": {
    "id": "blitz",
    "name": "Blitz",
    "phase": "Action",
    "description": "Each of your non-fighter ships in the active system that do not have BOMBARDMENT gain BOMBARDMENT 6 until the end of the invasion."
  },
  "counterstroke": {
    "id": "counterstroke",
    "name": "Counterstroke",
    "phase": "Action",
    "description": "Return that command token to your tactic pool."
  },
  "f_conscription": {
    "id": "f_conscription",
    "name": "Fighter Conscription",
    "phase": "Action",
    "description": "Place 1 fighter from your reinforcements in each system that contains 1 or more of your space docks or units that have capacity. They cannot be placed in systems that contain other players' ships."
  },
  "fighter_conscription": {
    "id": "f_conscription",
    "name": "Fighter Conscription",
    "phase": "Action",
    "description": "Place 1 fighter from your reinforcements in each system that contains 1 or more of your space docks or units that have capacity. They cannot be placed in systems that contain other players' ships."
  },
  "fsb": {
    "id": "fsb",
    "name": "Forward Supply Base",
    "phase": "Action",
    "description": "Gain 3 trade goods. Then, choose another player to gain 1 trade good."
  },
  "forward_supply_base": {
    "id": "fsb",
    "name": "Forward Supply Base",
    "phase": "Action",
    "description": "Gain 3 trade goods. Then, choose another player to gain 1 trade good."
  },
  "ghost_squad": {
    "id": "ghost_squad",
    "name": "Ghost Squad",
    "phase": "Action",
    "description": "Move any number of your ground forces from any planet you control in the active system to any other planet you control in the active system."
  },
  "hack": {
    "id": "hack",
    "name": "Hack Election",
    "phase": "Agenda",
    "description": "During this agenda, you vote last."
  },
  "hack_election": {
    "id": "hack",
    "name": "Hack Election",
    "phase": "Agenda",
    "description": "During this agenda, you vote last."
  },
  "harness": {
    "id": "harness",
    "name": "Harness Energy",
    "phase": "Action",
    "description": "Replenish your commodities."
  },
  "harness_energy": {
    "id": "harness",
    "name": "Harness Energy",
    "phase": "Action",
    "description": "Replenish your commodities."
  },
  "impersonation": {
    "id": "impersonation",
    "name": "Impersonation",
    "phase": "Action",
    "description": "Spend 3 influence to draw 1 secret objective."
  },
  "insider": {
    "id": "insider",
    "name": "Insider Information",
    "phase": "Agenda",
    "description": "Look at the top 3 cards of the agenda deck."
  },
  "insider_information": {
    "id": "insider",
    "name": "Insider Information",
    "phase": "Agenda",
    "description": "Look at the top 3 cards of the agenda deck."
  },
  "master_plan": {
    "id": "master_plan",
    "name": "Master Plan",
    "phase": "Action",
    "description": "Perform an additional action."
  },
  "plagiarize": {
    "id": "plagiarize",
    "name": "Plagiarize",
    "phase": "Action",
    "description": "Spend 5 influence and choose a non-faction technology owned by 1 of your neighbors. Gain that technology."
  },
  "rally": {
    "id": "rally",
    "name": "Rally",
    "phase": "Action",
    "description": "Place 2 command tokens from your reinforcements in your fleet pool."
  },
  "reflective": {
    "id": "reflective",
    "name": "Reflective Shielding",
    "phase": "Action",
    "description": "Produce 2 hits against your opponent's ships in the active system."
  },
  "reflective_shielding": {
    "id": "reflective",
    "name": "Reflective Shielding",
    "phase": "Action",
    "description": "Produce 2 hits against your opponent's ships in the active system."
  },
  "sanction": {
    "id": "sanction",
    "name": "Sanction",
    "phase": "Agenda",
    "description": "You cannot vote on this agenda. Predict aloud an outcome of this agenda. If your prediction is correct, each player that voted for that outcome returns 1 command token from their fleet supply to their reinforcements."
  },
  "scramble": {
    "id": "scramble",
    "name": "Scramble Frequency",
    "phase": "Action",
    "description": "That player rerolls all of their dice."
  },
  "scramble_frequency": {
    "id": "scramble",
    "name": "Scramble Frequency",
    "phase": "Action",
    "description": "That player rerolls all of their dice."
  },
  "solar_flare": {
    "id": "solar_flare",
    "name": "Solar Flare",
    "phase": "Action",
    "description": "During the \"Movement\" step of this tactical action, other players cannot use SPACE CANNON against your ships."
  },
  "war_machine1": {
    "id": "war_machine1",
    "name": "War Machine",
    "phase": "Action",
    "description": "Apply +4 to the total PRODUCTION value of your units and reduce the combined cost of the produced units by 1."
  },
  "war_machine": {
    "id": "war_machine4",
    "name": "War Machine",
    "phase": "Action",
    "description": "Apply +4 to the total PRODUCTION value of your units and reduce the combined cost of the produced units by 1."
  },
  "war_machine2": {
    "id": "war_machine2",
    "name": "War Machine",
    "phase": "Action",
    "description": "Apply +4 to the total PRODUCTION value of your units and reduce the combined cost of the produced units by 1."
  },
  "war_machine3": {
    "id": "war_machine3",
    "name": "War Machine",
    "phase": "Action",
    "description": "Apply +4 to the total PRODUCTION value of your units and reduce the combined cost of the produced units by 1."
  },
  "war_machine4": {
    "id": "war_machine4",
    "name": "War Machine",
    "phase": "Action",
    "description": "Apply +4 to the total PRODUCTION value of your units and reduce the combined cost of the produced units by 1."
  },
  "arch_expedition": {
    "id": "arch_expedition",
    "name": "Archaeological Expedition",
    "phase": "Action",
    "description": "Reveal the top 3 cards of an exploration deck that matches a planet you control; gain any relic fragments that you reveal and discard the rest."
  },
  "archaeological_expedition": {
    "id": "arch_expedition",
    "name": "Archaeological Expedition",
    "phase": "Action",
    "description": "Reveal the top 3 cards of an exploration deck that matches a planet you control; gain any relic fragments that you reveal and discard the rest."
  },
  "confounding": {
    "id": "confounding",
    "name": "Confounding Legal Text",
    "phase": "Agenda",
    "description": "You are the elected player instead."
  },
  "confounding_legal_text": {
    "id": "confounding",
    "name": "Confounding Legal Text",
    "phase": "Agenda",
    "description": "You are the elected player instead."
  },
  "coup": {
    "id": "coup",
    "name": "Coup d'Etat",
    "phase": "Action",
    "description": "End that player's turn, the strategic action is not resolved and the strategy card is not exhausted."
  },
  "coup_d_etat": {
    "id": "coup",
    "name": "Coup d'Etat",
    "phase": "Action",
    "description": "End that player's turn, the strategic action is not resolved and the strategy card is not exhausted."
  },
  "deadly_plot": {
    "id": "deadly_plot",
    "name": "Deadly Plot",
    "phase": "Agenda",
    "description": "If you voted for or predicted another outcome, discard the agenda instead. The agenda is resolved with no effect and it is not replaced. Then, exhaust all of your planets."
  },
  "decoy": {
    "id": "decoy",
    "name": "Decoy Operation",
    "phase": "Action",
    "description": "Remove up to 2 of your ground forces from the game board and place them on a planet you control in the active system."
  },
  "decoy_operation": {
    "id": "decoy",
    "name": "Decoy Operation",
    "phase": "Action",
    "description": "Remove up to 2 of your ground forces from the game board and place them on a planet you control in the active system."
  },
  "dp1": {
    "id": "dp1",
    "name": "Diplomatic Pressure",
    "phase": "Agenda",
    "description": "Choose another player. That player must give you 1 promissory note from their hand."
  },
  "diplo_pressure": {
    "id": "dp4",
    "name": "Diplomatic Pressure",
    "phase": "Agenda",
    "description": "Choose another player. That player must give you 1 promissory note from their hand."
  },
  "diplomatic_pressure": {
    "id": "dp4",
    "name": "Diplomatic Pressure",
    "phase": "Agenda",
    "description": "Choose another player. That player must give you 1 promissory note from their hand."
  },
  "dp2": {
    "id": "dp2",
    "name": "Diplomatic Pressure",
    "phase": "Agenda",
    "description": "Choose another player. That player must give you 1 promissory note from their hand."
  },
  "dp3": {
    "id": "dp3",
    "name": "Diplomatic Pressure",
    "phase": "Agenda",
    "description": "Choose another player. That player must give you 1 promissory note from their hand."
  },
  "dp4": {
    "id": "dp4",
    "name": "Diplomatic Pressure",
    "phase": "Agenda",
    "description": "Choose another player. That player must give you 1 promissory note from their hand."
  },
  "divert_funding": {
    "id": "divert_funding",
    "name": "Divert Funding",
    "phase": "Action",
    "description": "Return a non-unit upgrade, non-faction technology that you own to your technology deck. Then, research another technology."
  },
  "probe": {
    "id": "probe",
    "name": "Exploration Probe",
    "phase": "Action",
    "description": "Explore a frontier token that is in or adjacent to a system that contains 1 or more of your ships."
  },
  "exploration_probe": {
    "id": "probe",
    "name": "Exploration Probe",
    "phase": "Action",
    "description": "Explore a frontier token that is in or adjacent to a system that contains 1 or more of your ships."
  },
  "investments": {
    "id": "investments",
    "name": "Manipulate Investments",
    "phase": "Strategy",
    "description": "Place a total of 5 trade goods from the supply on strategy cards of your choice. You must place these tokens on at least 3 different cards."
  },
  "manipulate_investments": {
    "id": "investments",
    "name": "Manipulate Investments",
    "phase": "Strategy",
    "description": "Place a total of 5 trade goods from the supply on strategy cards of your choice. You must place these tokens on at least 3 different cards."
  },
  "nav_suite": {
    "id": "nav_suite",
    "name": "Nav Suite",
    "phase": "Action",
    "description": "During the 'Movement' step of this tactical action, ignore the effect of anomalies."
  },
  "refit": {
    "id": "refit",
    "name": "Refit Troops",
    "phase": "Action",
    "description": "Choose 1 or 2 of your infantry on the game board. Replace each of those infantry with mechs."
  },
  "refit_troops": {
    "id": "refit",
    "name": "Refit Troops",
    "phase": "Action",
    "description": "Choose 1 or 2 of your infantry on the game board. Replace each of those infantry with mechs."
  },
  "reveal_prototype": {
    "id": "reveal_prototype",
    "name": "Reveal Prototype",
    "phase": "Action",
    "description": "Spend 4 resources to research a unit upgrade technology of the same type as 1 of your units that is participating in this combat."
  },
  "reverse_engineer": {
    "id": "reverse_engineer",
    "name": "Reverse Engineer",
    "phase": "Any",
    "description": "Take that action card from the discard pile."
  },
  "rout": {
    "id": "rout",
    "name": "Rout",
    "phase": "Action",
    "description": "Your opponent must announce a retreat, if able."
  },
  "scuttle": {
    "id": "scuttle",
    "name": "Scuttle",
    "phase": "Action",
    "description": "Choose 1 or 2 of your non-fighter ships on the game board and return them to your reinforcements. Gain trade goods equal to the combined cost of those ships."
  },
  "seize": {
    "id": "seize",
    "name": "Seize Artifact",
    "phase": "Action",
    "description": "Choose 1 of your neighbors that has 1 or more relic fragments. That player must give you 1 relic fragment of your choice."
  },
  "seize_artifact": {
    "id": "seize",
    "name": "Seize Artifact",
    "phase": "Action",
    "description": "Choose 1 of your neighbors that has 1 or more relic fragments. That player must give you 1 relic fragment of your choice."
  },
  "waylay": {
    "id": "waylay",
    "name": "Waylay",
    "phase": "Action",
    "description": "Hits from this roll are produced against all ships (not just fighters)."
  },
  "lieinwait": {
    "id": "lieinwait",
    "name": "Lie in Wait",
    "phase": "Any",
    "description": "Look at each of those players' hands of action cards, then choose and take 1 action card from each."
  },
  "lie_in_wait": {
    "id": "lieinwait",
    "name": "Lie in Wait",
    "phase": "Any",
    "description": "Look at each of those players' hands of action cards, then choose and take 1 action card from each."
  },
  "exchangeprogram": {
    "id": "exchangeprogram",
    "name": "Exchange Program",
    "phase": "Action",
    "description": "Choose another player. You and that player may agree to place 1 infantry from each of your reinforcements into coexistence on a planet the other player controls that contains their ground forces; if no agreement is reached, you each discard 1 token from your fleet pool."
  },
  "exchange_program": {
    "id": "exchangeprogram",
    "name": "Exchange Program",
    "phase": "Action",
    "description": "Choose another player. You and that player may agree to place 1 infantry from each of your reinforcements into coexistence on a planet the other player controls that contains their ground forces; if no agreement is reached, you each discard 1 token from your fleet pool."
  },
  "puppetsonastring": {
    "id": "puppetsonastring",
    "name": "Puppets on a String",
    "phase": "Action",
    "description": "Perform 1 action."
  },
  "puppets_on_a_string": {
    "id": "puppetsonastring",
    "name": "Puppets on a String",
    "phase": "Action",
    "description": "Perform 1 action."
  },
  "extremeduress": {
    "id": "extremeduress",
    "name": "Extreme Duress",
    "phase": "Action",
    "description": "If that player's next action is not a strategic action, they discard all of their action cards, give you all of their trade goods, and show you all of their secret objectives."
  },
  "extreme_duress": {
    "id": "extremeduress",
    "name": "Extreme Duress",
    "phase": "Action",
    "description": "If that player's next action is not a strategic action, they discard all of their action cards, give you all of their trade goods, and show you all of their secret objectives."
  },
  "mercenarycontract": {
    "id": "mercenarycontract",
    "name": "Mercenary Contract",
    "phase": "Action",
    "description": "Spend 2 trade goods to place 2 neutral infantry on any non-home planet that contains no units; if that planet was owned by another player, they return its planet card to the planet card deck."
  },
  "mercenary_contract": {
    "id": "mercenarycontract",
    "name": "Mercenary Contract",
    "phase": "Action",
    "description": "Spend 2 trade goods to place 2 neutral infantry on any non-home planet that contains no units; if that planet was owned by another player, they return its planet card to the planet card deck."
  },
  "strategize1": {
    "id": "strategize1",
    "name": "Strategize",
    "phase": "Action",
    "description": "Perform the secondary ability of any readied or unchosen strategy card."
  },
  "strategize": {
    "id": "strategize4",
    "name": "Strategize",
    "phase": "Action",
    "description": "Perform the secondary ability of any readied or unchosen strategy card."
  },
  "strategize2": {
    "id": "strategize2",
    "name": "Strategize",
    "phase": "Action",
    "description": "Perform the secondary ability of any readied or unchosen strategy card."
  },
  "strategize3": {
    "id": "strategize3",
    "name": "Strategize",
    "phase": "Action",
    "description": "Perform the secondary ability of any readied or unchosen strategy card."
  },
  "strategize4": {
    "id": "strategize4",
    "name": "Strategize",
    "phase": "Action",
    "description": "Perform the secondary ability of any readied or unchosen strategy card."
  },
  "rescue": {
    "id": "rescue",
    "name": "Rescue",
    "phase": "Action",
    "description": "You may move 1 of your ships into the active system from any system that does not contain one of your command tokens."
  },
  "piratefleet": {
    "id": "piratefleet",
    "name": "Pirate Fleet",
    "phase": "Action",
    "description": "Spend 3 resources to place 1 neutral carrier, 1 neutral cruiser, 1 neutral destroyer, and 2 neutral fighters in a non-home system that contains no non-neutral ships."
  },
  "pirate_fleet": {
    "id": "piratefleet",
    "name": "Pirate Fleet",
    "phase": "Action",
    "description": "Spend 3 resources to place 1 neutral carrier, 1 neutral cruiser, 1 neutral destroyer, and 2 neutral fighters in a non-home system that contains no non-neutral ships."
  },
  "brilliance": {
    "id": "brilliance",
    "name": "Brilliance",
    "phase": "Action",
    "description": "Ready 1 of your planets that has a technology specialty or choose 1 player to gain their breakthrough."
  },
  "blackmarketdealing": {
    "id": "blackmarketdealing",
    "name": "Black Market Dealings",
    "phase": "Any",
    "description": "You and the other player may include relics, action cards, and unscored secret objectives as part of the transaction. This card cannot be canceled."
  },
  "black_market_dealings": {
    "id": "blackmarketdealing",
    "name": "Black Market Dealings",
    "phase": "Any",
    "description": "You and the other player may include relics, action cards, and unscored secret objectives as part of the transaction. This card cannot be canceled."
  },
  "piratecontract1": {
    "id": "piratecontract1",
    "name": "Pirate Contract",
    "phase": "Action",
    "description": "Place 1 neutral destroyer in a non-home system that contains no non-neutral ships."
  },
  "pirate_contract": {
    "id": "piratecontract4",
    "name": "Pirate Contract",
    "phase": "Action",
    "description": "Place 1 neutral destroyer in a non-home system that contains no non-neutral ships."
  },
  "piratecontract2": {
    "id": "piratecontract2",
    "name": "Pirate Contract",
    "phase": "Action",
    "description": "Place 1 neutral destroyer in a non-home system that contains no non-neutral ships."
  },
  "piratecontract3": {
    "id": "piratecontract3",
    "name": "Pirate Contract",
    "phase": "Action",
    "description": "Place 1 neutral destroyer in a non-home system that contains no non-neutral ships."
  },
  "piratecontract4": {
    "id": "piratecontract4",
    "name": "Pirate Contract",
    "phase": "Action",
    "description": "Place 1 neutral destroyer in a non-home system that contains no non-neutral ships."
  },
  "crashlanding": {
    "id": "crashlanding",
    "name": "Crash Landing",
    "phase": "Action",
    "description": "Place 1 of your ground forces from the space area of the active system onto a planet in that system other than Mecatol Rex; if the planet contains other players' units, place your ground force into coexistence."
  },
  "crash_landing": {
    "id": "crashlanding",
    "name": "Crash Landing",
    "phase": "Action",
    "description": "Place 1 of your ground forces from the space area of the active system onto a planet in that system other than Mecatol Rex; if the planet contains other players' units, place your ground force into coexistence."
  },
  "crisis": {
    "id": "crisis",
    "name": "Crisis",
    "phase": "Action",
    "description": "Skip the next player's turn."
  },
  "overrule": {
    "id": "overrule",
    "name": "Overrule",
    "phase": "Action",
    "description": "Perform the primary ability of a readied or unchosen strategy card."
  }
};

export const TECHNOLOGIES: Record<string, CardMeta> = {
  "amd": {
    "id": "amd",
    "name": "Antimass Deflectors",
    "description": "Your ships can move into and through asteroid fields.\nWhen other players' units use SPACE CANNON against your units, apply -1 to the result of each die roll."
  },
  "antimass_deflectors": {
    "id": "amd",
    "name": "Antimass Deflectors",
    "description": "Your ships can move into and through asteroid fields.\nWhen other players' units use SPACE CANNON against your units, apply -1 to the result of each die roll."
  },
  "gd": {
    "id": "gd",
    "name": "Gravity Drive",
    "description": "After you activate a system, apply +1 to the move value of 1 of your ships during this tactical action."
  },
  "gravity_drive": {
    "id": "gd",
    "name": "Gravity Drive",
    "description": "After you activate a system, apply +1 to the move value of 1 of your ships during this tactical action."
  },
  "fl": {
    "id": "fl",
    "name": "Fleet Logistics",
    "description": "During each of your turns of the action phase, you may perform 2 actions instead of 1."
  },
  "fleet_logistics": {
    "id": "fl",
    "name": "Fleet Logistics",
    "description": "During each of your turns of the action phase, you may perform 2 actions instead of 1."
  },
  "lwd": {
    "id": "lwd",
    "name": "Light/Wave Deflector",
    "description": "Your ships can move through systems that contain other players' ships."
  },
  "light_wave_deflector": {
    "id": "lwd",
    "name": "Light/Wave Deflector",
    "description": "Your ships can move through systems that contain other players' ships."
  },
  "det": {
    "id": "det",
    "name": "Dark Energy Tap",
    "description": "After you perform a tactical action in a system that contains a frontier token, if you have 1 or more ships in that system, explore that token.\nYour ships can retreat into adjacent systems that do not contain other players' units, even if you do not have units or control planets in that system."
  },
  "dark_energy_tap": {
    "id": "det",
    "name": "Dark Energy Tap",
    "description": "After you perform a tactical action in a system that contains a frontier token, if you have 1 or more ships in that system, explore that token.\nYour ships can retreat into adjacent systems that do not contain other players' units, even if you do not have units or control planets in that system."
  },
  "sr": {
    "id": "sr",
    "name": "Sling Relay",
    "description": "ACTION: Exhaust this card to produce 1 ship in any system that contains one of your space docks."
  },
  "sling_relay": {
    "id": "sr",
    "name": "Sling Relay",
    "description": "ACTION: Exhaust this card to produce 1 ship in any system that contains one of your space docks."
  },
  "nm": {
    "id": "nm",
    "name": "Neural Motivator",
    "description": "During the status phase, draw 2 action cards instead of 1."
  },
  "neural_motivator": {
    "id": "nm",
    "name": "Neural Motivator",
    "description": "During the status phase, draw 2 action cards instead of 1."
  },
  "dxa": {
    "id": "dxa",
    "name": "Dacxive Animators",
    "description": "After you win a ground combat, you may place 1 infantry from your reinforcements on that planet."
  },
  "dacxive_animators": {
    "id": "dxa",
    "name": "Dacxive Animators",
    "description": "After you win a ground combat, you may place 1 infantry from your reinforcements on that planet."
  },
  "hm": {
    "id": "hm",
    "name": "Hyper Metabolism",
    "description": "During the status phase, gain 3 command tokens instead of 2."
  },
  "hyper_metabolism": {
    "id": "hm",
    "name": "Hyper Metabolism",
    "description": "During the status phase, gain 3 command tokens instead of 2."
  },
  "x89_base": {
    "id": "x89_base",
    "name": "X-89 Bacterial Weapon",
    "description": "ACTION: Exhaust this card and choose 1 planet in a system that contains 1 or more of your ships that have BOMBARDMENT; destroy all infantry on that planet"
  },
  "x_89_bacterial_weapon": {
    "id": "x89_base",
    "name": "X-89 Bacterial Weapon",
    "description": "ACTION: Exhaust this card and choose 1 planet in a system that contains 1 or more of your ships that have BOMBARDMENT; destroy all infantry on that planet"
  },
  "x89": {
    "id": "x89",
    "name": "X-89 Bacterial Weapon Ω",
    "description": "After 1 or more of your units use BOMBARDMENT against a planet, if at least 1 of your opponent's infantry was destroyed, you may destroy all of your opponent's infantry on that planet."
  },
  "x_89_bacterial_weapon__": {
    "id": "x89",
    "name": "X-89 Bacterial Weapon Ω",
    "description": "After 1 or more of your units use BOMBARDMENT against a planet, if at least 1 of your opponent's infantry was destroyed, you may destroy all of your opponent's infantry on that planet."
  },
  "x89c4": {
    "id": "x89c4",
    "name": "X-89 Bacterial Weapon ΩΩ",
    "description": "Double the hits produced by your units' BOMBARDMENT and ground combat rolls.\nExhaust each planet you use BOMBARDMENT against."
  },
  "x_89_bacterial_weapon___": {
    "id": "x89c4",
    "name": "X-89 Bacterial Weapon ΩΩ",
    "description": "Double the hits produced by your units' BOMBARDMENT and ground combat rolls.\nExhaust each planet you use BOMBARDMENT against."
  },
  "pa": {
    "id": "pa",
    "name": "Psychoarchaeology",
    "description": "You can use technology specialties on planets you control without exhausting them, even if those planets are exhausted.\nDuring the action phase, you can exhaust planets you control that have technology specialties to gain 1 trade good."
  },
  "psychoarchaeology": {
    "id": "pa",
    "name": "Psychoarchaeology",
    "description": "You can use technology specialties on planets you control without exhausting them, even if those planets are exhausted.\nDuring the action phase, you can exhaust planets you control that have technology specialties to gain 1 trade good."
  },
  "bs": {
    "id": "bs",
    "name": "Bio-Stims",
    "description": "You may exhaust this card at the end of your turn to ready 1 of your planets that has a technology specialty or 1 of your other technologies."
  },
  "bio_stims": {
    "id": "bs",
    "name": "Bio-Stims",
    "description": "You may exhaust this card at the end of your turn to ready 1 of your planets that has a technology specialty or 1 of your other technologies."
  },
  "ps": {
    "id": "ps",
    "name": "Plasma Scoring",
    "description": "When 1 or more of your units use BOMBARDMENT or SPACE CANNON, 1 of those units may roll 1 additional die."
  },
  "plasma_scoring": {
    "id": "ps",
    "name": "Plasma Scoring",
    "description": "When 1 or more of your units use BOMBARDMENT or SPACE CANNON, 1 of those units may roll 1 additional die."
  },
  "md_base": {
    "id": "md_base",
    "name": "Magen Defense Grid",
    "description": "You may exhaust this card at the start of a round of ground combat on a planet that contains 1 or more of your units that have PLANETARY SHIELD; your opponent cannot make combat rolls this combat round."
  },
  "magen_defense_grid": {
    "id": "md_base",
    "name": "Magen Defense Grid",
    "description": "You may exhaust this card at the start of a round of ground combat on a planet that contains 1 or more of your units that have PLANETARY SHIELD; your opponent cannot make combat rolls this combat round."
  },
  "md_c1": {
    "id": "md_c1",
    "name": "Magen Defense Grid Ω",
    "description": "At the start of ground combat on a planet that contains 1 or more of your structures, produce 1 hit and assign it to 1 of your opponent's ground forces."
  },
  "magen_defense_grid__": {
    "id": "md_c1",
    "name": "Magen Defense Grid Ω",
    "description": "At the start of ground combat on a planet that contains 1 or more of your structures, produce 1 hit and assign it to 1 of your opponent's ground forces."
  },
  "md": {
    "id": "md",
    "name": "Magen Defense Grid ΩΩ",
    "description": "When any player activates a system that contains 1 or more of your structures, place 1 infantry from your reinforcements with each of those structures.\nAt the start of ground combat on a planet that contains 1 or more of your structures, produce 1 hit and assign it to 1 of your opponent's ground forces."
  },
  "magen_defense_grid___": {
    "id": "md",
    "name": "Magen Defense Grid ΩΩ",
    "description": "When any player activates a system that contains 1 or more of your structures, place 1 infantry from your reinforcements with each of those structures.\nAt the start of ground combat on a planet that contains 1 or more of your structures, produce 1 hit and assign it to 1 of your opponent's ground forces."
  },
  "da": {
    "id": "da",
    "name": "Duranium Armor",
    "description": "During each combat round, after you assign hits to your units, repair 1 of your damaged units that did not use SUSTAIN DAMAGE during this combat round."
  },
  "duranium_armor": {
    "id": "da",
    "name": "Duranium Armor",
    "description": "During each combat round, after you assign hits to your units, repair 1 of your damaged units that did not use SUSTAIN DAMAGE during this combat round."
  },
  "asc": {
    "id": "asc",
    "name": "Assault Cannon",
    "description": "At the start of a space combat in a system that contains 3 or more of your non-fighter ships, your opponent must destroy 1 of their non-fighter ships."
  },
  "assault_cannon": {
    "id": "asc",
    "name": "Assault Cannon",
    "description": "At the start of a space combat in a system that contains 3 or more of your non-fighter ships, your opponent must destroy 1 of their non-fighter ships."
  },
  "aida": {
    "id": "aida",
    "name": "AI Development Algorithm",
    "description": "When you research a unit upgrade technology, you may exhaust this card to ignore any 1 prerequisite.\nWhen 1 or more of your units use PRODUCTION, you may exhaust this card to reduce the combined cost of the produced units by the number of unit upgrade technologies that you own."
  },
  "ai_development_algorithm": {
    "id": "aida",
    "name": "AI Development Algorithm",
    "description": "When you research a unit upgrade technology, you may exhaust this card to ignore any 1 prerequisite.\nWhen 1 or more of your units use PRODUCTION, you may exhaust this card to reduce the combined cost of the produced units by the number of unit upgrade technologies that you own."
  },
  "sar": {
    "id": "sar",
    "name": "Self-Assembly Routines",
    "description": "After 1 or more of your units use PRODUCTION, you may exhaust this card to place 1 mech from your reinforcements on a planet you control in that system.\nAfter 1 of your mechs is destroyed, gain 1 trade good."
  },
  "self_assembly_routines": {
    "id": "sar",
    "name": "Self-Assembly Routines",
    "description": "After 1 or more of your units use PRODUCTION, you may exhaust this card to place 1 mech from your reinforcements on a planet you control in that system.\nAfter 1 of your mechs is destroyed, gain 1 trade good."
  },
  "st": {
    "id": "st",
    "name": "Sarween Tools",
    "description": "When 1 or more of your units use PRODUCTION, reduce the combined cost of the produced units by 1."
  },
  "sarween_tools": {
    "id": "st",
    "name": "Sarween Tools",
    "description": "When 1 or more of your units use PRODUCTION, reduce the combined cost of the produced units by 1."
  },
  "gls": {
    "id": "gls",
    "name": "Graviton Laser System",
    "description": "You may exhaust this card before 1 or more of your units uses SPACE CANNON; hits produced by those units must be assigned to non-fighter ships if able."
  },
  "graviton_laser_system": {
    "id": "gls",
    "name": "Graviton Laser System",
    "description": "You may exhaust this card before 1 or more of your units uses SPACE CANNON; hits produced by those units must be assigned to non-fighter ships if able."
  },
  "td": {
    "id": "td",
    "name": "Transit Diodes",
    "description": "You may exhaust this card at the start of your turn during the action phase; remove up to 4 of your ground forces from the game board and place them on 1 or more planets you control."
  },
  "transit_diodes": {
    "id": "td",
    "name": "Transit Diodes",
    "description": "You may exhaust this card at the start of your turn during the action phase; remove up to 4 of your ground forces from the game board and place them on 1 or more planets you control."
  },
  "ie": {
    "id": "ie",
    "name": "Integrated Economy",
    "description": "After you gain control of a planet, you may produce any number of units on that planet that have a combined cost equal to or less than that planet's resource value."
  },
  "integrated_economy": {
    "id": "ie",
    "name": "Integrated Economy",
    "description": "After you gain control of a planet, you may produce any number of units on that planet that have a combined cost equal to or less than that planet's resource value."
  },
  "sdn": {
    "id": "sdn",
    "name": "Scanlink Drone Network",
    "description": "When you activate a system, you may explore 1 planet in that system which contains 1 or more of your units."
  },
  "scanlink_drone_network": {
    "id": "sdn",
    "name": "Scanlink Drone Network",
    "description": "When you activate a system, you may explore 1 planet in that system which contains 1 or more of your units."
  },
  "pi": {
    "id": "pi",
    "name": "Predictive Intelligence",
    "description": "At the end of your turn, you may exhaust this card to redistribute your command tokens.\nWhen you cast votes during the agenda phase, you may cast 3 additional votes; if you do, and the outcome you voted for is not resolved, exhaust this card."
  },
  "predictive_intelligence": {
    "id": "pi",
    "name": "Predictive Intelligence",
    "description": "At the end of your turn, you may exhaust this card to redistribute your command tokens.\nWhen you cast votes during the agenda phase, you may cast 3 additional votes; if you do, and the outcome you voted for is not resolved, exhaust this card."
  },
  "ws": {
    "id": "ws",
    "name": "War Sun",
    "description": "Cost 12, Combat 3(x3), Move 2, Capacity 6\nSUSTAIN DAMAGE, BOMBARDMENT 3(x3)\n Other players' units in this system lose PLANETARY SHIELD."
  },
  "war_sun": {
    "id": "ws",
    "name": "War Sun",
    "description": "Cost 12, Combat 3(x3), Move 2, Capacity 6\nSUSTAIN DAMAGE, BOMBARDMENT 3(x3)\n Other players' units in this system lose PLANETARY SHIELD."
  },
  "sd2": {
    "id": "sd2",
    "name": "Space Dock II",
    "description": "PRODUCTION X\nThis unit's PRODUCTION value is equal to 4 more than the resource value of this planet.\nUp to 3 fighters in this system do not count against your ships' capacity."
  },
  "space_dock_ii": {
    "id": "sd2",
    "name": "Space Dock II",
    "description": "PRODUCTION X\nThis unit's PRODUCTION value is equal to 4 more than the resource value of this planet.\nUp to 3 fighters in this system do not count against your ships' capacity."
  },
  "cr2": {
    "id": "cr2",
    "name": "Cruiser II",
    "description": "Cost 2, Combat 6, Move 3, Capacity 1"
  },
  "cruiser_ii": {
    "id": "cr2",
    "name": "Cruiser II",
    "description": "Cost 2, Combat 6, Move 3, Capacity 1"
  },
  "dn2": {
    "id": "dn2",
    "name": "Dreadnought II",
    "description": "Cost 4, Combat 5, Move 2, Capacity 1\nSUSTAIN DAMAGE, BOMBARDMENT 5\nThis unit cannot be destroyed by \"Direct Hit\" action cards."
  },
  "dreadnought_ii": {
    "id": "dn2",
    "name": "Dreadnought II",
    "description": "Cost 4, Combat 5, Move 2, Capacity 1\nSUSTAIN DAMAGE, BOMBARDMENT 5\nThis unit cannot be destroyed by \"Direct Hit\" action cards."
  },
  "dd2": {
    "id": "dd2",
    "name": "Destroyer II",
    "description": "Cost 1, Combat 8, Move 2\nANTI-FIGHTER BARRAGE 6(x3)"
  },
  "destroyer_ii": {
    "id": "dd2",
    "name": "Destroyer II",
    "description": "Cost 1, Combat 8, Move 2\nANTI-FIGHTER BARRAGE 6(x3)"
  },
  "pds2": {
    "id": "pds2",
    "name": "PDS II",
    "description": "PLANETARY SHIELD, SPACE CANNON 5\nYou may use this unit's SPACE CANNON against ships that are adjacent to this unit's system."
  },
  "pds_ii": {
    "id": "pds2",
    "name": "PDS II",
    "description": "PLANETARY SHIELD, SPACE CANNON 5\nYou may use this unit's SPACE CANNON against ships that are adjacent to this unit's system."
  },
  "cv2": {
    "id": "cv2",
    "name": "Carrier II",
    "description": "Cost 3, Combat 9, Move 2, Capacity 6"
  },
  "carrier_ii": {
    "id": "cv2",
    "name": "Carrier II",
    "description": "Cost 3, Combat 9, Move 2, Capacity 6"
  },
  "ff2": {
    "id": "ff2",
    "name": "Fighter II",
    "description": "Cost 1(x2), Combat 8, Move 2\nThis unit may move without being transported.\nFighters in excess of your ships' capacity count against your fleet pool."
  },
  "fighter_ii": {
    "id": "ff2",
    "name": "Fighter II",
    "description": "Cost 1(x2), Combat 8, Move 2\nThis unit may move without being transported.\nFighters in excess of your ships' capacity count against your fleet pool."
  },
  "inf2": {
    "id": "inf2",
    "name": "Infantry II",
    "description": "Cost 1(x2), Combat 7\nAfter this unit is destroyed, roll 1 die. If the result is 6 or greater, place the unit on this card. At the start of your next turn, place each unit that is on this card on a planet you control in your home system."
  },
  "infantry_ii": {
    "id": "inf2",
    "name": "Infantry II",
    "description": "Cost 1(x2), Combat 7\nAfter this unit is destroyed, roll 1 die. If the result is 6 or greater, place the unit on this card. At the start of your next turn, place each unit that is on this card on a planet you control in your home system."
  },
  "lw2": {
    "id": "lw2",
    "name": "Letani Warrior II",
    "description": "Cost 1(x2), Combat 7\nPRODUCTION 2\nAfter this unit is destroyed, roll 1 die. If the result is 6 or greater, place the unit on this card. At the start of your next turn, place each unit that is on this card on a planet you control in your home system."
  },
  "letani_warrior_ii": {
    "id": "lw2",
    "name": "Letani Warrior II",
    "description": "Cost 1(x2), Combat 7\nPRODUCTION 2\nAfter this unit is destroyed, roll 1 die. If the result is 6 or greater, place the unit on this card. At the start of your next turn, place each unit that is on this card on a planet you control in your home system."
  },
  "swa2": {
    "id": "swa2",
    "name": "Strike Wing Alpha II",
    "description": "Cost 1, Combat 7, Move 2, Capacity 1\nANTI-FIGHTER BARRAGE 6(x3)\nWhen this unit uses ANTI-FIGHTER BARRAGE, each result of 9 or 10 also destroys 1 of your opponents infantry in the space area of the active system."
  },
  "strike_wing_alpha_ii": {
    "id": "swa2",
    "name": "Strike Wing Alpha II",
    "description": "Cost 1, Combat 7, Move 2, Capacity 1\nANTI-FIGHTER BARRAGE 6(x3)\nWhen this unit uses ANTI-FIGHTER BARRAGE, each result of 9 or 10 also destroys 1 of your opponents infantry in the space area of the active system."
  },
  "l4": {
    "id": "l4",
    "name": "L4 Disruptors",
    "description": "During an invasion, units cannot use SPACE CANNON against your units."
  },
  "l4_disruptors": {
    "id": "l4",
    "name": "L4 Disruptors",
    "description": "During an invasion, units cannot use SPACE CANNON against your units."
  },
  "cm": {
    "id": "cm",
    "name": "Chaos Mapping",
    "description": "Other players cannot activate asteroid fields that contain 1 or more of your ships.\nAt the start of your turn during the action phase, you may produce 1 unit in a system that contains at least 1 of your units that has PRODUCTION."
  },
  "chaos_mapping": {
    "id": "cm",
    "name": "Chaos Mapping",
    "description": "Other players cannot activate asteroid fields that contain 1 or more of your ships.\nAt the start of your turn during the action phase, you may produce 1 unit in a system that contains at least 1 of your units that has PRODUCTION."
  },
  "pws2": {
    "id": "pws2",
    "name": "Prototype War Sun II",
    "description": "Cost 10, Combat 3(x3), Move 3, Capacity 6\nSUSTAIN DAMAGE, BOMBARDMENT 3(x3)\nOther players' units in this system lose PLANETARY SHIELD."
  },
  "prototype_war_sun_ii": {
    "id": "pws2",
    "name": "Prototype War Sun II",
    "description": "Cost 10, Combat 3(x3), Move 3, Capacity 6\nSUSTAIN DAMAGE, BOMBARDMENT 3(x3)\nOther players' units in this system lose PLANETARY SHIELD."
  },
  "qdn": {
    "id": "qdn",
    "name": "Quantum Datahub Node",
    "description": "At the end of the strategy phase, you may spend 1 token from your strategy pool and give another player 3 of your trade goods. If you do, give 1 of your strategy cards to that player and take 1 of their strategy cards."
  },
  "quantum_datahub_node": {
    "id": "qdn",
    "name": "Quantum Datahub Node",
    "description": "At the end of the strategy phase, you may spend 1 token from your strategy pool and give another player 3 of your trade goods. If you do, give 1 of your strategy cards to that player and take 1 of their strategy cards."
  },
  "as": {
    "id": "as",
    "name": "Aetherstream",
    "description": "After you or one of your neighbors activates a system that is adjacent to an anomaly, you may apply +1 to the move value of all of that player's ships during this tactical action."
  },
  "aetherstream": {
    "id": "as",
    "name": "Aetherstream",
    "description": "After you or one of your neighbors activates a system that is adjacent to an anomaly, you may apply +1 to the move value of all of that player's ships during this tactical action."
  },
  "ac2": {
    "id": "ac2",
    "name": "Advanced Carrier II",
    "description": "Cost 3, Combat 9, Move 2, Capacity 8\nSUSTAIN DAMAGE"
  },
  "advanced_carrier_ii": {
    "id": "ac2",
    "name": "Advanced Carrier II",
    "description": "Cost 3, Combat 9, Move 2, Capacity 8\nSUSTAIN DAMAGE"
  },
  "sdn2": {
    "id": "sdn2",
    "name": "Super Dreadnought II",
    "description": "Cost 4, Combat 4, Move 2, Capacity 2\nSUSTAIN DAMAGE, BOMBARDMENT 4\nThis unit cannot be destroyed by \"Direct Hit\" action cards."
  },
  "super_dreadnought_ii": {
    "id": "sdn2",
    "name": "Super Dreadnought II",
    "description": "Cost 4, Combat 4, Move 2, Capacity 2\nSUSTAIN DAMAGE, BOMBARDMENT 4\nThis unit cannot be destroyed by \"Direct Hit\" action cards."
  },
  "cl2": {
    "id": "cl2",
    "name": "Crimson Legionnaire II",
    "description": "Cost 1(x2), Combat 7\nAfter this unit is destroyed, gain 1 commodity or convert 1 of your commodities to a trade good. Then, place the unit on this card. At the start of your next turn, place each unit that is on this card on a planet you control in your home system."
  },
  "crimson_legionnaire_ii": {
    "id": "cl2",
    "name": "Crimson Legionnaire II",
    "description": "Cost 1(x2), Combat 7\nAfter this unit is destroyed, gain 1 commodity or convert 1 of your commodities to a trade good. Then, place the unit on this card. At the start of your next turn, place each unit that is on this card on a planet you control in your home system."
  },
  "mc": {
    "id": "mc",
    "name": "Mirror Computing",
    "description": "When you spend trade goods, each trade good is worth 2 resources or influence instead of 1."
  },
  "mirror_computing": {
    "id": "mc",
    "name": "Mirror Computing",
    "description": "When you spend trade goods, each trade good is worth 2 resources or influence instead of 1."
  },
  "hcf2": {
    "id": "hcf2",
    "name": "Hybrid Crystal Fighter II",
    "description": "Cost 1(x2), Combat 7, Move 2\nThis unit may move without being transported.\nFighters in excess of your ships' capacity count as 1/2 of a ship against your fleet pool."
  },
  "hybrid_crystal_fighter_ii": {
    "id": "hcf2",
    "name": "Hybrid Crystal Fighter II",
    "description": "Cost 1(x2), Combat 7, Move 2\nThis unit may move without being transported.\nFighters in excess of your ships' capacity count as 1/2 of a ship against your fleet pool."
  },
  "pfa": {
    "id": "pfa",
    "name": "Pre-Fab Arcologies",
    "description": "After you explore a planet, ready that planet."
  },
  "pre_fab_arcologies": {
    "id": "pfa",
    "name": "Pre-Fab Arcologies",
    "description": "After you explore a planet, ready that planet."
  },
  "vax": {
    "id": "vax",
    "name": "Valefar Assimilator X",
    "description": "When you would gain another player's technology using 1 of your faction abilities, you may place the \"X\" assimilator token on a faction technology owned by that player instead.\nWhile that token is on a technology, this card gains that technology's text.\nYou cannot place an assimilator token on technology that already has an assimilator token."
  },
  "valefar_assimilator_x": {
    "id": "vax",
    "name": "Valefar Assimilator X",
    "description": "When you would gain another player's technology using 1 of your faction abilities, you may place the \"X\" assimilator token on a faction technology owned by that player instead.\nWhile that token is on a technology, this card gains that technology's text.\nYou cannot place an assimilator token on technology that already has an assimilator token."
  },
  "m2": {
    "id": "m2",
    "name": "Memoria II",
    "description": "Cost 8, Combat 5(x2), Move 2, Capacity 6\nSUSTAIN DAMAGE, ANTI-FIGHTER BARRAGE 5(x3)\nYou may treat this unit as if it were adjacent to systems that contain one or more of your mechs."
  },
  "memoria_ii": {
    "id": "m2",
    "name": "Memoria II",
    "description": "Cost 8, Combat 5(x2), Move 2, Capacity 6\nSUSTAIN DAMAGE, ANTI-FIGHTER BARRAGE 5(x3)\nYou may treat this unit as if it were adjacent to systems that contain one or more of your mechs."
  },
  "exo2": {
    "id": "exo2",
    "name": "Exotrireme II",
    "description": "Cost 4, Combat 5, Move 2, Capacity 1\nSUSTAIN DAMAGE, BOMBARDMENT 4(x2)\nThis unit cannot be destroyed by \"Direct Hit\" action cards.\nAfter a round of space combat, you may destroy this unit to destroy up to 2 ships in this system."
  },
  "exotrireme_ii": {
    "id": "exo2",
    "name": "Exotrireme II",
    "description": "Cost 4, Combat 5, Move 2, Capacity 1\nSUSTAIN DAMAGE, BOMBARDMENT 4(x2)\nThis unit cannot be destroyed by \"Direct Hit\" action cards.\nAfter a round of space combat, you may destroy this unit to destroy up to 2 ships in this system."
  },
  "se2": {
    "id": "se2",
    "name": "Saturn Engine II",
    "description": "Cost 2, Combat 6, Move 3, Capacity 2\nSUSTAIN DAMAGE"
  },
  "saturn_engine_ii": {
    "id": "se2",
    "name": "Saturn Engine II",
    "description": "Cost 2, Combat 6, Move 3, Capacity 2\nSUSTAIN DAMAGE"
  },
  "scc": {
    "id": "scc",
    "name": "Spatial Conduit Cylinders",
    "description": "You may exhaust this card after you activate a system that contains 1 or more of your units; that system is adjacent to all other systems that contain 1 or more of your units during this activation."
  },
  "spatial_conduit_cylinders": {
    "id": "scc",
    "name": "Spatial Conduit Cylinders",
    "description": "You may exhaust this card after you activate a system that contains 1 or more of your units; that system is adjacent to all other systems that contain 1 or more of your units during this activation."
  },
  "dt2": {
    "id": "dt2",
    "name": "Dimensional Tear II",
    "description": "PRODUCTION 7\nThis system is a gravity rift; your ships do not roll for this gravity rift. Place a dimensional tear token beneath this unit as a reminder.\nUp to 12 fighters in this system do not count against your ships' capacity."
  },
  "dimensional_tear_ii": {
    "id": "dt2",
    "name": "Dimensional Tear II",
    "description": "PRODUCTION 7\nThis system is a gravity rift; your ships do not roll for this gravity rift. Place a dimensional tear token beneath this unit as a reminder.\nUp to 12 fighters in this system do not count against your ships' capacity."
  },
  "lgf": {
    "id": "lgf",
    "name": "Lazax Gate Folding",
    "description": "During your tactical actions, if you do not control Mecatol Rex, treat its system as if it has both an α and β wormhole.\nACTION: If you control Mecatol Rex, exhaust this card to place 1 infantry from your reinforcements on Mecatol Rex."
  },
  "lazax_gate_folding": {
    "id": "lgf",
    "name": "Lazax Gate Folding",
    "description": "During your tactical actions, if you do not control Mecatol Rex, treat its system as if it has both an α and β wormhole.\nACTION: If you control Mecatol Rex, exhaust this card to place 1 infantry from your reinforcements on Mecatol Rex."
  },
  "it": {
    "id": "it",
    "name": "Instinct Training",
    "description": "You may exhaust this card and spend 1 token from your strategy pool when another player plays an action card; cancel that action card."
  },
  "instinct_training": {
    "id": "it",
    "name": "Instinct Training",
    "description": "You may exhaust this card and spend 1 token from your strategy pool when another player plays an action card; cancel that action card."
  },
  "mi": {
    "id": "mi",
    "name": "Mageon Implants",
    "description": "ACTION: Exhaust this card to look at another player's hand of action cards.  Choose 1 of those cards and add it to your hand."
  },
  "mageon_implants": {
    "id": "mi",
    "name": "Mageon Implants",
    "description": "ACTION: Exhaust this card to look at another player's hand of action cards.  Choose 1 of those cards and add it to your hand."
  },
  "bio": {
    "id": "bio",
    "name": "Bioplasmosis",
    "description": "At the end of the status phase, you may remove any number of infantry from planets you control and place them on 1 or more planets you control in the same or adjacent systems."
  },
  "bioplasmosis": {
    "id": "bio",
    "name": "Bioplasmosis",
    "description": "At the end of the status phase, you may remove any number of infantry from planets you control and place them on 1 or more planets you control in the same or adjacent systems."
  },
  "ah": {
    "id": "ah",
    "name": "Aerie Hololattice",
    "description": "Other players cannot move ships through systems that contain your structures.\nEach planet that contains 1 or more of your structures gains the PRODUCTION 1 ability as if it were a unit."
  },
  "aerie_hololattice": {
    "id": "ah",
    "name": "Aerie Hololattice",
    "description": "Other players cannot move ships through systems that contain your structures.\nEach planet that contains 1 or more of your structures gains the PRODUCTION 1 ability as if it were a unit."
  },
  "nes": {
    "id": "nes",
    "name": "Non-Euclidean Shielding",
    "description": "When 1 of your units uses SUSTAIN DAMAGE, cancel 2 hits instead of 1."
  },
  "non_euclidean_shielding": {
    "id": "nes",
    "name": "Non-Euclidean Shielding",
    "description": "When 1 of your units uses SUSTAIN DAMAGE, cancel 2 hits instead of 1."
  },
  "ffac2": {
    "id": "ffac2",
    "name": "Floating Factory II",
    "description": "Move 2, Capacity 5\nPRODUCTION 7.\nThis unit is placed in the space area instead of on a planet.\nThis unit can move and retreat as if it were a ship.\nIf this unit is blockaded, it is destroyed."
  },
  "floating_factory_ii": {
    "id": "ffac2",
    "name": "Floating Factory II",
    "description": "Move 2, Capacity 5\nPRODUCTION 7.\nThis unit is placed in the space area instead of on a planet.\nThis unit can move and retreat as if it were a ship.\nIf this unit is blockaded, it is destroyed."
  },
  "mr": {
    "id": "mr",
    "name": "Magmus Reactor",
    "description": "Your ships can move into supernovas.\nEach supernova that contains 1 or more of your units gains the PRODUCTION 5 ability as if it were 1 of your units."
  },
  "magmus_reactor": {
    "id": "mr",
    "name": "Magmus Reactor",
    "description": "Your ships can move into supernovas.\nEach supernova that contains 1 or more of your units gains the PRODUCTION 5 ability as if it were 1 of your units."
  },
  "pm": {
    "id": "pm",
    "name": "Production Biomes",
    "description": "ACTION: Exhaust this card and spend 1 token from your strategy pool to gain 4 trade goods and choose 1 other player; that player gains 2 trade goods."
  },
  "production_biomes": {
    "id": "pm",
    "name": "Production Biomes",
    "description": "ACTION: Exhaust this card and spend 1 token from your strategy pool to gain 4 trade goods and choose 1 other player; that player gains 2 trade goods."
  },
  "vw": {
    "id": "vw",
    "name": "Voidwatch",
    "description": "After a player moves ships into a system that contains 1 or more of your units, they must give you 1 promissory note from their hand, if able."
  },
  "voidwatch": {
    "id": "vw",
    "name": "Voidwatch",
    "description": "After a player moves ships into a system that contains 1 or more of your units, they must give you 1 promissory note from their hand, if able."
  },
  "so2": {
    "id": "so2",
    "name": "Spec Ops II",
    "description": "Cost 1(x2), Combat 6\nAfter this unit is destroyed, roll 1 die. If the result is 5 or greater, place the unit on this card. At the start of your next turn, place each unit that is on this card on a planet you control in your home system."
  },
  "spec_ops_ii": {
    "id": "so2",
    "name": "Spec Ops II",
    "description": "Cost 1(x2), Combat 6\nAfter this unit is destroyed, roll 1 die. If the result is 5 or greater, place the unit on this card. At the start of your next turn, place each unit that is on this card on a planet you control in your home system."
  },
  "wg": {
    "id": "wg",
    "name": "Wormhole Generator",
    "description": "ACTION: Exhaust this card to place or move a Creuss wormhole token into either a system that contains a planet you control or a non-home system that does not contain another player's ships."
  },
  "wormhole_generator": {
    "id": "wg",
    "name": "Wormhole Generator",
    "description": "ACTION: Exhaust this card to place or move a Creuss wormhole token into either a system that contains a planet you control or a non-home system that does not contain another player's ships."
  },
  "ds": {
    "id": "ds",
    "name": "Dimensional Splicer",
    "description": "At the start of space combat in a system that contains a wormhole and 1 or more of your ships, you may produce 1 hit and assign it to 1 of your opponent's ships."
  },
  "dimensional_splicer": {
    "id": "ds",
    "name": "Dimensional Splicer",
    "description": "At the start of space combat in a system that contains a wormhole and 1 or more of your ships, you may produce 1 hit and assign it to 1 of your opponent's ships."
  },
  "is": {
    "id": "is",
    "name": "Inheritance Systems",
    "description": "You may exhaust this card and spend 2 resources when you research a technology; ignore all of that technology's prerequisites."
  },
  "inheritance_systems": {
    "id": "is",
    "name": "Inheritance Systems",
    "description": "You may exhaust this card and spend 2 resources when you research a technology; ignore all of that technology's prerequisites."
  },
  "gr": {
    "id": "gr",
    "name": "Genetic Recombination",
    "description": "You may exhaust this card before a player casts votes; that player must cast at least 1 vote for an outcome of your choice or remove 1 token from their fleet pool and return it to their reinforcements."
  },
  "genetic_recombination": {
    "id": "gr",
    "name": "Genetic Recombination",
    "description": "You may exhaust this card before a player casts votes; that player must cast at least 1 vote for an outcome of your choice or remove 1 token from their fleet pool and return it to their reinforcements."
  },
  "so": {
    "id": "so",
    "name": "Salvage Operations",
    "description": "After you win or lose a space combat, gain 1 trade good; if you won the combat, you may also produce 1 ship in that system of any ship type that was destroyed during the combat."
  },
  "salvage_operations": {
    "id": "so",
    "name": "Salvage Operations",
    "description": "After you win or lose a space combat, gain 1 trade good; if you won the combat, you may also produce 1 ship in that system of any ship type that was destroyed during the combat."
  },
  "ng": {
    "id": "ng",
    "name": "Neuroglaive",
    "description": "After another player activates a system that contains 1 or more of your ships, that player removes 1 token from their fleet pool and returns it to their reinforcements."
  },
  "neuroglaive": {
    "id": "ng",
    "name": "Neuroglaive",
    "description": "After another player activates a system that contains 1 or more of your ships, that player removes 1 token from their fleet pool and returns it to their reinforcements."
  },
  "sc": {
    "id": "sc",
    "name": "Supercharge",
    "description": "At the start of a combat round, you may exhaust this card to apply +1 to the result of each of your unit's combat rolls during this combat round."
  },
  "supercharge": {
    "id": "sc",
    "name": "Supercharge",
    "description": "At the start of a combat round, you may exhaust this card to apply +1 to the result of each of your unit's combat rolls during this combat round."
  },
  "vay": {
    "id": "vay",
    "name": "Valefar Assimilator Y",
    "description": "When you would gain another player's technology using 1 of your faction abilities, you may place the \"Y\" assimilator token on a faction technology owned by that player instead.\nWhile that token is on a technology, this card gains that technology's text.\nYou cannot place an assimilator token on technology that already has an assimilator token."
  },
  "valefar_assimilator_y": {
    "id": "vay",
    "name": "Valefar Assimilator Y",
    "description": "When you would gain another player's technology using 1 of your faction abilities, you may place the \"Y\" assimilator token on a faction technology owned by that player instead.\nWhile that token is on a technology, this card gains that technology's text.\nYou cannot place an assimilator token on technology that already has an assimilator token."
  },
  "tcs": {
    "id": "tcs",
    "name": "Temporal Command Suite",
    "description": "After any player's agent becomes exhausted, you may exhaust this card to ready that agent; if you ready another player's agent, you may perform a transaction with that player."
  },
  "temporal_command_suite": {
    "id": "tcs",
    "name": "Temporal Command Suite",
    "description": "After any player's agent becomes exhausted, you may exhaust this card to ready that agent; if you ready another player's agent, you may perform a transaction with that player."
  },
  "vpw": {
    "id": "vpw",
    "name": "Valkyrie Particle Weave",
    "description": "After making combat rolls during a round of ground combat, if your opponent produced 1 or more hits, you produce 1 additional hit."
  },
  "valkyrie_particle_weave": {
    "id": "vpw",
    "name": "Valkyrie Particle Weave",
    "description": "After making combat rolls during a round of ground combat, if your opponent produced 1 or more hits, you produce 1 additional hit."
  },
  "ht2": {
    "id": "ht2",
    "name": "Hel-Titan II",
    "description": "Combat 6\nPLANETARY SHIELD, SPACE CANNON 5, SUSTAIN DAMAGE, PRODUCTION 1\nThis unit is treated as both a structure and a ground force. It cannot be transported.\nYou may use this unit's SPACE CANNON against ships that are adjacent to this unit's systems."
  },
  "hel_titan_ii": {
    "id": "ht2",
    "name": "Hel-Titan II",
    "description": "Combat 6\nPLANETARY SHIELD, SPACE CANNON 5, SUSTAIN DAMAGE, PRODUCTION 1\nThis unit is treated as both a structure and a ground force. It cannot be transported.\nYou may use this unit's SPACE CANNON against ships that are adjacent to this unit's systems."
  },
  "ers": {
    "id": "ers",
    "name": "E-Res Siphons",
    "description": "After another player activates a system that contains 1 or more of your ships, gain 4 trade goods."
  },
  "e_res_siphons": {
    "id": "ers",
    "name": "E-Res Siphons",
    "description": "After another player activates a system that contains 1 or more of your ships, gain 4 trade goods."
  },
  "vtx": {
    "id": "vtx",
    "name": "Vortex",
    "description": "ACTION: Exhaust this card to choose another player's non-structure unit in a system that is adjacent to 1 or more of your space docks. Capture 1 unit of that type from that player's reinforcements."
  },
  "vortex": {
    "id": "vtx",
    "name": "Vortex",
    "description": "ACTION: Exhaust this card to choose another player's non-structure unit in a system that is adjacent to 1 or more of your space docks. Capture 1 unit of that type from that player's reinforcements."
  },
  "htp": {
    "id": "htp",
    "name": "Hegemonic Trade Policy",
    "description": "Exhaust this card when 1 or more of your units use PRODUCTION; swap the resource and influence values of 1 planet you control during that use of Production."
  },
  "hegemonic_trade_policy": {
    "id": "htp",
    "name": "Hegemonic Trade Policy",
    "description": "Exhaust this card when 1 or more of your units use PRODUCTION; swap the resource and influence values of 1 planet you control during that use of Production."
  },
  "nf": {
    "id": "nf",
    "name": "Nullification Field",
    "description": "After another player activates a system that contains 1 or more of your ships, you may exhaust this card and spend 1 token from your strategy pool; immediately end that player's turn."
  },
  "nullification_field": {
    "id": "nf",
    "name": "Nullification Field",
    "description": "After another player activates a system that contains 1 or more of your ships, you may exhaust this card and spend 1 token from your strategy pool; immediately end that player's turn."
  },
  "yso": {
    "id": "yso",
    "name": "Yin Spinner Omega",
    "description": "After you produce units, place up to 2 infantry from your reinforcements on any planet you control or in any space area that contains 1 or more of your ships."
  },
  "yin_spinner_omega": {
    "id": "yso",
    "name": "Yin Spinner Omega",
    "description": "After you produce units, place up to 2 infantry from your reinforcements on any planet you control or in any space area that contains 1 or more of your ships."
  },
  "ic": {
    "id": "ic",
    "name": "Impulse Core",
    "description": "At the start of a space combat, you may destroy 1 of your cruisers or destroyers in the active system to produce 1 hit against your opponent's ships; that hit must be assigned by your opponent to 1 of their non-fighters ships if able."
  },
  "impulse_core": {
    "id": "ic",
    "name": "Impulse Core",
    "description": "At the start of a space combat, you may destroy 1 of your cruisers or destroyers in the active system to produce 1 hit against your opponent's ships; that hit must be assigned by your opponent to 1 of their non-fighters ships if able."
  },
  "tp": {
    "id": "tp",
    "name": "Transparasteel Plating",
    "description": "During your turn of the action phase, players that have passed cannot play action cards."
  },
  "transparasteel_plating": {
    "id": "tp",
    "name": "Transparasteel Plating",
    "description": "During your turn of the action phase, players that have passed cannot play action cards."
  },
  "iihq": {
    "id": "iihq",
    "name": "I.I.H.Q. Modernization",
    "description": "You are neighbors with all players that have units or control planets in or adjacent to the Mecatol Rex system.\nGain the Custodia Vigilia planet card and its legendary planet ability card. You cannot lose these cards, and this card cannot have an X or Y assimilator token placed on it."
  },
  "i_i_h_q__modernization": {
    "id": "iihq",
    "name": "I.I.H.Q. Modernization",
    "description": "You are neighbors with all players that have units or control planets in or adjacent to the Mecatol Rex system.\nGain the Custodia Vigilia planet card and its legendary planet ability card. You cannot lose these cards, and this card cannot have an X or Y assimilator token placed on it."
  },
  "asn": {
    "id": "asn",
    "name": "Agency Supply Network",
    "description": "Once per action, when you resolve a unit's PRODUCTION ability, you may resolve another of your unit's PRODUCTION abilities in any system."
  },
  "agency_supply_network": {
    "id": "asn",
    "name": "Agency Supply Network",
    "description": "Once per action, when you resolve a unit's PRODUCTION ability, you may resolve another of your unit's PRODUCTION abilities in any system."
  },
  "nekroc4y": {
    "id": "nekroc4y",
    "name": "???\\_NULL\\_REFERENCE\\_???",
    "description": "When one of your ships is destroyed, you may produce a ship of the same type at a space dock in your home system."
  },
  "_____null__reference_____": {
    "id": "nekroc4y",
    "name": "???\\_NULL\\_REFERENCE\\_???",
    "description": "When one of your ships is destroyed, you may produce a ship of the same type at a space dock in your home system."
  },
  "nekroc4r": {
    "id": "nekroc4r",
    "name": "???\\_ERROR\\_ERROR\\_???",
    "description": "ACTION: Exhaust this card to place 1 PDS on a planet you control. \nACTION: Exhaust this card to repair all of your damaged units. \nACTION: Exhaust this card and discard 1 action card to draw 1 action card."
  },
  "_____error__error_____": {
    "id": "nekroc4r",
    "name": "???\\_ERROR\\_ERROR\\_???",
    "description": "ACTION: Exhaust this card to place 1 PDS on a planet you control. \nACTION: Exhaust this card to repair all of your damaged units. \nACTION: Exhaust this card and discard 1 action card to draw 1 action card."
  },
  "proxima": {
    "id": "proxima",
    "name": "Proxima Targeting VI",
    "description": "Cancel 1 hit produced by BOMBARDMENT rolls made against your ground forces for each of your galvanized units present. \nAt the start of a round of ground combat, you may resolve BOMBARDMENT 8 (x3) against your opponent's ground forces; if you do, make an identical roll against your ground forces."
  },
  "proxima_targeting_vi": {
    "id": "proxima",
    "name": "Proxima Targeting VI",
    "description": "Cancel 1 hit produced by BOMBARDMENT rolls made against your ground forces for each of your galvanized units present. \nAt the start of a round of ground combat, you may resolve BOMBARDMENT 8 (x3) against your opponent's ground forces; if you do, make an identical roll against your ground forces."
  },
  "helios2": {
    "id": "helios2",
    "name": "4X41C \"Helios\" V2",
    "description": "This unit's PRODUCTION value is equal to 4 more than the resource value of this planet.\nThe resource value of this planet is increased by 2. \nUp to 3 fighters in this system do not count against your ships' capacity."
  },
  "4x41c__helios__v2": {
    "id": "helios2",
    "name": "4X41C \"Helios\" V2",
    "description": "This unit's PRODUCTION value is equal to 4 more than the resource value of this planet.\nThe resource value of this planet is increased by 2. \nUp to 3 fighters in this system do not count against your ships' capacity."
  },
  "hydrothermal": {
    "id": "hydrothermal",
    "name": "Hydrothermal Mining",
    "description": "At the start of the status phase, gain 1 trade good for each ocean card in play."
  },
  "hydrothermal_mining": {
    "id": "hydrothermal",
    "name": "Hydrothermal Mining",
    "description": "At the start of the status phase, gain 1 trade good for each ocean card in play."
  },
  "radical": {
    "id": "radical",
    "name": "Radical Advancement",
    "description": "At the start of the status phase, you may replace one of your non-unit upgrade technologies with a technology of the same color that has exactly 1 more prerequisite."
  },
  "radical_advancement": {
    "id": "radical",
    "name": "Radical Advancement",
    "description": "At the start of the status phase, you may replace one of your non-unit upgrade technologies with a technology of the same color that has exactly 1 more prerequisite."
  },
  "subatomic": {
    "id": "subatomic",
    "name": "Subatomic Splicer",
    "description": "When one of your ships is destroyed, you may produce a ship of the same type at a space dock in your home system."
  },
  "subatomic_splicer": {
    "id": "subatomic",
    "name": "Subatomic Splicer",
    "description": "When one of your ships is destroyed, you may produce a ship of the same type at a space dock in your home system."
  },
  "exile2": {
    "id": "exile2",
    "name": "Exile II",
    "description": "Cost 1, Combat 7, Move 2, ANTI-FIGHTER BARRAGE 6 (x3). At the end of any players' combat in this unit's system or up to 2 systems away, you may place 1 active or inactive breach in that system."
  },
  "exile_ii": {
    "id": "exile2",
    "name": "Exile II",
    "description": "Cost 1, Combat 7, Move 2, ANTI-FIGHTER BARRAGE 6 (x3). At the end of any players' combat in this unit's system or up to 2 systems away, you may place 1 active or inactive breach in that system."
  },
  "nanomachines": {
    "id": "nanomachines",
    "name": "Nanomachines",
    "description": "ACTION: Exhaust this card to place 1 PDS on a planet you control.\nACTION: Exhaust this card to repair all of your damaged units.\nACTION: Exhaust this card and discard 1 action card to draw 1 action card."
  },
  "linkship2": {
    "id": "linkship2",
    "name": "Linkship II",
    "description": "Cost 1, Combat 8, Move 4, ANTI-FIGHTER BARRAGE 6 (x3). This unit can use the SPACE CANNON ability of one of your structures in its space area; each linkship can trigger the same structure."
  },
  "linkship_ii": {
    "id": "linkship2",
    "name": "Linkship II",
    "description": "Cost 1, Combat 8, Move 4, ANTI-FIGHTER BARRAGE 6 (x3). This unit can use the SPACE CANNON ability of one of your structures in its space area; each linkship can trigger the same structure."
  },
  "planesplitter-firm": {
    "id": "planesplitter-firm",
    "name": "Planesplitter (Firmament)",
    "description": "When you gain this card, put The Fracture into play.\nFlip this card if the Obsidian faction is in play."
  },
  "planesplitter__firmament_": {
    "id": "planesplitter-firm",
    "name": "Planesplitter (Firmament)",
    "description": "When you gain this card, put The Fracture into play.\nFlip this card if the Obsidian faction is in play."
  },
  "parasite-firm": {
    "id": "parasite-firm",
    "name": "Neural Parasite (Firmament)",
    "description": "At the start of the status phase, you may place 1 infantry from your reinforcements on a planet you control in your home system.\nFlip this card if the Obsidian faction is in play."
  },
  "neural_parasite__firmament_": {
    "id": "parasite-firm",
    "name": "Neural Parasite (Firmament)",
    "description": "At the start of the status phase, you may place 1 infantry from your reinforcements on a planet you control in your home system.\nFlip this card if the Obsidian faction is in play."
  },
  "planesplitter-obs": {
    "id": "planesplitter-obs",
    "name": "Planesplitter (Obsidian)",
    "description": "When you perform a strategic action, you may move an ingress token into a system that contains or is adjacent to your units.\nThis technology cannot be researched."
  },
  "planesplitter__obsidian_": {
    "id": "planesplitter-obs",
    "name": "Planesplitter (Obsidian)",
    "description": "When you perform a strategic action, you may move an ingress token into a system that contains or is adjacent to your units.\nThis technology cannot be researched."
  },
  "parasite-obs": {
    "id": "parasite-obs",
    "name": "Neural Parasite (Obsidian)",
    "description": "At the start of your turn, destroy 1 of another player's infantry in or adjacent to a system that contains your infantry.\nThis technology cannot be researched."
  },
  "neural_parasite__obsidian_": {
    "id": "parasite-obs",
    "name": "Neural Parasite (Obsidian)",
    "description": "At the start of your turn, destroy 1 of another player's infantry in or adjacent to a system that contains your infantry.\nThis technology cannot be researched."
  },
  "executiveorder": {
    "id": "executiveorder",
    "name": "Executive Order",
    "description": "ACTION: Exhaust this card and draw the top or bottom card of the Agenda deck. Players immediately vote on this agenda as if you were the speaker; you can spend trade goods and resources on this agenda as if they were votes."
  },
  "executive_order": {
    "id": "executiveorder",
    "name": "Executive Order",
    "description": "ACTION: Exhaust this card and draw the top or bottom card of the Agenda deck. Players immediately vote on this agenda as if you were the speaker; you can spend trade goods and resources on this agenda as if they were votes."
  }
};

export function humanizeId(id: string): string {
  return id
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (char) => char.toUpperCase());
}

export function getStrategyCardMeta(id: string): StrategyCardMeta {
  const normalized = id.toLowerCase().trim();
  if (STRATEGY_CARDS[normalized]) {
    return STRATEGY_CARDS[normalized];
  }
  for (const card of Object.values(STRATEGY_CARDS)) {
    if (normalized.includes(card.name.toLowerCase()) || normalized === String(card.initiative)) {
      return card;
    }
  }
  return {
    id,
    name: humanizeId(id),
    initiative: 0,
    primaryText: '',
    secondaryText: '',
  };
}

export function getSecretObjectiveMeta(id: string): ObjectiveMeta {
  const normalized = id.toLowerCase().trim();
  if (SECRET_OBJECTIVES[normalized]) {
    return SECRET_OBJECTIVES[normalized];
  }
  return {
    id,
    name: humanizeId(id),
    phase: 'Secret',
    points: 1,
    description: 'Secret Objective',
  };
}

export function getPublicObjectiveMeta(id: string): ObjectiveMeta {
  const normalized = id.toLowerCase().trim();
  if (PUBLIC_OBJECTIVES[normalized]) {
    return PUBLIC_OBJECTIVES[normalized];
  }
  return {
    id,
    name: humanizeId(id),
    phase: 'Public',
    points: 1,
    description: 'Public Objective',
  };
}

export function getActionCardMeta(id: string): CardMeta {
  const normalized = id.toLowerCase().trim();
  if (ACTION_CARDS[normalized]) {
    return ACTION_CARDS[normalized];
  }
  return {
    id,
    name: humanizeId(id),
    description: 'Action Card',
  };
}

export function getTechnologyMeta(id: string): CardMeta {
  const normalized = id.toLowerCase().trim();
  if (TECHNOLOGIES[normalized]) {
    return TECHNOLOGIES[normalized];
  }
  return {
    id,
    name: humanizeId(id),
    description: 'Technology',
  };
}

export function formatActionDescription(optionId: string): string {
  if (!optionId) return '';
  const trimmed = optionId.trim();

  if (trimmed.startsWith('strategic|')) {
    const cardId = trimmed.slice('strategic|'.length);
    const meta = getStrategyCardMeta(cardId);
    return `Play Strategy Card: ${meta.initiative > 0 ? `${meta.initiative}. ` : ''}${meta.name}`;
  }

  if (trimmed.startsWith('pok') || STRATEGY_CARDS[trimmed.toLowerCase()]) {
    const meta = getStrategyCardMeta(trimmed);
    if (meta.initiative > 0) {
      return `Strategy Card: ${meta.initiative}. ${meta.name}`;
    }
  }

  if (trimmed.startsWith('tactical|')) {
    return `Tactical Action (System ${trimmed.slice('tactical|'.length)})`;
  }

  if (trimmed.startsWith('component|')) {
    return `Component Action: ${humanizeId(trimmed.slice('component|'.length))}`;
  }

  if (trimmed === 'pass') {
    return 'Pass Turn';
  }

  if (trimmed === 'generic') {
    return 'Confirm Selection';
  }

  if (trimmed === 'top') {
    return 'Place on Top of Deck';
  }

  if (trimmed === 'bottom') {
    return 'Place on Bottom of Deck';
  }

  if (trimmed.startsWith('produce|') || trimmed.startsWith('produce_unit|')) {
    const unit = trimmed.split('|')[1];
    return `Produce ${humanizeId(unit)}`;
  }

  if (trimmed.startsWith('place ')) {
    return humanizeId(trimmed);
  }

  return humanizeId(trimmed);
}
