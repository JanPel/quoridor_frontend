use std::collections::VecDeque;
use std::ops::Deref;
use std::process::CommandArgs;
use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use gloo::timers::future::TimeoutFuture;
use js_sys::Uint8Array;
use log::info;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicUsize;
use wasm_bindgen::prelude::*;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent, Worker, WorkerOptions, WorkerType};

use quoridor::{AIControlledBoard, Board, MonteCarloTree, Move, PreCalc};

static AI_PLAYER: AtomicUsize = AtomicUsize::new(1);

#[derive(Clone, Copy)]
pub struct QuoridorWorker<'a> {
    worker: &'a Worker,
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
    ai_player: usize,
) -> (QuoridorWorker, &UseState<CalculateUpdate>, &UseRef<Board>) {
    let latest_update = use_state(cx, || CalculateUpdate::Progress(0.0));
    let board = use_ref(cx, || Board::new());

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
                        if board.turn % 2 == ai_player {
                            let res = board.game_move(game_move);
                            info!("Taking AI MOVE AUTOMATICALLY: {:?}", res);
                        }
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

        let quoridor_worker = QuoridorWorker { worker: &worker };
        quoridor_worker.send_command(UserCommand::SetAIPlayer(ai_player));
        worker
    });

    (QuoridorWorker { worker }, latest_update, board)
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

async fn try_downloading_pre_calc(
    board: &Board,
) -> Result<MonteCarloTree, Box<dyn std::error::Error + Sync + Send>> {
    let resp = reqwest::get(format!(
        "http://localhost:8080/pre_calc_old/precalc/{}.mc_node",
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

async fn get_board_scores() -> Result<PreCalc, Box<dyn std::error::Error + Sync + Send>> {
    Ok(
        reqwest::get("http://localhost:8080/pre_calc_old/to_precalc.json")
            .await?
            .json()
            .await?,
    )
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
    loop {
        TimeoutFuture::new(10).await;
        if let Some(next_command) = user_commands.recv_next() {
            log::info!("Message from main thread: {:?}", next_command);
            match next_command {
                UserCommand::DecodeBoard => {
                    log::info!("Decoding board");
                    // decode the board
                }
                UserCommand::GameMove(game_move) => {
                    log::info!("Game Move {:?}", game_move);
                    ai_controlled_board.game_move(game_move);
                    match try_downloading_pre_calc(&ai_controlled_board.board).await {
                        Ok(mc_tree) => {
                            ai_controlled_board.relevant_mc_tree = mc_tree;
                        }
                        Err(err) => {
                            log::warn!("{}", err);
                        }
                    }
                    // make a game move
                }
                UserCommand::SetAIPlayer(player) => {
                    log::info!("Setting AI Player to {}", player);
                    AI_PLAYER.store(player, std::sync::atomic::Ordering::Release);
                }
            }
        }
        let resp = ai_controlled_board.ai_move(10_000, &pre_calc);
        //log::info!("AI Move: {:?}", resp);

        let number_visits = ai_controlled_board.relevant_mc_tree.mc_node.number_visits();
        log::info!("Number of visits: {:?}", number_visits);
        if number_visits > 100_000 {
            log::info!(
                "{}, {}",
                ai_controlled_board.board.turn % 2,
                AI_PLAYER.load(std::sync::atomic::Ordering::Acquire)
            );
            if ai_controlled_board.board.turn % 2
                == AI_PLAYER.load(std::sync::atomic::Ordering::Acquire)
            {
                log::info!("AI TOOK MOVE IN WORKER Move: {:?}", resp.0);
                ai_controlled_board.game_move(resp.0);
                calc_update_channel.send_update(CalculateUpdate::Finish(resp.0));
            }
        } else {
            calc_update_channel
                .send_update(CalculateUpdate::Progress(number_visits as f32 / 100_000.0));
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
        log::info!("closure called");
        let data = event.data();
        log::info!("Message from main thread: {:?}", &data);
        let uint8_array: Uint8Array = data.into();
        // Create a Vec<u8> with the same length as the Uint8Array
        let mut bytes = vec![0; uint8_array.length() as usize];
        log::info!("Message from main thread: {:?}", &bytes);
        // Copy the contents of the Uint8Array into the Vec<u8>
        uint8_array.copy_to(&mut bytes);
        let user_command: UserCommand = bincode::deserialize(&bytes).unwrap();
        log::info!("Message from main thread: {:?}", user_command);
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
