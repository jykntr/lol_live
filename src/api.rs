use crate::types::ApiData;

pub async fn poll_game_data(client: &reqwest::Client) -> Option<ApiData> {
    let response = client
        .get("https://127.0.0.1:2999/liveclientdata/allgamedata")
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let json: serde_json::Value = response.json().await.ok()?;

    let summoner_name = json["activePlayer"]["summonerName"].as_str()?.to_string();

    let game_time = json["gameData"]["gameTime"].as_f64()?;

    let players = json["allPlayers"].as_array()?;
    for player in players {
        if player["summonerName"].as_str() == Some(&summoner_name) {
            let cs = player["scores"]["creepScore"].as_i64()?;
            return Some(ApiData {
                game_time,
                cs,
                summoner_name,
            });
        }
    }

    None
}
