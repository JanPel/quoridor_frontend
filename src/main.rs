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
use quoridor::{Move, PawnMove};

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

                    worker.send_command(UserCommand::GameMove(Move::PawnMove(PawnMove::Up, None)));
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
