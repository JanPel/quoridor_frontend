use std::collections::VecDeque;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use gloo::timers::future::TimeoutFuture;
use js_sys::Uint8Array;
use log::info;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent, Worker, WorkerOptions, WorkerType};

use quoridor::{AIControlledBoard, Board, MirrorMoveType, MonteCarloTree, Move, PreCalc};

//const BASE_URL: &str = "https://janpel.github.io/quoridor_frontend/";
const BASE_URL: &str = "http://localhost:8080/";
//const BASE_URL: &str = "https://storage.googleapis.com/quoridor_openingbook/";

#[derive(Clone, Copy)]
pub struct QuoridorWorker<'a> {
    worker: &'a Worker,
}

pub struct BoardWithHistory {
    pub board: Board,
    pub historic_moves: Vec<String>,
}

impl BoardWithHistory {
    fn new() -> Self {
        BoardWithHistory {
            board: Board::new(),
            historic_moves: vec![],
        }
    }

    pub fn game_move(&mut self, game_move: Move) {
        let quoridor_strats_move = game_move.to_quoridor_strat_notation(&self.board);
        self.historic_moves.push(quoridor_strats_move);
        self.board.game_move(game_move);
    }

    pub fn historic_moves(&self) -> String {
        self.historic_moves.join(";")
    }
}

impl<'a> QuoridorWorker<'a> {
    pub fn send_command(&self, command: UserCommand) {
        let encoded = bincode::serialize(&command).unwrap();
        let uint8_array = js_sys::Uint8Array::new_with_length(encoded.len() as u32);
        uint8_array.copy_from(&encoded);
        log::info!("Sending command to worker: {:?}", command);
        self.worker.post_message(&JsValue::from(uint8_array));
    }
}

pub fn use_webworker(
    cx: &ScopeState,
) -> (
    QuoridorWorker,
    &UseState<CalculateUpdate>,
    &UseRef<BoardWithHistory>,
    &UseState<Option<usize>>,
) {
    let latest_update = use_state(cx, || CalculateUpdate::Progress(0.0));
    let board = use_ref(cx, || BoardWithHistory::new());
    let ai_player: &UseState<Option<usize>> = use_state(cx, || None);

    let worker = cx.use_hook(|| {
        let worker = Worker::new_with_options("worker.js", &worker_options()).unwrap();

        let latest_update = latest_update.clone();
        let board = board.clone();
        let f: Closure<dyn Fn(MessageEvent) -> ()> = Closure::new(move |event: MessageEvent| {
            let data = event.data();
            let uint8_array: Uint8Array = data.into();
            // Create a Vec<u8> with the same length as the Uint8Array
            let mut bytes = vec![0; uint8_array.length() as usize];
            uint8_array.copy_to(&mut bytes);
            // Copy the contents of the Uint8Array into the Vec<u8>
            let calculate_update: CalculateUpdate = bincode::deserialize(&bytes).unwrap();

            match calculate_update {
                CalculateUpdate::Finish(game_move) => {
                    //log::info!("AI finish move suggested : {:?}", game_move);
                    board.with_mut(|board| {
                        let res = board.game_move(game_move);
                        info!("Taking AI {:?} MOVE AUTOMATICALLY: {:?}", game_move, res);
                    });
                }
                CalculateUpdate::Progress(f) => {
                    latest_update.set(CalculateUpdate::Progress(f));
                }
            }
            latest_update.set(calculate_update);
        });

        let val = f.into_js_value();
        let f = js_sys::Function::unchecked_from_js(val);
        worker.set_onmessage(Some(&f));
        worker
    });

    (QuoridorWorker { worker }, latest_update, board, ai_player)
}

fn worker_options() -> WorkerOptions {
    let mut options = WorkerOptions::new();
    options.type_(WorkerType::Module);
    options
}

