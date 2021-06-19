use std::time::Duration;

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use js_sys::Date;

use chess::*;
use serde::{Serialize, Deserialize, Deserializer};
use lunatic::engine::*;
use lunatic::time::*;

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(message: &str);
}

#[cfg(debug_assertions)]
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[cfg(not(debug_assertions))]
macro_rules! console_log {
    ($($t:tt)*) => {}
}

#[derive(Deserialize)]
struct SearchArgs {
    time_left: u64,
    #[serde(deserialize_with = "deserialize_board")]
    init_pos: Board,
    #[serde(deserialize_with = "deserialize_moves")]
    moves: Vec<ChessMove>
}

fn deserialize_board<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Board, D::Error> {
    let board: &str = Deserialize::deserialize(deserializer)?;
    Ok(board.parse().unwrap())
}

fn deserialize_moves<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<ChessMove>, D::Error> {
    let moves: Vec<&str> = Deserialize::deserialize(deserializer)?;
    Ok(moves.into_iter().map(|mv| mv.parse().unwrap()).collect())
}

fn on_new_search(mut callback: impl FnMut(SearchArgs) + 'static) {
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = addEventListener)]
        fn add_event_listener(event: &str, callback: &Closure<dyn FnMut(JsValue)>);
    }
    let callback = Box::new(move |value: JsValue| {
        let value = Reflect::get(&value, &"data".into()).unwrap();
        callback(value.into_serde().unwrap())
    });
    let callback = Closure::wrap(callback as _);
    add_event_listener("message", &callback);
    callback.forget();
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = postMessage)]
    fn post_message(message: JsValue);
}

fn post_search_result(result: &SearchResult, time: Duration) {
    #[derive(Serialize)]
    pub struct SerializableSearchResult {
        pub mv: String,
        pub value: String,
        pub nodes: u32,
        pub depth: u8,
        pub principal_variation: Vec<String>,
        pub transposition_table_size: usize,
        pub transposition_table_entries: usize,
        pub time: f64
    }
    let result = SerializableSearchResult {
        mv: result.mv.to_string(),
        value: format!("{}", result.value),
        nodes: result.nodes,
        depth: result.depth,
        principal_variation: result.principal_variation
            .iter().map(|mv| mv.to_string()).collect(),
        transposition_table_size: result.transposition_table_size,
        transposition_table_entries: result.transposition_table_entries,
        time: time.as_secs_f64()
    };
    post_message(JsValue::from_serde(&result).unwrap());
}

struct Handler {
    time_manager: StandardTimeManager,
    last_update: u64,
    time_left: Duration,
    start_time: u64
}

impl LunaticHandler for Handler {
    fn time_up(&mut self) -> bool {
        if Duration::from_millis(Date::now() as u64 - self.last_update) > self.time_left {
            post_message(JsValue::NULL);
            true
        } else {
            false
        }
    }

    fn search_result(&mut self, result: SearchResult) {
        let now = Date::now() as u64;
        post_search_result(&result, Duration::from_millis(now - self.start_time));
        let elapsed = Duration::from_millis(now - self.last_update);
        self.time_left = self.time_manager.update(result, elapsed);
        self.last_update = now;
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
    on_new_search(|args| {
        let time_left = Duration::from_millis(args.time_left);
        let now = Date::now() as u64;
        let handler = Handler {
            time_manager: StandardTimeManager::new(time_left, 0.04, Duration::ZERO),
            last_update: now,
            time_left: Duration::MAX,
            start_time: now
        };
        LunaticSearchState::new(
            handler,
            &args.init_pos,
            args.moves,
            SearchOptions::default()
        ).search();
    });
    post_message(JsValue::NULL);
}
