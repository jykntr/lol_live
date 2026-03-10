#[derive(Clone, Debug)]
pub struct CsRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug)]
pub struct ApiData {
    pub game_time: f64,
    pub cs: i64,
}