#[derive(Deserialize, Serialize, Debug)]
pub enum UserCommand {
    DecodeBoard,
    GameMove(Move),
    SetAIPlayer(usize),
}

#[derive(Deserialize, Serialize, Debug)]
pub enum CalculateUpdate {
    Finish(Move),
    Progress(f32),
}

struct WorkerUpdates {
    scope: DedicatedWorkerGlobalScope,
}

impl WorkerUpdates {
    pub fn send_update(&self, update: CalculateUpdate) {
        let encoded = bincode::serialize(&update).unwrap();
        let uint8_array = js_sys::Uint8Array::new_with_length(encoded.len() as u32);
        uint8_array.copy_from(&encoded);
        self.scope.post_message(&JsValue::from(uint8_array));
    }
}

#[derive(Clone)]
struct CommandChannel {
    user_commands: Arc<Mutex<VecDeque<UserCommand>>>,
}

impl CommandChannel {
    pub fn recv_next(&self) -> Option<UserCommand> {
        self.user_commands.lock().unwrap().pop_front()
    }
}

#[derive(Deserialize, Serialize)]
struct SeenTableNew {
    tabel: String,
    win_rate: f64,
    visits: u32,
    ai_player: bool,
}

pub fn quoridor_strats_moves(historic_moves: &Vec<Move>) -> Vec<String> {
    let mut board = Board::new();
    let mut quoridor_strats_moves = vec![];
    for game_move in historic_moves {
        let quoridor_strats_move = game_move.to_quoridor_strat_notation(&board);
        quoridor_strats_moves.push(quoridor_strats_move);
        board.game_move(*game_move);
    }
    quoridor_strats_moves
}

async fn add_table(
    board: &Board,
    win_rate: f64,
    visits: u32,
    ai_player_zero: bool,
    historic_moves: Vec<Move>,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let url = "https://quoridorwebsite.shuttleapp.rs/seen_tables"; // Adjust the URL path as necessary

    let payload = SeenTableNew {
        tabel: board.encode(),
        win_rate,
        visits,
        ai_player: ai_player_zero,
    };

    let response = client.post(url).json(&payload).send().await?;
    let status = response.status();
    if status.is_success() {
        Ok(())
    } else {
        // format the server error
        return Err(format!(
            "Failed to add table: {}, http response: {}",
            response.text().await?,
            status
        ))?;
    }
}

async fn add_table_ignore_error(
    board: Board,
    win_rate: f64,
    visits: u32,
    ai_player_zero: bool,
    historic_moves: Vec<Move>,
) {
    if let Err(err) = add_table(&board, win_rate, visits, ai_player_zero, historic_moves).await {
        log::warn!("{}", err);
    }
}

async fn store_table_if_unknown_and_ai_loses(
    ai_controlled_board: &AIControlledBoard,
    ai_player: usize,
    historic_moves: &Vec<Move>,
) -> Result<(), Box<dyn std::error::Error>> {
    let score = ai_controlled_board.relevant_mc_tree.mc_node.scores();
    let win_rate_prev_player = score.0 as f64 / score.1 as f64;
    let win_rate_ai = if ai_player == ai_controlled_board.board.turn % 2 {
        1.0 - win_rate_prev_player
    } else {
        win_rate_prev_player
    };
    if win_rate_ai < 0.4 && score.1 > 300_000 {
        wasm_bindgen_futures::spawn_local(add_table_ignore_error(
            ai_controlled_board.board.clone(),
            win_rate_ai,
            score.1,
            ai_player == 0,
            historic_moves.clone(),
        ));
    }
    Ok(())
}
async fn try_downloading_pre_calc(
    board: &Board,
) -> Result<MonteCarloTree, Box<dyn std::error::Error + Sync + Send>> {
    let resp = reqwest::get(format!(
        "{}precalc/precalc/{}.mc_node",
        BASE_URL,
        board.encode()
    ))
    .await?;
    if resp.status() == 200 {
        let body = resp.bytes().await?;
        Ok(MonteCarloTree::deserialize(&body))
    } else {
        return Err("not found")?;
    }
}

