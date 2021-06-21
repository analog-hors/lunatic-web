use std::time::Duration;
use std::io::Cursor;

use js_sys::Reflect;
use wasm_bindgen::prelude::*;
use js_sys::Date;

use chess::*;
use serde::{Serialize, Deserialize, Deserializer};
use lunatic::engine::*;
use lunatic::time::*;
use lunatic::evaluator::*;
use chess_polyglot_reader::{PolyglotReader, PolyglotKey};

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(message: &str);

    #[wasm_bindgen(js_namespace = Math)]
    fn random() -> f64;
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
    think_time: u64,
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

#[derive(Serialize)]
#[serde(tag = "type")]
enum JsSearchResult {
    Book {
        mv: String,
        weight: u16
    },
    Engine {
        mv: String,
        value: String,
        nodes: u32,
        depth: u8,
        principal_variation: Vec<String>,
        transposition_table_size: usize,
        transposition_table_entries: usize,
        time: f64
    }
}

impl JsSearchResult {
    fn from_result(result: &SearchResult, time: Duration) -> Self {
        Self::Engine {
            mv: result.mv.to_string(),
            value: format!("{}", result.value),
            nodes: result.nodes,
            depth: result.depth,
            principal_variation: result.principal_variation
                .iter().map(|mv| mv.to_string()).collect(),
            transposition_table_size: result.transposition_table_size,
            transposition_table_entries: result.transposition_table_entries,
            time: time.as_secs_f64()
        }
    }
}

fn post_search_result(result: &JsSearchResult) {
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
        Duration::from_millis(Date::now() as u64 - self.last_update) > self.time_left
    }

    fn search_result(&mut self, result: SearchResult) {
        let now = Date::now() as u64;
        post_search_result(&JsSearchResult::from_result(
            &result, Duration::from_millis(now - self.start_time)
        ));
        let elapsed = Duration::from_millis(now - self.last_update);
        self.time_left = self.time_manager.update(result, elapsed);
        self.last_update = now;
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    const OPENINGS: &'static [u8] = include_bytes!("openings-8ply-10k.bin");
    
    on_new_search(|args| {
        let mut reader = PolyglotReader::new(Cursor::new(OPENINGS)).unwrap();
        let mut board = args.init_pos;
        for &mv in &args.moves {
            board = board.make_move_new(mv);
        }
        let key = PolyglotKey::from_board(&board);
        let entries = reader.get(&key).unwrap();
        let total: i32 = entries.iter().map(|e| e.weight as i32).sum();
        let mut value = (random() * total as f64) as i32;
        for mut entry in entries {
            value -= entry.weight as i32;
            if value < 0 {
                if matches!(entry.mv.source.into(), Square::E1 | Square::E8) {
                    let is_castle = match entry.mv.dest.into() {
                        Square::H1 => key.white_castle.king_side,
                        Square::A1 => key.white_castle.queen_side,
                        Square::H8 => key.black_castle.king_side,
                        Square::A8 => key.black_castle.queen_side,
                        _ => false
                    };
                    if is_castle {
                        if entry.mv.dest.file < entry.mv.source.file {
                            entry.mv.dest.file += 1;
                        } else {
                            entry.mv.dest.file -= 1;
                        }
                    }
                }
                post_search_result(&JsSearchResult::Book {
                    mv: format!("{}", ChessMove::from(entry.mv)),
                    weight: entry.weight
                });
                post_message(JsValue::NULL);
                return;
            }
        }

        let think_time = Duration::from_millis(args.think_time);
        let now = Date::now() as u64;
        let handler = Handler {
            time_manager: StandardTimeManager::new(Duration::ZERO, 0.0, think_time),
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
        post_message(JsValue::NULL);
    });
    post_message(JsValue::NULL);
}
