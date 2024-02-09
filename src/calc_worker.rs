use std::collections::VecDeque;
use std::ops::Deref;
use std::process::CommandArgs;
use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use gloo::timers::future::TimeoutFuture;
use js_sys::Uint8Array;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent, Worker, WorkerOptions, WorkerType};

use quoridor::{AIControlledBoard, PreCalc};

struct QuoridorWorker<'a> {
    worker: &'a mut Worker,
}

impl<'a> QuoridorWorker<'a> {
    pub fn send_command(&self, command: UserCommand) {
        let encoded = bincode::serialize(&command).unwrap();
        let uint8_array = js_sys::Uint8Array::new_with_length(encoded.len() as u32);
        uint8_array.copy_from(&encoded);
        self.worker.post_message(&JsValue::from(uint8_array));
    }
}

pub fn use_webworker(cx: &ScopeState) -> (QuoridorWorker, &UseState<CalculateUpdate>) {
    let latest_update = use_state(cx, || CalculateUpdate::Progress(0.0));

    let worker = cx.use_hook(|| {
        let worker = Worker::new_with_options("worker.js", &worker_options()).unwrap();

        let latest_update = latest_update.clone();
        let f: Closure<dyn Fn(MessageEvent) -> ()> = Closure::new(move |event: MessageEvent| {
            let data = event.data();
            let uint8_array: Uint8Array = data.into();
            log::info!("Message from main thread: {:?}", event.data());
            // Create a Vec<u8> with the same length as the Uint8Array
            let mut bytes = vec![0; uint8_array.length() as usize];
            log::info!("Bytes {:?}", bytes);
            // Copy the contents of the Uint8Array into the Vec<u8>
            uint8_array.copy_to(&mut bytes);
            let calculate_update: CalculateUpdate = bincode::deserialize(&bytes).unwrap();

            latest_update.set(calculate_update);
        });

        let val = f.into_js_value();
        let f = js_sys::Function::unchecked_from_js(val);
        worker.set_onmessage(Some(&f));

        worker
    });

    (QuoridorWorker { worker }, latest_update)
}

fn worker_options() -> WorkerOptions {
    let mut options = WorkerOptions::new();
    options.type_(WorkerType::Module);
    options
}

#[derive(Deserialize, Serialize, Debug)]
pub enum UserCommand {
    DecodeBoard,
    GameMove,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum CalculateUpdate {
    Finish,
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

// Here we will put the actual worker code. This will be running the monte carlo simulations in the background.
async fn internal_worker(user_commands: CommandChannel, calc_update_channel: WorkerUpdates) {
    let mut i = 0;
    let mut ai_controlled_board = AIControlledBoard::decode("0;10E1;10E9").unwrap();
    loop {
        if let Some(next_command) = user_commands.recv_next() {
            log::info!("Message from main thread: {:?}", next_command);
            match next_command {
                UserCommand::DecodeBoard => {
                    log::info!("Decoding board");
                    // decode the board
                }
                UserCommand::GameMove => {
                    log::info!("Game Move");
                    // make a game move
                }
            }
        }
        let resp = ai_controlled_board.ai_move(1000, &PreCalc::new());
        log::info!("AI Move: {:?}", resp);

        calc_update_channel.send_update(CalculateUpdate::Progress(i as f32 / 100.0));
        i += 1;
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

    let _scope = scope.clone();

    let message_received = std::sync::Arc::new(std::sync::Mutex::new(VecDeque::new()));
    let local_queu = message_received.clone();

    // Here we put messages send to the worker on the internal queu
    let f: Closure<dyn Fn(MessageEvent) -> ()> = Closure::new(move |event: MessageEvent| {
        let data = event.data();
        let uint8_array: Uint8Array = data.into();
        log::info!("Message from main thread: {:?}", event.data());
        log::info!("Message from main 2 thread: {:?}", event.data());
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