async fn take_game_move(
    ai_controlled_board: &mut AIControlledBoard,
    pre_calc: &PreCalc,
    game_move: Move,
    ai_player: usize,
    mirror_calc_board: &mut Option<bool>,
    historic_moves: &Vec<Move>,
) {
    ai_controlled_board.game_move(game_move);
    if let Some((score_zero, pre_calc_mirrored)) =
        pre_calc.roll_out_score(&ai_controlled_board.board)
    {
        let to_download = if pre_calc_mirrored {
            Board::decode(&ai_controlled_board.board.encode_mirror()).unwrap()
        } else {
            ai_controlled_board.board.clone()
        };
        match try_downloading_pre_calc(&to_download).await {
            Ok(mc_tree) => {
                log::info!(
                    "Found precalc {} with {} visits, is precalc mirror {}",
                    to_download.encode(),
                    mc_tree.mc_node.number_visits(),
                    pre_calc_mirrored
                );
                ai_controlled_board.relevant_mc_tree = mc_tree;
                ai_controlled_board.board = to_download;
                //ai_controlled_board
                //    .relevant_mc_tree
                //    .select_best_move(&ai_controlled_board.board, &pre_calc);
                *mirror_calc_board = match (*mirror_calc_board, pre_calc_mirrored) {
                    (Some(value), true) => Some(!value),
                    (Some(value), false) => Some(value),
                    (None, _) => None,
                };
            }
            Err(err) => {
                log::warn!("{}", err);
            }
        }
    } else {
        if let Err(err) =
            store_table_if_unknown_and_ai_loses(ai_controlled_board, ai_player, historic_moves)
                .await
        {
            log::warn!("{}", err);
        }
    }
}

