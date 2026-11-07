use super::{GameConfig, GameResult, PlayerId, ResultReason};
use crate::gamemode::Gamemode;

pub(super) fn result(
    config: GameConfig,
    partner: Option<PlayerId>,
    points: [u16; 4],
    tricks_won: [u8; 4],
) -> GameResult {
    let declarer = config.declarer;
    let (winners, reason) = match config.mode {
        Gamemode::Bettel => {
            let taken = tricks_won[declarer.index()];
            let winners = PlayerId::ALL
                .into_iter()
                .filter(|player| (*player == declarer) == (taken == 0))
                .collect();
            (
                winners,
                ResultReason::Bettel {
                    declarer,
                    tricks_taken: taken,
                },
            )
        }
        Gamemode::Ramsch => {
            if let Some(player) = PlayerId::ALL
                .into_iter()
                .find(|player| tricks_won[player.index()] == 8)
            {
                (vec![player], ResultReason::Durchmarsch { player })
            } else {
                let highest_points = *points.iter().max().unwrap();
                let winners = PlayerId::ALL
                    .into_iter()
                    .filter(|player| points[player.index()] < highest_points)
                    .collect();
                (winners, ResultReason::Ramsch { highest_points })
            }
        }
        _ => {
            let team_points =
                points[declarer.index()] + partner.map_or(0, |player| points[player.index()]);
            let won = team_points >= 61;
            let winners = PlayerId::ALL
                .into_iter()
                .filter(|player| {
                    let on_team = *player == declarer || Some(*player) == partner;
                    on_team == won
                })
                .collect();
            (
                winners,
                ResultReason::Points {
                    declarer,
                    partner,
                    team_points,
                },
            )
        }
    };
    GameResult {
        winners,
        points,
        tricks_won,
        reason,
    }
}

#[cfg(test)]
mod tests;
