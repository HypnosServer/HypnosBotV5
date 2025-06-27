use std::{
    collections::{HashMap, hash_map::Entry},
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
};

use flate2::bufread::GzDecoder;
use poise::serenity_prelude::prelude::TypeMapKey;
use valence_nbt::{Value, from_binary};

pub struct ScoreboardNames {
    pub names: Vec<String>,
    last_update: std::time::Instant,
}

impl ScoreboardNames {
    pub fn new() -> Self {
        Self {
            names: Vec::new(),
            last_update: std::time::Instant::now(),
        }
    }

    pub fn update(&mut self, names: Vec<String>) {
        self.names = names;
        self.last_update = std::time::Instant::now();
    }

    pub fn should_update(&self) -> bool {
        self.last_update.elapsed().as_secs() > 60 // 1 minute
    }
}

#[derive(Debug, Clone)]
pub struct Scoreboard {
    pub name: String,
    pub scores: HashMap<String, i32>,
    pub total: i64,
    last_update: std::time::Instant,
}

impl Scoreboard {
    pub fn new(name: String) -> Self {
        Self {
            name,
            scores: HashMap::new(),
            total: 0,
            last_update: std::time::Instant::now(),
        }
    }

    pub fn update(&mut self, scores: HashMap<String, i32>, total: i64) {
        self.scores = scores;
        self.total = total;
        self.last_update = std::time::Instant::now();
    }

    pub fn should_update(&self) -> bool {
        self.last_update.elapsed().as_secs() > 60 // 1 minute
    }
}

pub struct CachedScoreboard {
    pub scoreboard_names: ScoreboardNames,
    pub scoreboards: HashMap<String, Scoreboard>,
    // Bucket to delete scoreboards that are not used anymore
    path: PathBuf,
}

impl CachedScoreboard {
    pub fn new(path: PathBuf) -> Self {
        let mut s = Self {
            scoreboard_names: ScoreboardNames::new(),
            scoreboards: HashMap::new(),
            path,
        };
        s.load_names()
            .unwrap_or_else(|e| println!("Failed to load scoreboard names: {}", e));
        s
    }

    pub fn load_names(&mut self) -> Result<(), String> {
        let mut file =
            File::open(&self.path).map_err(|e| format!("Failed to open scoreboard file: {}", e))?;
        let mut buf = Vec::new();
        let mut d = GzDecoder::new(BufReader::new(&mut file));
        d.read_to_end(&mut buf)
            .map_err(|e| format!("Failed to read score file: {}", e))?;
        let (scoreboard, _) = from_binary::<String>(&mut buf.as_slice())
            .map_err(|e| format!("Failed to parse score file: {}", e))?;

        let Some(Value::Compound(data)) = scoreboard.get("data") else {
            return Err("No data found in scoreboard".to_string());
        };
        let Some(Value::List(objectives)) = data.get("Objectives") else {
            return Err("No objectives found in scoreboard".to_string());
        };
        let names = objectives
            .iter()
            .filter_map(|objective| {
                if let Value::Compound(compound) = objective.to_value() {
                    compound.get("Name").and_then(|name| {
                        if let Value::String(name_str) = name {
                            Some(name_str.to_string())
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();
        self.scoreboard_names.update(names);

        Ok(())
    }

    pub fn load_scoreboard(&mut self, name: &str) -> Result<(), String> {
        let mut file =
            File::open(&self.path).map_err(|e| format!("Failed to open scoreboard file: {}", e))?;
        let mut buf = Vec::new();
        let mut d = GzDecoder::new(BufReader::new(&mut file));
        d.read_to_end(&mut buf)
            .map_err(|e| format!("Failed to read score file: {}", e))?;
        let (scoreboard, _) = from_binary::<String>(&mut buf.as_slice())
            .map_err(|e| format!("Failed to parse score file: {}", e))?;

        let Some(Value::Compound(data)) = scoreboard.get("data") else {
            return Err("No data found in scoreboard".to_string());
        };

        let Some(Value::List(player_scores)) = data.get("PlayerScores") else {
            return Err("No player scores found in scoreboard".to_string());
        };

        let mut scores = HashMap::new();
        let mut total = 0;
        for score in player_scores.iter() {
            if let Value::Compound(compound) = score.to_value() {
                if let Some(Value::String(objective_name)) = compound.get("Objective") {
                    if objective_name == name {
                        if let Some(Value::String(player_name)) = compound.get("Name") {
                            if player_name == "Total" {
                                continue;
                            }
                            if let Some(Value::Int(score_value)) = compound.get("Score") {
                                scores.insert(player_name.to_string(), *score_value);
                                total += *score_value as i64;
                            }
                        }
                    }
                }
            }
        }

        if scores.is_empty() {
            return Err(format!("No scores found for objective '{}'", name));
        }

        match self.scoreboards.entry(name.to_string()) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().update(scores, total);
            }
            Entry::Vacant(entry) => {
                let scoreboard = Scoreboard::new(name.to_string());
                entry.insert(scoreboard).update(scores, total);
            }
        }

        Ok(())
    }

    pub fn get_scoreboard(&mut self, name: &str) -> Result<&Scoreboard, String> {
        if self.scoreboards.get(name).is_none() {
            self.load_scoreboard(name)?;
        }
        self.scoreboards
            .get(name)
            .ok_or_else(|| format!("Scoreboard '{}' not found", name))
    }
}

pub struct Scoreboards;

impl TypeMapKey for Scoreboards {
    type Value = CachedScoreboard;
}