async fn get_board_scores() -> Result<PreCalc, Box<dyn std::error::Error + Sync + Send>> {
    Ok(reqwest::get(format!("{}precalc/to_precalc.json", BASE_URL))
        .await?
        .json()
        .await?)
}
// Here we will put the actual worker code. This will be running the monte carlo simulations in the background.
async fn internal_worker(user_commands: CommandChannel, calc_update_channel: WorkerUpdates) {
    let mut ai_controlled_board = AIControlledBoard::decode("0;10E1;10E9").unwrap();
    let pre_calc: PreCalc = match get_board_scores().await {
        Ok(pre_calc) => pre_calc,
        Err(err) => {
            log::warn!("{}", err);
            PreCalc::new()
        }
    };
    if let Ok(rel_tree) = try_downloading_pre_calc(&ai_controlled_board.board).await {
        ai_controlled_board.relevant_mc_tree = rel_tree;
    }
    let mut ai_player = None;

    let mut mirror_calc_board: Option<bool> = None;
    let mut new_command = false;
    let mut historic_moves = vec![];
    loop {
        TimeoutFuture::new(10).await;
        if let Some(next_command) = user_commands.recv_next() {
            new_command = true;
            //log::info!("Message from main thread: {:?}", next_command);
            match next_command {
                UserCommand::DecodeBoard => {
                    log::info!("Decoding board");
                    // decode the board
                }
                UserCommand::GameMove(game_move) => {
                    log::info!("Game Move {:?}", game_move);
                    if mirror_calc_board.is_none() {
                        match game_move.mirror_move_type() {
                            MirrorMoveType::Right => {
                                mirror_calc_board = Some(true);
                            }
                            MirrorMoveType::Left => {
                                mirror_calc_board = Some(false);
                            }
                            _ => {}
                        }
                    }

                    historic_moves.push(game_move);
                    let game_move = if mirror_calc_board == Some(true) {
                        game_move.mirror_move()
                    } else {
                        game_move
                    };
                    take_game_move(
                        &mut ai_controlled_board,
                        &pre_calc,
                        game_move,
                        ai_player.unwrap(),
                        &mut mirror_calc_board,
                        &historic_moves,
                    )
                    .await;
                    // make a game move
                }
                UserCommand::SetAIPlayer(player) => {
                    log::info!("Setting AI Player to {}", player);
                    ai_player = Some(player);
                }
            }
        }

        let number_visits = ai_controlled_board.relevant_mc_tree.mc_node.number_visits();
        if (ai_controlled_board.is_played_out() || number_visits >= 20_000_000) && !new_command {
            // Here we want to just wait for 100 ms and then continue, so to make it more responsive
            TimeoutFuture::new(100).await;
            continue;
        }
        let number_of_steps = if new_command { 100 } else { 10_000 };

        let resp = ai_controlled_board.ai_move(number_of_steps, &pre_calc);
        //log::info!("AI Move: {:?}", resp);

        if number_visits > 600_000
            || ai_controlled_board.is_played_out()
            || resp.number_of_simulations >= 300_000
        {
            if ai_player == Some(ai_controlled_board.board.turn % 2) {
                log::info!("AI TOOK MOVE IN WORKER Move: {:?}", resp.suggested_move);
                let game_move = resp.suggested_move;
                if mirror_calc_board.is_none() {
                    match game_move.mirror_move_type() {
                        MirrorMoveType::Right => {
                            mirror_calc_board = Some(true);
                        }
                        MirrorMoveType::Left => {
                            mirror_calc_board = Some(false);
                        }
                        _ => {}
                    }
                }
                let to_send = if mirror_calc_board == Some(true) {
                    resp.suggested_move.mirror_move()
                } else {
                    resp.suggested_move
                };
                historic_moves.push(to_send);
                calc_update_channel.send_update(CalculateUpdate::Finish(to_send));
                take_game_move(
                    &mut ai_controlled_board,
                    &pre_calc,
                    resp.suggested_move,
                    ai_player.unwrap(),
                    &mut mirror_calc_board,
                    &historic_moves,
                )
                .await;
            }
            new_command = false;
        } else {
            new_command = false;
            calc_update_channel
                .send_update(CalculateUpdate::Progress(number_visits as f32 / 600_000.0));
        }
    }
}

// todo: on the dioxus side of things, we can make this a macro or something that writes the JS snippet automatically to
// link it all together
#[wasm_bindgen]
pub async fn start_webworker() {
    log::info!("Starting webworker");

    log::info!("Starting MORE LOGGING");

    let self_ = js_sys::global();
    let js_value = self_.deref();
    let scope = DedicatedWorkerGlobalScope::unchecked_from_js_ref(js_value);
    // let scope = WorkerGlobalScope::unchecked_from_js_ref(js_value);

    let message_received = std::sync::Arc::new(std::sync::Mutex::new(VecDeque::new()));
    let local_queu = message_received.clone();

    // Here we put messages send to the worker on the internal queu
    let f: Closure<dyn Fn(MessageEvent) -> ()> = Closure::new(move |event: MessageEvent| {
        let data = event.data();
        let uint8_array: Uint8Array = data.into();
        // Create a Vec<u8> with the same length as the Uint8Array
        let mut bytes = vec![0; uint8_array.length() as usize];
        // Copy the contents of the Uint8Array into the Vec<u8>
        uint8_array.copy_to(&mut bytes);
        let user_command: UserCommand = bincode::deserialize(&bytes).unwrap();
        local_queu.lock().unwrap().push_back(user_command);
    });
    let val = f.into_js_value();
    let f = js_sys::Function::unchecked_from_js(val);
    scope.set_onmessage(Some(&f));

    let command_channel = CommandChannel {
        user_commands: message_received.clone(),
    };
    let worker_updates = WorkerUpdates {
        scope: scope.clone(),
    };
    internal_worker(command_channel, worker_updates).await;
}
