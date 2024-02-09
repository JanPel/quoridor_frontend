mod calc_worker;

use std::collections::VecDeque;
use std::ops::Deref;

use dioxus::prelude::*;
use gloo::timers::callback::Timeout;
use gloo::timers::future::TimeoutFuture;
use js_sys::Uint8Array;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::{
    DedicatedWorkerGlobalScope, MessageEvent, Worker, WorkerGlobalScope, WorkerOptions, WorkerType,
};

pub use calc_worker::start_webworker;
use calc_worker::*;

fn main() {
    wasm_logger::init(wasm_logger::Config::default());

    dioxus_web::launch(app);
}

fn app(cx: Scope) -> Element {
    let (worker, msgs) = use_webworker(cx);
    let message = use_state(cx, || "".to_string());

    let latest_update = format!("{:?}", msgs.get());
    render! {
        div {
            button {
                onclick: move |_| {
                    let msg = format!("Message from main: {}", message.get());

                    worker.send_command(UserCommand::GameMove);
                },
                "Send a message to the worker"
            }
            input {
                value: "{message}",
                oninput: move |event| {
                    message.set(event.value.clone());
                }
            }
        }
        div {
            div {
                "{latest_update}"
            }
        }
    }
}

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
