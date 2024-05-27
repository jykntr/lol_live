use std::time;

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let sleep_time = time::Duration::from_secs(3);
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;

    let mut last_cs = -1;

    loop {
        let response = client
            .get("https://127.0.0.1:2999/liveclientdata/allgamedata")
            .send()
            .await;

        match response {
            Ok(r) => {
                if r.status().is_success() {
                    let json: serde_json::Value = r.json().await?;

                    let summoner = json["activePlayer"]["summonerName"]
                        .as_str()
                        .expect("activePlayer[\"summonerName\"] was not a string!");

                    for player in json["allPlayers"]
                        .as_array()
                        .expect("Could not parse \"allPlayers\"")
                    {
                        if player["summonerName"]
                            .as_str()
                            .expect("Could not parse player summoner name")
                            == summoner
                        {
                            let cs = player["scores"]["creepScore"]
                                .as_i64()
                                .expect("Creep score not an integer!");

                            if cs > last_cs {
                                last_cs = cs;
                                let time = json["gameData"]["gameTime"]
                                    .as_f64()
                                    .expect("Game time was not a valid number!");
                                let cs_per_minute = cs as f64 / (time / 60.0);
                                let game_time = json["gameData"]["gameTime"]
                                    .as_f64()
                                    .expect("Game time not parsable");
                                println!(
                                    "CS per minute: {:.1} ({} CS @ {}:{:0>2})",
                                    cs_per_minute,
                                    cs,
                                    game_time as i64 / 60,
                                    game_time as i64 % 60,
                                );
                            }
                        }
                    }
                } else {
                    println!("Game loading...");
                    last_cs = -1;
                }
            }
            Err(_) => {
                last_cs = -1;
                println!("No live game detected...")
            }
        }

        tokio::time::sleep(sleep_time).await;
    }
}
